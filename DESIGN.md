# E4E Point Cloud Viewer

A cross-platform viewer for precomputed point clouds. The user imports clouds on a desktop machine, optionally views them there, and streams them to a head-mounted or phone-based viewer over the local network.

## Goals

- Import LAZ point clouds from arbitrary sources (photogrammetry, iPhone LiDAR, NeRF/3DGS exports, etc.).
- Render the same cloud well on a desktop monitor (anaglyph), an Oculus Quest 2 (stereoscopic VR), and a phone in a Google Cardboard housing.
- Let a viewer device browse every cloud the desktop has shared with it and pick one to load.
- Allow the user to translate, rotate, and scale the cloud while viewing.

## Non-goals (v1)

- Editing or annotating point clouds.
- Running the photogrammetry / NeRF / Gaussian-splatting pipeline. The user brings precomputed clouds.
- True 3D Gaussian Splatting scene import (deferred — see "Splat handling" below).
- Multi-user / collaborative viewing.
- Internet-routable streaming. LAN only for v1.
- Real-time sensor streaming (e.g., live SLAM).

## Users & scenarios

- **Field researcher** captures imagery on an Olympus TG-6, runs photogrammetry offline, and wants to inspect the resulting cloud in VR.
- **iPhone LiDAR user** exports a room scan and views it in stereo on a Quest 2.
- **Demo / outreach** — share a cloud with anyone who has a phone and a Cardboard headset.

## Data model

- **Input format:** LAZ (compressed LAS). Sources produce wildly varying coordinate systems, units, point densities, and orientations.
- **Expected size:** typically 500K–10M points; occasionally larger. Photogrammetry from ~12MP TG-6 stills lands in this range.
- **Internal format:** a Potree-style spatial octree of splat primitives, baked once on the desktop at import time. The octree enables level-of-detail streaming so the Quest never has to hold the full cloud in memory.

### Import normalization

Because clouds come from many sources, the desktop normalizes on import:

- Recenter the cloud's bounding box to the origin.
- Auto-scale to a sane unit range (~1–10 meters across the longest axis).
- Let the user nudge orientation (a 6-DOF gizmo, snap to axes) before baking.

### Splat handling

Every primitive in the internal format is a splat — position, color, opacity, and a 3D covariance — even when the source is a plain point cloud. This gives a single render path on every platform.

For imported point clouds (no covariance information), the baker:

1. Estimates a local normal via PCA on each point's k-nearest neighbors.
2. Sizes the splat from the local point density (mean kNN distance).
3. Orients the splat as a flat disk tangent to the local surface.

This is classical oriented disk splatting. It looks substantially better than raw `GL_POINTS` and degrades gracefully on lower-end hardware (Cardboard) by falling back to isotropic splats or points at coarse LOD.

True 3DGS imports (with optimized covariances and SH coefficients) are a future extension to the same pipeline.

## System architecture

Two deployment shapes from one codebase:

- **Desktop build** = viewer + importer + baker + LAN server.
- **Device build** (Quest 2, Cardboard / phone browser) = viewer only. Discovers desktops on the LAN, lists their shared clouds, downloads on selection, caches locally.

```
+------------------+      LAN       +-------------------+
|  Desktop app     | <------------> |  Device viewer    |
|                  |   discovery    |                   |
|  - Import LAZ    |   list clouds  |  - Browse clouds  |
|  - Normalize     |   download     |  - Cache locally  |
|  - Bake octree   |                |  - Render         |
|  - Serve         |                |                   |
|  - View locally  |                |                   |
+------------------+                +-------------------+
```

## Engine choice

WebXR via **three.js** with a Potree-style LOD layer for octree streaming and a community Gaussian splat renderer for splat primitives.

Reasoning:

- One codebase covers desktop browser, Quest 2 (Quest Browser supports WebXR), and Cardboard (mobile browser + WebXR polyfill).
- Avoids the deprecated Google Cardboard SDK situation that plagues Unity targeting.
- No app-store distribution needed — devices load the viewer by URL.
- `AnaglyphEffect` is built into three.js for the desktop red-cyan mode.
- LAZ parsing on the desktop side via [`laz-perf`](https://github.com/hobuinc/laz-perf) (WASM) or native LASzip if the desktop ships as an Electron/Tauri app.

Tradeoff: peak per-primitive performance is below native Unity. Mitigated by aggressive LOD and the fact that v1 targets ≤10M points.

## Network protocol

- **Discovery:** mDNS / Bonjour. Desktop advertises `_e4epc._tcp` on the LAN; devices browse for it. No configuration in the common case.
- **Pairing fallback:** desktop displays a short alphanumeric code (e.g., `PAIR-7K2X`); user types it on the device. Always works, no camera or network discovery required.
- **QR pairing (optional):** desktop shows a QR encoding the host + token. Quest 2 cannot reliably scan QR codes from inside an app (camera access is restricted), so this is implemented as a *companion-phone scan*: the user scans with their phone, the phone POSTs the pairing token to the desktop, and the desktop authorizes the headset's next connection.
- **Transport:** HTTPS for the cloud listing and download. Octree tiles are individually addressable, so downloads are naturally resumable and cache-friendly.
- **Auth:** per-pairing bearer token. Tokens are revocable from the desktop UI.

## Per-platform viewers

| Platform | Display | Input | Notes |
|---|---|---|---|
| Desktop | Anaglyph (red/cyan) or mono | Mouse + keyboard, gamepad | Combined with the importer UI in the same app. |
| Quest 2 | Stereoscopic, 6-DOF | Touch controllers — translate, rotate-by-grip, scale-by-pinch, teleport for large clouds | WebXR session in Quest Browser. |
| Cardboard | Stereoscopic split-screen | Gaze + magnetic / touch trigger; head movement only (3-DOF) | Lowest-fidelity LOD by default. |

All three platforms share the same scene graph, camera rig abstraction, and cloud loader.

## Open questions

- Exact LOD heuristic for the Cardboard target — what's the largest cloud a mid-range Android phone can comfortably render at 60 FPS in WebXR?
- Should the desktop persist baked octrees on disk (for re-streaming without re-baking), or treat the bake as an in-memory operation per session?
- Token lifetime / pairing UX — one-shot, session-scoped, or persistent across reboots?
- Whether to support a "snapshot view" (single frustum-locked render the device can fall back to if streaming stalls).
