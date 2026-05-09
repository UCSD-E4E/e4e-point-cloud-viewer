import { describe, expect, it } from "vitest";
import {
  type CloudSummary,
  extentOf,
  formatExtentLine,
  formatSummary,
} from "./summary.ts";

const autzen: CloudSummary = {
  point_count: 110_000,
  bbox_min: [636_001.76, 848_935.2, 406.26],
  bbox_max: [637_179.22, 849_497.9, 520.51],
  has_color: true,
};

describe("extentOf", () => {
  it("returns max - min on each axis", () => {
    expect(extentOf([0, 0, 0], [10, 20, 30])).toEqual([10, 20, 30]);
  });

  it("handles negative bbox bounds", () => {
    expect(extentOf([-5, -2, -1], [5, 8, 9])).toEqual([10, 10, 10]);
  });
});

describe("formatExtentLine", () => {
  it("formats with two decimals and × separators", () => {
    expect(formatExtentLine([0, 0, 0], [1.5, 2.5, 3.5])).toBe(
      "extent: 1.50 × 2.50 × 3.50",
    );
  });
});

describe("formatSummary", () => {
  it("groups thousands in the point count", () => {
    expect(formatSummary(autzen)).toContain("110,000 points");
  });

  it("reports color when present", () => {
    expect(formatSummary(autzen)).toContain("color: yes");
  });

  it("reports color absence", () => {
    expect(formatSummary({ ...autzen, has_color: false })).toContain("color: no");
  });

  it("reports the bbox extent on each axis", () => {
    const out = formatSummary(autzen);
    // longest axis ~1177 across
    expect(out).toMatch(/1[,.]?177(\.\d+)?/);
  });
});
