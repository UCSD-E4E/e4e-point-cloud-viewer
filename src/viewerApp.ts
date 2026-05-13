/// The viewer-only app: what the browser PWA mounts (no file picker for LAZ,
/// no normalize UI, no bake controls — just render a baked `.e4epc` cloud).
///
/// `main.ts` calls `mountViewer()` when not running under Tauri. The cloud comes
/// from `?cloud=<url>` (served by `e4epc-server`) or from a file the user opens;
/// see `cloudSource.ts`.

import type { AnaglyphMode } from "./anaglyph.ts";
import {
  type LoadedCloud,
  displayNameFromUrl,
  loadCloudFromFile,
  loadCloudFromUrl,
  parseCloudParam,
} from "./cloudSource.ts";
import { SplatRenderer } from "./renderer.ts";
import type { SplatCloud } from "./splatCloud.ts";

/** The slice of `SplatRenderer` the viewer drives — narrow so tests can stub it. */
export interface ViewerRenderer {
  setCloud(cloud: SplatCloud): void;
  setSizeMultiplier(value: number): void;
  setAnaglyph(enabled: boolean): void;
  setAnaglyphMode(mode: AnaglyphMode): void;
  toggleFullscreen(): void;
}

export interface MountViewerDeps {
  /** Build the renderer for the canvas. Defaults to a real `SplatRenderer`. */
  createRenderer?: (canvas: HTMLCanvasElement) => ViewerRenderer;
  /** Load a cloud from a URL. Defaults to `loadCloudFromUrl`. */
  loadFromUrl?: (url: string) => Promise<LoadedCloud>;
  /** Load a cloud from an opened file. Defaults to `loadCloudFromFile`. */
  loadFromFile?: (file: Blob) => Promise<LoadedCloud>;
  /** Query string to read `?cloud=` from. Defaults to `window.location.search`. */
  search?: string;
}

const NUM = new Intl.NumberFormat("en-US");

export function mountViewer(deps: MountViewerDeps = {}): void {
  const createRenderer = deps.createRenderer ?? ((c) => new SplatRenderer(c));
  const loadFromUrl = deps.loadFromUrl ?? loadCloudFromUrl;
  const loadFromFile = deps.loadFromFile ?? loadCloudFromFile;
  const search = deps.search ?? window.location.search;

  const $ = <T extends Element>(sel: string): T => {
    const el = document.querySelector<T>(sel);
    if (!el) throw new Error(`missing element: ${sel}`);
    return el;
  };
  const canvas = $<HTMLCanvasElement>("#viewer-canvas");
  const info = $<HTMLParagraphElement>("#viewer-info");
  const fallback = $<HTMLParagraphElement>("#viewer-fallback");
  const fileInput = $<HTMLInputElement>("#viewer-file");
  const anaglyphToggle = $<HTMLInputElement>("#viewer-anaglyph");
  const anaglyphModeSelect = $<HTMLSelectElement>("#viewer-anaglyph-mode");
  const sizeSlider = $<HTMLInputElement>("#viewer-size");
  const fullscreenBtn = $<HTMLButtonElement>("#viewer-fullscreen");

  const renderer = createRenderer(canvas);

  anaglyphToggle.addEventListener("change", () => renderer.setAnaglyph(anaglyphToggle.checked));
  anaglyphModeSelect.addEventListener("change", () =>
    renderer.setAnaglyphMode(anaglyphModeSelect.value as AnaglyphMode),
  );
  sizeSlider.addEventListener("input", () =>
    renderer.setSizeMultiplier(Number.parseFloat(sizeSlider.value)),
  );
  fullscreenBtn.addEventListener("click", () => renderer.toggleFullscreen());

  const show = (loaded: LoadedCloud): void => {
    renderer.setCloud(loaded.cloud);
    info.textContent = `${loaded.name} — ${NUM.format(loaded.cloud.count)} splats`;
    fallback.hidden = true;
  };
  const fail = (message: string): void => {
    info.textContent = message;
    fallback.hidden = false;
  };

  fileInput.addEventListener("change", () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    info.textContent = `Loading ${file.name}…`;
    loadFromFile(file)
      .then(show)
      .catch((err: unknown) => fail(`Couldn't read ${file.name}: ${String(err)}`));
  });

  const cloudUrl = parseCloudParam(search);
  if (cloudUrl === null) {
    fail("No cloud specified. Open an .e4epc file below, or follow a ?cloud=… link.");
    return;
  }
  const cloudName = displayNameFromUrl(cloudUrl);
  info.textContent = `Loading ${cloudName}…`;
  loadFromUrl(cloudUrl)
    .then(show)
    .catch((err: unknown) => fail(`Couldn't load ${cloudName}: ${String(err)}`));
}
