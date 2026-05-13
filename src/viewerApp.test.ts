import { beforeEach, describe, expect, it } from "vitest";
import type { SplatCloud } from "./splatCloud.ts";
import { type ViewerRenderer, mountViewer } from "./viewerApp.ts";

const VIEWER_HTML = `
  <div id="viewer-root">
    <p id="viewer-info"></p>
    <p id="viewer-fallback" hidden></p>
    <input type="file" id="viewer-file" />
    <input type="checkbox" id="viewer-anaglyph" />
    <select id="viewer-anaglyph-mode">
      <option value="half-color"></option>
      <option value="full-color"></option>
    </select>
    <input type="range" id="viewer-size" value="1" />
    <button id="viewer-fullscreen"></button>
    <canvas id="viewer-canvas"></canvas>
  </div>
`;

function fakeCloud(count: number): SplatCloud {
  return {
    count,
    positions: new Float32Array(count * 3),
    colors: new Uint8Array(count * 4),
    normals: new Float32Array(count * 3),
    radii: new Float32Array(count),
  };
}

function stubRenderer(): {
  renderer: ViewerRenderer;
  clouds: SplatCloud[];
  sizes: number[];
  anaglyph: boolean[];
  modes: string[];
  fullscreenCalls: () => number;
} {
  const clouds: SplatCloud[] = [];
  const sizes: number[] = [];
  const anaglyph: boolean[] = [];
  const modes: string[] = [];
  let fullscreen = 0;
  return {
    clouds,
    sizes,
    anaglyph,
    modes,
    fullscreenCalls: () => fullscreen,
    renderer: {
      setCloud: (c) => clouds.push(c),
      setSizeMultiplier: (v) => sizes.push(v),
      setAnaglyph: (b) => anaglyph.push(b),
      setAnaglyphMode: (m) => modes.push(m),
      toggleFullscreen: () => {
        fullscreen += 1;
      },
    },
  };
}

/** Resolve after the microtask queue *and* a timer tick — flushes load `.then`s. */
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  document.body.innerHTML = VIEWER_HTML;
});

describe("mountViewer", () => {
  it("loads the cloud named by ?cloud= and shows it in the info bar", async () => {
    const stub = stubRenderer();
    mountViewer({
      createRenderer: () => stub.renderer,
      loadFromUrl: async (url) => {
        expect(url).toBe("/fixtures/autzen.e4epc");
        return { cloud: fakeCloud(42), name: "autzen.e4epc" };
      },
      search: "?cloud=/fixtures/autzen.e4epc",
    });
    await flush();

    expect(stub.clouds).toHaveLength(1);
    expect(stub.clouds[0]!.count).toBe(42);
    const info = document.querySelector("#viewer-info")!.textContent ?? "";
    expect(info).toContain("autzen.e4epc");
    expect(info).toContain("42");
    expect((document.querySelector("#viewer-fallback") as HTMLElement).hidden).toBe(true);
  });

  it("shows the file-picker fallback when there is no ?cloud=", () => {
    const stub = stubRenderer();
    mountViewer({ createRenderer: () => stub.renderer, search: "" });

    expect(stub.clouds).toHaveLength(0);
    expect((document.querySelector("#viewer-fallback") as HTMLElement).hidden).toBe(false);
  });

  it("shows the fallback (and names the cloud) when a ?cloud= load fails", async () => {
    const stub = stubRenderer();
    mountViewer({
      createRenderer: () => stub.renderer,
      loadFromUrl: async () => {
        throw new Error("404 Not Found");
      },
      search: "?cloud=/c/missing.e4epc",
    });
    await flush();

    expect(stub.clouds).toHaveLength(0);
    expect((document.querySelector("#viewer-fallback") as HTMLElement).hidden).toBe(false);
    expect(document.querySelector("#viewer-info")!.textContent ?? "").toContain("missing.e4epc");
  });

  it("renders a cloud opened via the file picker", async () => {
    const stub = stubRenderer();
    const opened: Blob[] = [];
    mountViewer({
      createRenderer: () => stub.renderer,
      loadFromFile: async (file) => {
        opened.push(file);
        return { cloud: fakeCloud(7), name: "picked.e4epc" };
      },
      search: "",
    });

    const input = document.querySelector<HTMLInputElement>("#viewer-file")!;
    const file = new File([new ArrayBuffer(4)], "picked.e4epc");
    Object.defineProperty(input, "files", { value: [file], configurable: true });
    input.dispatchEvent(new Event("change"));
    await flush();

    expect(opened).toHaveLength(1);
    expect(stub.clouds.at(-1)!.count).toBe(7);
    expect(document.querySelector("#viewer-info")!.textContent ?? "").toContain("picked.e4epc");
  });

  it("drives the renderer from the view controls", () => {
    const stub = stubRenderer();
    mountViewer({ createRenderer: () => stub.renderer, search: "" });

    const anaglyph = document.querySelector<HTMLInputElement>("#viewer-anaglyph")!;
    anaglyph.checked = true;
    anaglyph.dispatchEvent(new Event("change"));
    expect(stub.anaglyph).toEqual([true]);

    const mode = document.querySelector<HTMLSelectElement>("#viewer-anaglyph-mode")!;
    mode.value = "full-color";
    mode.dispatchEvent(new Event("change"));
    expect(stub.modes).toEqual(["full-color"]);

    const size = document.querySelector<HTMLInputElement>("#viewer-size")!;
    size.value = "2.5";
    size.dispatchEvent(new Event("input"));
    expect(stub.sizes).toEqual([2.5]);

    document.querySelector("#viewer-fullscreen")!.dispatchEvent(new Event("click"));
    expect(stub.fullscreenCalls()).toBe(1);
  });
});
