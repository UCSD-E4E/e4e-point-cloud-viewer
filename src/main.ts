/// Entry point. Picks which app to mount based on the runtime, then dynamically
/// imports it — so the browser bundle never pulls in the `@tauri-apps/*`
/// packages the importer needs, and the Tauri build never pulls in nothing it
/// doesn't. See `runtime.ts` for the detection.

import { isTauriRuntime } from "./runtime.ts";

const importerRoot = document.getElementById("importer-root");
const viewerRoot = document.getElementById("viewer-root");

if (isTauriRuntime()) {
  viewerRoot?.remove();
  void import("./importerApp.ts").then((m) => {
    m.mountImporter();
  });
} else {
  importerRoot?.remove();
  void import("./viewerApp.ts").then((m) => {
    m.mountViewer();
  });
}
