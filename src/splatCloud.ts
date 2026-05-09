export interface SplatCloud {
  count: number;
  positions: Float32Array;
  colors: Uint8Array;
  normals: Float32Array;
  radii: Float32Array;
}

/// Decode the SoA binary format produced by `pack_splat_cloud` on the Rust side.
export function unpackSplatCloud(buffer: ArrayBuffer): SplatCloud {
  const dv = new DataView(buffer);
  const count = dv.getUint32(0, true);
  let offset = 4;

  const positions = new Float32Array(buffer, offset, count * 3);
  offset += count * 12;

  const colors = new Uint8Array(buffer, offset, count * 4);
  offset += count * 4;

  const normals = new Float32Array(buffer, offset, count * 3);
  offset += count * 12;

  const radii = new Float32Array(buffer, offset, count);

  return { count, positions, colors, normals, radii };
}

/// Min/max along each axis from a packed positions array (length = count*3).
export function bboxOfPositions(
  positions: Float32Array,
): { min: [number, number, number]; max: [number, number, number] } {
  if (positions.length === 0) {
    return { min: [0, 0, 0], max: [0, 0, 0] };
  }
  const min: [number, number, number] = [Infinity, Infinity, Infinity];
  const max: [number, number, number] = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < positions.length; i += 3) {
    for (let a = 0; a < 3; a++) {
      const v = positions[i + a]!;
      if (v < min[a]!) min[a] = v;
      if (v > max[a]!) max[a] = v;
    }
  }
  return { min, max };
}
