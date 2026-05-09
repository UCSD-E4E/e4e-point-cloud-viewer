# Implementation Plan

Three releases. MVP is desktop-only end-to-end (import → bake → view). v2 adds the Quest 2 viewer and the LAN streaming backbone that enables it. v3 adds the Cardboard target.

## Stack assumptions (carried from DESIGN.md)

- **Desktop shell:** Tauri (Rust core + TypeScript frontend).
- **Viewer:** TypeScript on three.js + WebXR.
- **Tile decoder (later):** Rust → WASM, shared with the desktop baker.

The Rust/Tauri choice is the one part of this plan most worth re-evaluating before starting. If the team has no Rust experience, swap to Electron and accept a slower bake; everything else in the plan stays the same.

---

## MVP — Desktop only

Goal: a user can open a `.laz` file in the desktop app, see it baked, and view it in mono and anaglyph modes with mouse/keyboard navigation. No networking, no other devices.

### M0 — Scaffolding

- Tauri project with a TypeScript + Vite frontend.
- Rust workspace: `e4epc-baker`, `e4epc-formats`, `e4epc-desktop` (Tauri commands).
- Lint, format, type-check, test on every push (GitHub Actions or similar).
- Hello-world: button in the UI invokes a Rust command and renders the result.

**Exit:** clean dev loop, CI green, app launches on Linux/macOS/Windows dev machines.

### M1 — LAZ import

- LAZ reader in Rust via the `las` / `laz` crates.
- File picker in UI; on select, Rust reports point count, bounding box, color/intensity presence, source CRS if present.
- Stream points from the file in chunks rather than loading the whole cloud into RAM.

**Exit:** open a 10M-point LAZ from iPhone LiDAR or photogrammetry and see correct stats in the UI within a few seconds.

### M2 — Normalization

- Recenter the bounding box to the origin.
- Auto-scale longest axis to ~5 meters of internal units.
- UI: six "snap to axis" buttons and an "invert Z" toggle for orientation. (Free-rotate gizmo is post-MVP.)

**Exit:** any of three test clouds (TG-6 photogrammetry, iPhone LiDAR, a found-on-the-internet LAZ) lands centered, sanely scaled, and right-side-up after at most one user click.

### M3 — Splat baker

- k-nearest-neighbor index via `kiddo`.
- Per-point PCA on k neighbors → normal estimate → splat orientation.
- Splat size from mean kNN distance.
- **Output for MVP: a flat in-memory splat buffer**, not the tiled octree. The octree is v2 work; deferring it keeps MVP scope honest while still committing to splats as the primitive.
- Run the bake on a Rust thread pool (`rayon`) so the UI stays responsive. Show a progress bar.

**Exit:** 5M-point cloud bakes in under 30 seconds on a typical dev laptop. Output buffer hands cleanly to the renderer.

### M4 — Splat renderer

