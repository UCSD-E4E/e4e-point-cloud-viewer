/// The importer app: the full desktop flow (open .laz → normalize → bake →
/// view), wired against the Tauri backend. `main.ts` dynamically imports this
/// only when running under Tauri, so the `@tauri-apps/*` packages it pulls in
/// stay out of the browser bundle.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { AnaglyphMode } from "./anaglyph.ts";
import { type BakeProgress, type BakeResult, formatBakeResult } from "./bake.ts";
import { SplatRenderer } from "./renderer.ts";
import { unpackSplatCloud } from "./splatCloud.ts";
import { type CloudSummary, formatExtentLine, formatSummary } from "./summary.ts";

type UpAxis = "PosX" | "NegX" | "PosY" | "NegY" | "PosZ" | "NegZ";

interface NormalizationResult {
  normalization: {
    translation: [number, number, number];
    scale: number;
    rotation: number[][];
  };
  normalized_bbox_min: [number, number, number];
  normalized_bbox_max: [number, number, number];
}

export function mountImporter(): void {
  const $ = <T extends Element>(sel: string): T => {
    const el = document.querySelector<T>(sel);
    if (!el) throw new Error(`missing element: ${sel}`);
    return el;
  };

  const openBtn = $<HTMLButtonElement>("#open-btn");
  const pathOut = $<HTMLParagraphElement>("#path-out");
  const summaryOut = $<HTMLPreElement>("#summary-out");
  const normalizedOut = $<HTMLPreElement>("#normalized-out");
  const invertZBox = $<HTMLInputElement>("#invert-z");
  const sourceSection = $<HTMLElement>("#source-section");
  const orientationSection = $<HTMLElement>("#orientation-section");
  const normalizedSection = $<HTMLElement>("#normalized-section");
  const bakeSection = $<HTMLElement>("#bake-section");
  const bakeBtn = $<HTMLButtonElement>("#bake-btn");
  const resetCameraBtn = $<HTMLButtonElement>("#reset-camera-btn");
  const fullscreenBtn = $<HTMLButtonElement>("#fullscreen-btn");
  const bakeProgressBar = $<HTMLProgressElement>("#bake-progress");
  const bakeOut = $<HTMLPreElement>("#bake-out");
  const splatCanvas = $<HTMLCanvasElement>("#splat-canvas");
  const sizeSlider = $<HTMLInputElement>("#size-slider");
  const sizeValue = $<HTMLSpanElement>("#size-value");
  const saturationSlider = $<HTMLInputElement>("#saturation-slider");
  const saturationValue = $<HTMLSpanElement>("#saturation-value");
  const bgPicker = $<HTMLInputElement>("#bg-picker");
  const anaglyphToggle = $<HTMLInputElement>("#anaglyph-toggle");
  const anaglyphModeSelect = $<HTMLSelectElement>("#anaglyph-mode");
  const recentSection = $<HTMLElement>("#recent-section");
  const recentList = $<HTMLUListElement>("#recent-list");

  let currentSummary: CloudSummary | null = null;
  let currentPath: string | null = null;

  const splatRenderer = new SplatRenderer(splatCanvas);

  sizeSlider.addEventListener("input", () => {
    const v = Number.parseFloat(sizeSlider.value);
    splatRenderer.setSizeMultiplier(v);
    sizeValue.textContent = `×${v.toFixed(1)}`;
  });

  saturationSlider.addEventListener("input", () => {
    const v = Number.parseFloat(saturationSlider.value);
    splatRenderer.setSaturation(v);
    saturationValue.textContent = `×${v.toFixed(2)}`;
  });

  bgPicker.addEventListener("input", () => {
    splatRenderer.setBackgroundColor(bgPicker.value);
    splatCanvas.style.background = bgPicker.value;
  });

  anaglyphToggle.addEventListener("change", () => {
    splatRenderer.setAnaglyph(anaglyphToggle.checked);
  });

  anaglyphModeSelect.addEventListener("change", () => {
    splatRenderer.setAnaglyphMode(anaglyphModeSelect.value as AnaglyphMode);
  });

  void listen<BakeProgress>("bake-progress", (event) => {
    bakeProgressBar.max = event.payload.total;
    bakeProgressBar.value = event.payload.done;
  });

  function readUpAxis(): UpAxis {
    const checked = document.querySelector<HTMLInputElement>(
      'input[name="up-axis"]:checked',
    );
    return (checked?.value ?? "PosZ") as UpAxis;
  }

  async function refreshNormalization() {
    if (!currentSummary) return;
    try {
      const result = await invoke<NormalizationResult>("compute_normalization", {
        summary: currentSummary,
        upAxis: readUpAxis(),
        invertZ: invertZBox.checked,
      });
      normalizedOut.textContent = [
        `scale: ×${result.normalization.scale.toFixed(6)}`,
        formatExtentLine(result.normalized_bbox_min, result.normalized_bbox_max),
      ].join("\n");
      normalizedSection.hidden = false;
    } catch (err) {
      normalizedOut.textContent = `error: ${String(err)}`;
    }
  }

  document.querySelectorAll<HTMLInputElement>('input[name="up-axis"]').forEach((el) => {
    el.addEventListener("change", () => void refreshNormalization());
  });
  invertZBox.addEventListener("change", () => void refreshNormalization());

  resetCameraBtn.addEventListener("click", () => splatRenderer.resetCamera());

  fullscreenBtn.addEventListener("click", () => splatRenderer.toggleFullscreen());

  document.addEventListener("fullscreenchange", () => {
    fullscreenBtn.textContent = document.fullscreenElement === splatCanvas
      ? "Exit fullscreen"
      : "Fullscreen";
  });

  window.addEventListener("keydown", (e) => {
    if (e.key === "f" && !(e.target instanceof HTMLInputElement)) {
      splatRenderer.toggleFullscreen();
    }
  });

  function bboxFrom(positions: Float32Array): {
    min: [number, number, number];
    max: [number, number, number];
  } {
    if (positions.length === 0) {
      return { min: [0, 0, 0], max: [0, 0, 0] };
    }
    const min: [number, number, number] = [Infinity, Infinity, Infinity];
    const max: [number, number, number] = [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < positions.length; i += 3) {
      for (let a = 0; a < 3; a++) {
        const v = positions[i + a]!;
        if (v < min[a]!) min[a] = v;
        if (v > max[a]!) max[a] = v;
      }
    }
    return { min, max };
  }

  bakeBtn.addEventListener("click", () => {
    void (async () => {
      if (!currentSummary || !currentPath) return;
      bakeBtn.disabled = true;
      bakeProgressBar.value = 0;
      bakeOut.textContent = "(baking…)";
      const startedAt = performance.now();
      try {
        const buffer = await invoke<ArrayBuffer>("bake_cloud", {
          path: currentPath,
          summary: currentSummary,
          upAxis: readUpAxis(),
          invertZ: invertZBox.checked,
        });
        const elapsed_ms = Math.round(performance.now() - startedAt);
        bakeProgressBar.value = bakeProgressBar.max;

        const cloud = unpackSplatCloud(buffer);
        splatRenderer.setCloud(cloud);
        resetCameraBtn.hidden = false;
        fullscreenBtn.hidden = false;

        const { min, max } = bboxFrom(cloud.positions);
        const result: BakeResult = {
          splat_count: cloud.count,
          bbox_min: min,
          bbox_max: max,
          elapsed_ms,
        };
        bakeOut.textContent = formatBakeResult(result);
      } catch (err) {
        bakeOut.textContent = `error: ${String(err)}`;
      } finally {
        bakeBtn.disabled = false;
      }
    })();
  });

  function renderRecentList(paths: string[]) {
    recentList.innerHTML = "";
    if (paths.length === 0) {
      recentSection.hidden = true;
      return;
    }
    for (const p of paths) {
      const li = document.createElement("li");
      const btn = document.createElement("button");
      btn.type = "button";
      btn.textContent = p;
      btn.addEventListener("click", () => void loadFromPath(p));
      li.appendChild(btn);
      recentList.appendChild(li);
    }
    recentSection.hidden = false;
  }

  async function loadFromPath(picked: string) {
    currentPath = picked;
    pathOut.textContent = picked;
    pathOut.dataset.state = "loading";
    summaryOut.textContent = "(reading…)";
    bakeOut.textContent = "(not baked yet)";
    bakeProgressBar.value = 0;
    resetCameraBtn.hidden = true;
    fullscreenBtn.hidden = true;

    try {
      currentSummary = await invoke<CloudSummary>("summarize_cloud", { path: picked });
      summaryOut.textContent = formatSummary(currentSummary);
      pathOut.dataset.state = "ok";
      sourceSection.hidden = false;
      orientationSection.hidden = false;
      bakeSection.hidden = false;
      await refreshNormalization();
      const updated = await invoke<string[]>("add_recent_file", { path: picked });
      renderRecentList(updated);
    } catch (err) {
      summaryOut.textContent = `error: ${String(err)}`;
      pathOut.dataset.state = "error";
      currentSummary = null;
      currentPath = null;
    }
  }

  openBtn.addEventListener("click", () => {
    void (async () => {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "LAZ point cloud", extensions: ["laz", "las"] }],
      });
      if (typeof picked !== "string") return;
      await loadFromPath(picked);
    })();
  });

  void (async () => {
    try {
      const recent = await invoke<string[]>("get_recent_files");
      renderRecentList(recent);
    } catch (err) {
      console.warn("Failed to load recent files:", err);
    }
  })();
}
