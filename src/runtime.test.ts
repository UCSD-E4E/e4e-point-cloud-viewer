import { describe, expect, it } from "vitest";
import { isTauriRuntime } from "./runtime.ts";

describe("isTauriRuntime", () => {
  it("is false when the Tauri internals marker is absent", () => {
    expect(isTauriRuntime({})).toBe(false);
  });

  it("is true when the Tauri internals marker is present", () => {
    expect(isTauriRuntime({ __TAURI_INTERNALS__: {} })).toBe(true);
  });

  it("defaults to the global scope (no Tauri under vitest)", () => {
    expect(isTauriRuntime()).toBe(false);
  });
});
