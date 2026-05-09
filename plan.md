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

## v2 — Phone PWA + share-via-URL backend

Goal: someone with a phone (no desktop, no app install) views a baked cloud you've shared with them in under 60 seconds. The desktop bakes; a small backend stores and serves; the phone PWA renders. Authentik handles authentication for upload and admin operations; recipients of share URLs don't need accounts.

### Architecture

```
+-------------------+        +-------------------------------+        +-------------------+
| Desktop (Tauri)   |  POST  | Backend container             |  GET   | Phone PWA         |
|                   | -----> |  axum + SQLite + blob storage | <----- |  (renderer-only)  |
| - bake (existing) |        |  - PWA static files           |        |                   |
| - "Share" button  |        |  - upload  (auth required)    |        |  no auth needed:  |
| - QR + URL out    |        |  - serve blobs (open by URL)  |        |  the link is the  |
|                   |        |  - admin portal               |        |  access token     |
+-------------------+        +-------------------------------+        +-------------------+
        ↑                                  ↓
        └────── OIDC device flow ── Authentik (institutional IdP) ── OIDC code flow ──→ admin browser
```

One Docker image. One SQLite database (volume-mounted). Blob storage under `/var/clouds` (volume-mounted). PWA, admin portal, and API all served same-origin so no CORS choreography.

### M6 — Backend skeleton + Docker + ghcr

- New `e4epc-server` crate in the workspace: axum, sqlx (SQLite), tower middleware, tracing.
- Routes (auth wired in M7):
  - `GET /` → PWA static bundle
  - `GET /assets/*`, `GET /admin` → static
  - `POST /clouds` → write blob to `/var/clouds/<short-id>`, return `{ id, url }`
  - `GET /clouds/:id` → stream bytes
- SQLite schema via `sqlx migrate`: `users`, `clouds`, `oauth_state` (CSRF for the OIDC dance).
- Multi-stage Dockerfile: `node` stage builds the PWA, `rust` stage builds the binary, slim `debian:slim` runtime image. Target final image ≤ 50 MB.
- GitHub Action: on push to `main`, build + test, push image to `ghcr.io/ucsd-e4e/e4e-point-cloud-viewer:latest` (and `:vX.Y.Z` on tagged releases).
- Health endpoint, structured logs, request IDs from day one.

### M7 — Authentik OIDC

- **Browser (admin portal)** uses the OIDC Authorization Code flow with PKCE: redirect to Authentik, exchange code at the callback, drop a session cookie, store OIDC tokens server-side keyed by session.
- **Desktop client** uses the OIDC Device Authorization Grant (RFC 8628): desktop calls `/api/auth/device`, gets back a `user_code` + `verification_uri`, displays both to the user, polls for completion. User opens Authentik in any browser, types the code, approves; desktop receives access + refresh tokens and stores them in `tauri-plugin-stronghold` (OS-keychain-backed).
- User identity comes from Authentik claims (`sub`, `email`, `name`); a lightweight `users` row keyed by `sub` is cached for blob-ownership joins.
- Role check: a configured Authentik group (e.g. `e4epc-admin`) maps to the admin role; read from the `groups` claim on every request.
- All write/admin endpoints sit behind an auth middleware that validates the session cookie or bearer access token and attaches the user to the request extension.

### M8 — Admin portal

- Lives at `/admin` in the same PWA bundle (same domain, same auth, same Docker image).
- Pages:
  - **Login** — kicks off the OIDC redirect.
  - **Clouds** — id, owner, original filename, size, uploaded date, last accessed, action: delete.
  - **Users** — local cache of users we've seen log in, with admin status flagged (read-only — admin assignment lives in Authentik groups).
  - **My uploads** — clouds list filtered to the current user, available to non-admins too.
- Delete is hard-delete (row + file). No expiry policy per spec; admins clean up manually.
- Confirmation prompt on every destructive action.

### M9 — PWA build target

- Introduce `src/cloudSource.ts` abstracting "where do bytes come from":
  - Tauri runtime → `invoke('bake_cloud', ...)`
  - Browser runtime → `fetch(urlFromQueryParam)`
- Single Vite build with runtime detection (`if (window.__TAURI_INTERNALS__)`); tree-shaking removes the unused side from the PWA bundle.
- All renderer controls (orientation, render modes, sliders, anaglyph) work in either mode.
- The phone-launched flow (loaded with `?cloud=<id>`) shows only the renderer + a minimal info bar (cloud name, splat count). No file picker, no normalize UI, no bake controls.

### M10 — Desktop "Share" command

