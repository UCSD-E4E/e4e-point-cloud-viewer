import { formatExtentLine } from "./summary.ts";

export interface BakeResult {
  splat_count: number;
  bbox_min: [number, number, number];
  bbox_max: [number, number, number];
  elapsed_ms: number;
}

export interface BakeProgress {
  done: number;
  total: number;
}

const NUM = new Intl.NumberFormat("en-US");

function formatElapsed(ms: number): string {
  return ms >= 1000 ? `${(ms / 1000).toFixed(2)} s` : `${ms} ms`;
}

export function formatBakeResult(r: BakeResult): string {
  return [
    `${NUM.format(r.splat_count)} splats in ${formatElapsed(r.elapsed_ms)}`,
    formatExtentLine(r.bbox_min, r.bbox_max),
  ].join("\n");
}
