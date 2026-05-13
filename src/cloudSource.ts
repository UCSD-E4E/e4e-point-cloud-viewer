/// Where the bytes of a baked cloud come from.
///
/// The desktop app bakes in-process; the browser PWA gets a packed `.e4epc`
/// buffer either from a URL (`?cloud=…`, served by `e4epc-server`) or from a
/// file the user opens. Both paths end at `unpackSplatCloud`; this module is
/// the thin "fetch / read the bytes, then decode" layer plus its helpers.

import { type SplatCloud, unpackSplatCloud } from "./splatCloud.ts";

/** A decoded cloud plus a human-facing label for the info bar. */
export interface LoadedCloud {
  cloud: SplatCloud;
  name: string;
}

/** The slice of `fetch`'s `Response` we use — narrow so tests can stub it. */
export interface FetchResponse {
  readonly ok: boolean;
  readonly status: number;
  readonly statusText: string;
  arrayBuffer(): Promise<ArrayBuffer>;
}

export type FetchLike = (url: string) => Promise<FetchResponse>;

/** Extract the `?cloud=` query parameter, percent-decoded, or `null` if absent/empty. */
export function parseCloudParam(search: string): string | null {
  const value = new URLSearchParams(search).get("cloud");
  return value !== null && value.length > 0 ? value : null;
}

/** Last path segment of a URL/path, percent-decoded; falls back to the input. */
export function displayNameFromUrl(url: string): string {
  const path = url.split(/[?#]/, 1)[0] ?? url;
  const segment = path.split("/").filter((s) => s.length > 0).pop();
  if (segment === undefined) return url;
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}

/** Fetch a packed `.e4epc` blob from a URL and decode it. */
export async function loadCloudFromUrl(
  url: string,
  fetchImpl: FetchLike = globalThis.fetch,
): Promise<LoadedCloud> {
  const response = await fetchImpl(url);
  if (!response.ok) {
    throw new Error(`fetch ${url} failed: ${response.status} ${response.statusText}`);
  }
  const buffer = await response.arrayBuffer();
  return { cloud: unpackSplatCloud(buffer), name: displayNameFromUrl(url) };
}

/** Read a packed `.e4epc` blob from a `File`/`Blob` and decode it. */
export async function loadCloudFromFile(file: Blob): Promise<LoadedCloud> {
  const buffer = await file.arrayBuffer();
  const name = file instanceof File ? file.name : "(file)";
  return { cloud: unpackSplatCloud(buffer), name };
}
