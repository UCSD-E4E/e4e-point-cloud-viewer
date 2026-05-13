/// Runtime detection: are we inside the Tauri desktop webview, or a plain browser?
///
/// Tauri injects `__TAURI_INTERNALS__` on the global object. A plain browser
/// (the PWA served by `e4epc-server`) doesn't have it. `main.ts` uses this to
/// dynamically import the importer app (Tauri) or the viewer-only app (browser),
/// which keeps the `@tauri-apps/*` packages out of the browser bundle.
export function isTauriRuntime(scope: object = globalThis): boolean {
  return "__TAURI_INTERNALS__" in scope;
}
