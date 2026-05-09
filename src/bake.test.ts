import { describe, expect, it } from "vitest";
import { type BakeResult, formatBakeResult } from "./bake.ts";

const baseResult: BakeResult = {
  splat_count: 110_000,
  bbox_min: [-2.5, -1.4, -0.5],
  bbox_max: [2.5, 1.4, 0.5],
  elapsed_ms: 2150,
};

describe("formatBakeResult", () => {
  it("groups thousands in the splat count", () => {
    expect(formatBakeResult(baseResult)).toContain("110,000 splats");
  });

  it("reports elapsed time in seconds when over a second", () => {
    expect(formatBakeResult(baseResult)).toContain("2.15 s");
  });

  it("reports elapsed time in milliseconds for sub-second bakes", () => {
    expect(formatBakeResult({ ...baseResult, elapsed_ms: 250 })).toContain("250 ms");
  });

  it("includes the output extent line", () => {
    const out = formatBakeResult(baseResult);
    expect(out).toContain("extent: 5.00 × 2.80 × 1.00");
  });
});