- three.js scene, perspective camera, splat renderer.
- **Decision needed before starting M4:** which splat renderer? Likely [`@mkkellogg/gaussian-splats-3d`](https://github.com/mkkellogg/GaussianSplats3D) or a thin custom shader that draws each splat as an oriented disk billboard. A custom shader is more work but gives full control over the disk-splat path (which is what we're rendering, not true 3DGS).
- Mouse: orbit, pan, zoom. Keyboard: WASD-style fly. Reset-camera hotkey.

**Exit:** a baked cloud renders at ≥60 FPS on dev hardware at 1080p, navigation feels good, no obvious popping or sorting artifacts.

### M5 — Anaglyph + polish

- three.js `AnaglyphEffect` behind a UI toggle.
- Settings panel: splat size multiplier, background color, point-budget slider.
- Recent-files list. (Full library management is post-MVP.)

**Exit:** anaglyph mode looks correct with red-cyan glasses; the app feels like a tool, not a demo.

### MVP ships when

- All of M0–M5 done.
- Three real test clouds (TG-6, iPhone LiDAR, third-party) all import, bake, and view without intervention beyond orientation snap.
- A teammate who has not seen the app can open a cloud and view it without verbal instructions.

---

## v2 — Quest 2 viewer + LAN streaming

Goal: a Quest 2 user opens the Quest Browser, navigates to the desktop's URL, sees the list of clouds the desktop has baked, picks one, and views it in stereoscopic VR.

This release is mostly about the *backbone* (tile format, server, discovery, pairing), with the Quest viewer as the consumer that proves the backbone works.

### M6 — Tiled octree format

- Split the flat splat buffer from MVP into a Potree-style spatial octree.
- Each tile = a binary blob of splats, addressable by octree path.
- Define the on-wire format. Quantize positions (16-bit per axis within the tile bbox), pack colors, pack covariance basis.
- Rust → WASM decoder for the tile format. Same crate the baker uses to encode.

### M7 — Local LAN server

- Tauri-side `axum` server bound to the LAN interface.
- Endpoints: list clouds, get cloud metadata (octree root + bbox + transform), get tile by path.
- HTTPS with a self-signed cert generated per install (Quest Browser will need a one-time trust prompt).

### M8 — Discovery and pairing

- mDNS advertisement (`_e4epc._tcp`) via `mdns-sd`.
- Short-code pairing UI: desktop displays code, device enters it, desktop issues a bearer token.
- Token revocation UI on the desktop.

### M9 — Quest 2 viewer build

- WebXR session in three.js.
- Touch-controller input: grip-to-rotate, joystick translate, two-handed pinch scale, B/Y to teleport-to-point.
- LOD driven by camera distance + per-frame budget; tiles streamed on demand from the server, cached in IndexedDB.
- Comfort: vignette during translation, snap turn option.

### M10 — Desktop becomes a server *and* a viewer

- Refactor MVP viewer to consume the same tiled-octree pipeline as the Quest viewer (single render path).
- Desktop app ships as a Tauri-wrapped instance of the same WebXR-less viewer build, plus the importer / baker / server UI.

---

## v3 — Cardboard

Goal: anyone with an Android phone and a Cardboard housing can view a shared cloud.

### M11 — Mobile WebXR target

- Same viewer code as v2, with a "Cardboard mode" that uses the WebXR magic-window / inline session and the Cardboard-style stereo split.
- Lowest LOD bracket by default; aggressive point-budget caps.
- Gaze + tap input model. No translation — orbit-around-cloud with head movement only (3-DOF).

### M12 — Mobile-friendly pairing

- QR-on-desktop, scan-with-phone-camera-app, deep-link into the viewer with the pairing token in the URL fragment. No in-app camera access required.

### M13 — Performance pass

- Profile on mid-range Android. Tune tile size, splat count caps, and shader complexity until 60 FPS holds on a representative device.

---

## Cross-cutting risks

- **Splat renderer choice (M4).** Library vs. custom shader is the single biggest unknown for MVP feel and v2 reuse. Worth a one-day spike before committing.
- **Quest Browser HTTPS trust (M7).** The self-signed cert flow may be friction-heavy. Fallback: ship a tiny relay that the Quest connects to via HTTPS while the desktop talks to the relay on LAN. Adds complexity; only do it if the trust prompt is unworkable.
- **kNN at 10M points (M3).** `kiddo` is fast but may need tuning. If bake time blows past 30s, switch to approximate-NN (e.g., voxel-bucketed neighbor search) which is plenty for normal estimation.
- **Tile format churn (M6).** Once the format is shipped to a device, changing it forces re-baking and re-streaming. Version the format from day one.

## Open decisions before starting

1. **Tauri vs. Electron** — confirm before M0.
2. **Splat library vs. custom shader** — defer to start of M4, but keep an eye on it.
3. **Octree partitioning strategy** — Potree's exact scheme, or a simpler regular octree? Defer to M6.
4. **Recent-files persistence** — Tauri's app-data dir, fine. Just confirming it's in MVP.

## Deferred optimizations

- **GPU compute for the bake (kNN + PCA normal).** The hot loop in [bake.rs](crates/e4epc-baker/src/bake.rs) — neighbor lookup + 3×3 covariance + eigendecomposition per point — is a textbook GPU workload. With rayon on a 4-core laptop, autzen (110k) bakes in ~2 s, so we are not gated on this. Revisit if real-world bake times for 10M+ point clouds become a UX problem. Implementation path: `wgpu` compute shader (same API as the M4 renderer, so toolchain reuse), keep LAZ decode and kdtree build on CPU, ship the kNN+PCA loop to the GPU. LAZ decode is sequential range-coding and is not GPU-friendly; tree construction is doable on GPU but complex and not where the time goes.
