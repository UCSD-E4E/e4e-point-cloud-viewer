export interface CloudSummary {
  point_count: number;
  bbox_min: [number, number, number];
  bbox_max: [number, number, number];
  has_color: boolean;
}

const POINT_FORMATTER = new Intl.NumberFormat("en-US");

export function extentOf(
  min: [number, number, number],
  max: [number, number, number],
): [number, number, number] {
  return [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
}

export function formatExtentLine(
  min: [number, number, number],
  max: [number, number, number],
): string {
  const [dx, dy, dz] = extentOf(min, max);
  return `extent: ${dx.toFixed(2)} × ${dy.toFixed(2)} × ${dz.toFixed(2)}`;
}

export function formatSummary(s: CloudSummary): string {
  return [
    `${POINT_FORMATTER.format(s.point_count)} points`,
    formatExtentLine(s.bbox_min, s.bbox_max),
    `color: ${s.has_color ? "yes" : "no"}`,
  ].join("\n");
}
