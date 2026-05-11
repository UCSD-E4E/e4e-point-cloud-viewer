import { describe, expect, it } from "vitest";
import { type GamepadFrame, applyDeadzone, readOrbitInput } from "./gamepad.ts";

function pad(overrides: { axes?: number[]; buttons?: number[] } = {}): GamepadFrame {
  return {
    axes: overrides.axes ?? [0, 0, 0, 0],
    buttons: (overrides.buttons ?? [0, 0, 0, 0, 0, 0, 0, 0]).map((v) => ({
      value: v,
      pressed: v > 0.5,
    })),
  };
}

describe("applyDeadzone", () => {
  it("treats axis values within the deadzone as exactly zero", () => {
    expect(applyDeadzone(0.05, 0.15)).toBe(0);
    expect(applyDeadzone(-0.1, 0.15)).toBe(0);
    expect(applyDeadzone(0.15, 0.15)).toBe(0);
  });

  it("rescales past-the-deadzone values so the curve starts at 0 (no jump)", () => {
    // Halfway between deadzone and 1 should produce 0.5, not 0.575.
    expect(applyDeadzone(0.575, 0.15)).toBeCloseTo(0.5);
    expect(applyDeadzone(1, 0.15)).toBeCloseTo(1);
    expect(applyDeadzone(-1, 0.15)).toBeCloseTo(-1);
  });
});

describe("readOrbitInput", () => {
  it("returns all-zero deltas for a neutral controller", () => {
    expect(readOrbitInput(pad(), 1 / 60)).toEqual({
      azimuth: 0,
      polar: 0,
      panX: 0,
      panY: 0,
      dolly: 0,
      resetPressed: false,
    });
  });

  it("right-stick X drives azimuth: stick right decreases theta", () => {
    // OrbitControls convention: dragging mouse right rotates the orbit's theta
    // negatively. We match that so stick-right and drag-right feel identical.
    const out = readOrbitInput(pad({ axes: [0, 0, 1, 0] }), 1);
    expect(out.azimuth).toBeLessThan(0);
    expect(out.polar).toBeCloseTo(0);
  });

  it("right-stick Y drives polar: stick down increases phi (camera moves below horizon)", () => {
    const out = readOrbitInput(pad({ axes: [0, 0, 0, 1] }), 1);
    expect(out.polar).toBeGreaterThan(0);
    expect(out.azimuth).toBeCloseTo(0);
  });

  it("left-stick X pans positive (stick right) and stick-up pans positive Y", () => {
    // Browsers report stick-up as a NEGATIVE axes[1]; we flip so panY is
    // positive when the player pushes the stick up.
    expect(readOrbitInput(pad({ axes: [1, 0, 0, 0] }), 1).panX).toBeGreaterThan(0);
    expect(readOrbitInput(pad({ axes: [0, -1, 0, 0] }), 1).panY).toBeGreaterThan(0);
  });

  it("triggers drive dolly: RT zooms in (negative scale), LT zooms out (positive)", () => {
    const rt = readOrbitInput(pad({ buttons: [0, 0, 0, 0, 0, 0, 0, 1] }), 1);
    const lt = readOrbitInput(pad({ buttons: [0, 0, 0, 0, 0, 0, 1, 0] }), 1);
    expect(rt.dolly).toBeLessThan(0);
    expect(lt.dolly).toBeGreaterThan(0);
  });

  it("scales all stick-driven deltas linearly with dt", () => {
    const half = readOrbitInput(pad({ axes: [0.5, 0, 0, 0] }), 1);
    const full = readOrbitInput(pad({ axes: [0.5, 0, 0, 0] }), 2);
    expect(full.panX).toBeCloseTo(half.panX * 2);
  });

  it("button 0 (A on Xbox layout) signals reset-camera intent", () => {
    expect(readOrbitInput(pad({ buttons: [1, 0, 0, 0, 0, 0, 0, 0] }), 1).resetPressed).toBe(true);
    expect(readOrbitInput(pad(), 1).resetPressed).toBe(false);
  });
});
