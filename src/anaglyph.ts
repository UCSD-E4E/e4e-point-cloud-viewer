// Color combination matrices for the red/cyan AnaglyphEffect, shaped for
// THREE.Matrix3.fromArray (column-major, 9 elements).
//
// - dubois     : minimizes retinal rivalry; washes color out of cyan/teal
//                scenes (the three.js default).
// - half-color : left-eye luma → red; right-eye G/B passed through. Best
//                trade-off for color-rich content with little red.
// - full-color : pure channel split (R from left, G/B from right). Best
//                color fidelity, more retinal rivalry on saturated reds.

export type AnaglyphMode = "dubois" | "half-color" | "full-color";

export interface AnaglyphMatrices {
  left: number[];
  right: number[];
}

const DUBOIS: AnaglyphMatrices = {
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
};

const HALF_COLOR: AnaglyphMatrices = {
  left: [0.299, 0, 0, 0.587, 0, 0, 0.114, 0, 0],
  right: [0, 0, 0, 0, 1, 0, 0, 0, 1],
};

const FULL_COLOR: AnaglyphMatrices = {
  left: [1, 0, 0, 0, 0, 0, 0, 0, 0],
  right: [0, 0, 0, 0, 1, 0, 0, 0, 1],
};

export function anaglyphMatricesFor(mode: AnaglyphMode): AnaglyphMatrices {
  switch (mode) {
    case "dubois":
      return DUBOIS;
    case "half-color":
      return HALF_COLOR;
    case "full-color":
      return FULL_COLOR;
  }
}
