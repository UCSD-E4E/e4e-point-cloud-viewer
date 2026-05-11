import { describe, expect, it } from "vitest";
import { anaglyphMatricesFor } from "./anaglyph.ts";

describe("anaglyphMatricesFor", () => {
  it("full-color: left passes only red, right passes only green+blue", () => {
    expect(anaglyphMatricesFor("full-color")).toEqual({
      left: [1, 0, 0, 0, 0, 0, 0, 0, 0],
      right: [0, 0, 0, 0, 1, 0, 0, 0, 1],
    });
  });

  it("half-color: left contributes Rec601 luma to red, right passes green+blue", () => {
    expect(anaglyphMatricesFor("half-color")).toEqual({
      left: [0.299, 0, 0, 0.587, 0, 0, 0.114, 0, 0],
      right: [0, 0, 0, 0, 1, 0, 0, 0, 1],
    });
  });

  it("dubois: reproduces the matrices three.js's AnaglyphEffect ships with", () => {
    // Source of truth: three/examples/jsm/effects/AnaglyphEffect.js. If three
    // ever revises these we want the test to flag the drift, not silently
    // diverge from upstream's default.
    expect(anaglyphMatricesFor("dubois")).toEqual({
      left: [
        0.4561, -0.0400822, -0.0152161,
        0.500484, -0.0378246, -0.0205971,
        0.176381, -0.0157589, -0.00546856,
      ],
      right: [
        -0.0434706, 0.378476, -0.0721527,
        -0.0879388, 0.73364, -0.112961,
        -0.00155529, -0.0184503, 1.2264,
      ],
    });
  });
});