- New Tauri command `share_cloud(blob, server_url)` that POSTs to `/clouds` with the user's bearer token, gets back the cloud ID, builds the share URL.
- Desktop UI: "Share" button under the rendered cloud → uploads → displays the URL + a QR code (`qrcode-rs` to canvas).
- Share-server URL configurable in app settings; defaults to the production hostname once it exists.
- "Sign in" button in settings: kicks off the device auth flow, surfaces the user_code, opens the browser to Authentik. Stores tokens in stronghold on success.

### M11 — Phone-friendly UX pass

- Touch gestures tuned for orbit controls (pinch-to-zoom, two-finger pan, one-finger orbit). OrbitControls handles this but defaults need bumping for touch.
- Mobile layout: collapsed control bar by default, fullscreen button prominent, tap-to-toggle UI overlay, viewport meta tweaks.
- Service worker for offline-after-first-load — once a phone has loaded a cloud, it can render it again without network.
- iOS Safari smoke test (different WebGL behavior, different fullscreen permissions).

---

## v3 — Stereo modes + native Quest

Goal: extend the v2 PWA with comfortable stereo viewing on phones, then a higher-fidelity Quest 2 path.

### M12 — Stereo modes for phone (Cardboard)

- Hand-rolled stereo split renderer alongside the existing anaglyph mode. Two cameras with eye-separation offset, rendered to left/right halves of the canvas. Avoids WebXR (iOS support is too patchy to build on).
- Head tracking via `DeviceOrientationEvent` (gyro-only, 3-DOF). iOS requires a permission prompt; surface that clearly.
- Render-mode toggle in the PWA: mono / anaglyph / cardboard-stereo.
- Comfort tuning: vignette during fast head motion, cap render distance, default to a constrained orbit camera that doesn't translate.

### M13 — Native Quest 2

- WebXR session via three.js (`renderer.xr.enabled = true`) — Quest Browser supports it cleanly even though iOS doesn't.
- Touch-controller input: grip-to-rotate, joystick translate, two-handed pinch scale, B/Y to teleport-to-point.
- Reuses the same PWA shell and same backend; no separate build target initially. If perf doesn't hold, then a native build (Unity, or wgpu native) becomes a v4 conversation.

---

## Cross-cutting risks

- **Authentik availability (v2).** All upload + admin functionality depends on Authentik being reachable. Existing share URLs keep working (read is unauthenticated), but new uploads block. Worth a graceful-degradation mode that surfaces this clearly rather than failing opaquely.
- **Blob storage growth (M8).** No automatic expiry per spec; the admin portal is the cleanup mechanism. Add a periodic disk-usage report on the admin dashboard.
- **iOS WebGL / fullscreen quirks (M11, M12).** Mobile Safari has historically been the weakest WebGL platform. May need to drop pixel ratio, simplify the shader, or stub out fullscreen on iOS.
- **PWA service-worker invalidation (M11).** A stale cache on a recipient's phone leaves them stuck on an old build. Need content-hashed asset filenames and a versioned service-worker registration on every release.
- **Desktop OIDC device-flow UX (M7).** First-time login forces a context switch to a browser. Needs to be quick and unambiguous or users will give up.
- **kNN at 10M points (M3, retroactive).** `kiddo` has been fine through MVP. If real-world bake times blow past 30 s, swap to approximate-NN (voxel-bucketed neighbor search), which is plenty for normal estimation.

## Open decisions

1. **Tauri vs. Electron** — Tauri *(resolved at M0).*
2. **Splat library vs. custom shader** — custom shader *(resolved at M4).*
3. **Recent-files persistence** — Tauri app-data dir *(resolved at M5).*
4. **Tiled octree** — *deferred indefinitely.* The flat splat buffer + share-via-URL covers the elevator-pitch use case. Revisit only if blob sizes outgrow what the backend can comfortably stream.
5. **Auth provider** — Authentik over OIDC *(resolved before M7).*
6. **Hosting target** — self-hosted Docker container, image at `ghcr.io/ucsd-e4e/...` *(resolved before M6).*
7. **Blob expiry** — none; manual cleanup via admin portal *(resolved before M8).*
8. **Container registry** — GitHub Container Registry *(resolved before M6).*

## Deferred optimizations

- **GPU compute for the bake (kNN + PCA normal).** The hot loop in [bake.rs](crates/e4epc-baker/src/bake.rs) — neighbor lookup + 3×3 covariance + eigendecomposition per point — is a textbook GPU workload. With rayon on a 4-core laptop, autzen (110k) bakes in ~2 s, so we are not gated on this. Revisit if real-world bake times for 10M+ point clouds become a UX problem. Implementation path: `wgpu` compute shader (same API as the M4 renderer, so toolchain reuse), keep LAZ decode and kdtree build on CPU, ship the kNN+PCA loop to the GPU. LAZ decode is sequential range-coding and is not GPU-friendly; tree construction is doable on GPU but complex and not where the time goes.
