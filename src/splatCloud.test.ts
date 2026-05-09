import { describe, expect, it } from "vitest";
import { unpackSplatCloud } from "./splatCloud.ts";

function buildFixtureBuffer(): ArrayBuffer {
  // Mirrors the Rust pack_splat_cloud test: 2 splats.
  const buf = new ArrayBuffer(4 + 24 + 8 + 24 + 8);
  new DataView(buf).setUint32(0, 2, true);
  new Float32Array(buf, 4, 6).set([1, 2, 3, 4, 5, 6]);
  new Uint8Array(buf, 28, 8).set([10, 20, 30, 40, 50, 60, 70, 80]);
  new Float32Array(buf, 36, 6).set([0, 0, 1, 0, 1, 0]);
  new Float32Array(buf, 60, 2).set([0.5, 0.7]);
  return buf;
}

describe("unpackSplatCloud", () => {
  it("recovers count from the header", () => {
    const cloud = unpackSplatCloud(buildFixtureBuffer());
    expect(cloud.count).toBe(2);
  });

  it("returns positions as a Float32Array view", () => {
    const cloud = unpackSplatCloud(buildFixtureBuffer());
    expect(cloud.positions).toBeInstanceOf(Float32Array);
    expect(cloud.positions.length).toBe(6);
    expect(Array.from(cloud.positions)).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it("returns colors as a Uint8Array view", () => {
    const cloud = unpackSplatCloud(buildFixtureBuffer());
    expect(cloud.colors).toBeInstanceOf(Uint8Array);
    expect(Array.from(cloud.colors)).toEqual([10, 20, 30, 40, 50, 60, 70, 80]);
  });

  it("returns normals as a Float32Array view", () => {
    const cloud = unpackSplatCloud(buildFixtureBuffer());
    expect(Array.from(cloud.normals)).toEqual([0, 0, 1, 0, 1, 0]);
  });

  it("returns radii as a Float32Array view", () => {
    const cloud = unpackSplatCloud(buildFixtureBuffer());
    expect(cloud.radii).toBeInstanceOf(Float32Array);
    expect(cloud.radii.length).toBe(2);
    // 0.7 is not exactly representable as f32, so compare with tolerance.
    expect(cloud.radii[0]).toBeCloseTo(0.5, 6);
    expect(cloud.radii[1]).toBeCloseTo(0.7, 6);
  });

  it("handles an empty cloud", () => {
    const buf = new ArrayBuffer(4);
    new DataView(buf).setUint32(0, 0, true);
    const cloud = unpackSplatCloud(buf);
    expect(cloud.count).toBe(0);
    expect(cloud.positions.length).toBe(0);
    expect(cloud.colors.length).toBe(0);
  });
});
