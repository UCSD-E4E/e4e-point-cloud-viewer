// Gamepad → orbit-camera input mapping.
//
// Pure / no-three.js so it can be unit-tested directly. The renderer feeds
// raw `Gamepad` state in each rAF tick and applies the returned deltas to
// the OrbitControls state via spherical math.
//
// Conventions (Xbox / standard mapping):
//   axes[0..1] = left stick  (X, Y)         → pan
//   axes[2..3] = right stick (X, Y)         → orbit (azimuth, polar)
//   buttons[0]                               = A — reset camera
//   buttons[6] = LT, buttons[7] = RT         → dolly out / dolly in

export interface GamepadFrameButton {
  value: number;
  pressed: boolean;
}

export interface GamepadFrame {
  axes: ReadonlyArray<number>;
  buttons: ReadonlyArray<GamepadFrameButton>;
}

export interface OrbitInput {
  /** Delta to add to spherical theta. Stick-right → negative (matches mouse drag). */
  azimuth: number;
  /** Delta to add to spherical phi. Stick-down → positive. */
  polar: number;
  /** Pan along camera's right axis, expressed as a fraction of orbit radius. */
  panX: number;
  /** Pan along camera's up axis, expressed as a fraction of orbit radius. */
  panY: number;
  /** Log-scale change to orbit radius: `radius *= exp(dolly)`. RT → negative, LT → positive. */
  dolly: number;
  /** True while the reset-camera button is held. Caller is expected to debounce if needed. */
  resetPressed: boolean;
}

export interface GamepadConfig {
  /** Stick magnitudes <= this snap to 0; values above are rescaled so the post-deadzone curve starts at 0. */
  deadzone: number;
  /** Radians per second at full stick deflection. */
  rotateSpeed: number;
  /** Pan distance, as a fraction of orbit radius, per second at full deflection. */
  panSpeed: number;
  /** ln(zoom-factor) per second at full trigger pull. */
  dollySpeed: number;
}

export const DEFAULT_GAMEPAD_CONFIG: GamepadConfig = {
  deadzone: 0.15,
  rotateSpeed: 2.5,
  panSpeed: 1.5,
  dollySpeed: 1.5,
};

export function applyDeadzone(v: number, deadzone: number): number {
  const mag = Math.abs(v);
  if (mag <= deadzone) return 0;
  const sign = v < 0 ? -1 : 1;
  return sign * ((mag - deadzone) / (1 - deadzone));
}

export function readOrbitInput(
  pad: GamepadFrame,
  dt: number,
  config: GamepadConfig = DEFAULT_GAMEPAD_CONFIG,
): OrbitInput {
  const lx = applyDeadzone(pad.axes[0] ?? 0, config.deadzone);
  const ly = applyDeadzone(pad.axes[1] ?? 0, config.deadzone);
  const rx = applyDeadzone(pad.axes[2] ?? 0, config.deadzone);
  const ry = applyDeadzone(pad.axes[3] ?? 0, config.deadzone);
  const lt = pad.buttons[6]?.value ?? 0;
  const rt = pad.buttons[7]?.value ?? 0;
  // `0 - x` rather than `-x` so neutral sticks return +0 (Object.is treats
  // -0 ≠ 0 and that bites snapshot/equality comparisons in callers and tests).
  return {
    azimuth: 0 - rx * config.rotateSpeed * dt,
    polar: ry * config.rotateSpeed * dt,
    panX: lx * config.panSpeed * dt,
    panY: 0 - ly * config.panSpeed * dt,
    dolly: (lt - rt) * config.dollySpeed * dt,
    resetPressed: pad.buttons[0]?.pressed ?? false,
  };
}
