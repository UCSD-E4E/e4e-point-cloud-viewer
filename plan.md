# Implementation Plan

Four releases (one deferred). **MVP** is desktop-only end-to-end (import → bake → view). **v2** gets that same viewer running on phones and on a Quest 2, fed clouds over the LAN or as a sideloaded file — no accounts, no hosting service. **v3** adds head-tracked stereo: phone-in-Cardboard split-screen and a native Quest WebXR session. **v4** is an authenticated hosting backend for sharing beyond the LAN — deferred until LAN/file sharing proves insufficient.

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
- **Output for MVP: a flat in-memory splat buffer**, not the tiled octree. The octree is later work; deferring it keeps MVP scope honest while still committing to splats as the primitive.
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

## v2 — Self-hosted viewer (phone + Quest), no accounts

Goal: someone holding a phone or a Quest 2 can open a cloud the desktop just baked and view it well — in mono and the existing anaglyph mode — without installing anything. The clouds are served by a small `e4epc-server` you run **on your own machine** (a dev box, a Pi, a spare desktop) — not the desktop Tauri app, and not yet an institutional deployment. No login: the link is the access token. The authenticated, containerized, institution-hosted version is **v4**, and is built by hardening this same server, not by replacing it.

Why this shape: the flat splat buffer (deferred octree — see decision 4) means a baked cloud is one self-contained blob the desktop already produces (`pack_splat_cloud` in `e4epc-formats`) and the frontend already decodes (`unpackSplatCloud` in `src/splatCloud.ts`). Getting that blob onto a device is then a transport problem; a tiny HTTP server that holds the PWA bundle and the blobs same-origin is the whole answer, and "run it on my machine" keeps the v2 critical path free of CORS, Docker, OIDC, and a database. The PWA's `?cloud=<url>` entry point doesn't care whether that URL points at `http://192.168.1.x:8080`, a home server reached through a tunnel, or a future `ucsd-e4e.example` — so nothing here forecloses v4.

A `.e4epc` file export is the fallback transport for when there's no server at all (AirDrop / USB / Drive it; open it with the PWA's file picker).

### Architecture (v2)

```
+------------------+   bake → .e4epc   +-------------------------+        +-------------------------+
| Desktop (Tauri)  | ───── POST ─────▶ |  e4epc-server           |  GET   |  Device browser         |
|                  |                   |  (axum, runs on YOUR    | ◀───── |  (phone / Quest)        |
| - bake (existing)|   "Share" button  |   machine — no auth)    |        |                         |
| - "Share" → URL+QR│ ◀── { id, url } ─ |  - serves PWA bundle    |  ── scan QR / type URL ─▶      |
|                  |                   |  - serves /c/<id>.e4epc |        |  PWA bundle + blob, then  |
| (or) "Save .e4epc"│ ─── file ─────────────────────────────────────────▶ |  renders mono / anaglyph  |
+------------------+                   +-------------------------+        +-------------------------+
        one Vite bundle ships in Tauri and on the server: `__TAURI_INTERNALS__` in window picks the path
```

One Vite build. Under Tauri it is the importer + baker + viewer that already exists. Served by `e4epc-server` it is viewer-only: no file picker, no normalize UI, no bake controls — just the renderer and a one-line info bar (cloud name, splat count).

**v2 is plain HTTP.** No TLS, no certs, no tunnel. That buys simplicity and costs the two browser features that require a secure context: offline-via-service-worker and on-device WebXR / iOS head-tracking. Both are deliberately v3 work — v3's M10 puts `e4epc-server` behind TLS and they light up then. Over HTTP on your LAN, v2 still delivers the whole flat viewer — mono and anaglyph, touch navigation, fullscreen — on any phone or Quest that can reach the port. That's the v2 bargain; if it isn't enough, the next step is v3's HTTPS, not a v2 patch.

### M6 — Browser build target + `cloudSource` abstraction

- Introduce `src/cloudSource.ts`: an interface that yields a decoded `SplatCloud`. Implementations: `tauriBakeSource` (wraps the existing `invoke('bake_cloud', …)` flow), `urlSource` (`fetch` an `.e4epc` URL taken from the `?cloud=` query param), and `fileSource` (an `<input type="file">` / drag-and-drop for sideloaded blobs).
- Runtime split in `main.ts`: `if ('__TAURI_INTERNALS__' in window)` → mount the full importer UI; else → mount the viewer-only UI bound to `urlSource` / `fileSource`. Tauri-only imports (`@tauri-apps/api`, `@tauri-apps/plugin-dialog`) move behind a dynamic `import()` on the Tauri branch so the browser bundle tree-shakes them out.
- `renderer.ts`, `splatCloud.ts`, `anaglyph.ts`, and `gamepad.ts` are already Tauri-free; no change needed there.
- Tests: a `cloudSource` test that `urlSource` decodes a fixture blob byte-for-byte into the same `SplatCloud` the desktop path produces; a DOM test that the browser entry mounts the viewer-only layout and the Tauri entry mounts the importer.

**Exit:** `npm run build` produces one bundle; opening `dist/index.html?cloud=/fixtures/autzen.e4epc` in a plain browser renders the baked autzen cloud with working orbit and anaglyph and no Tauri errors in the console.

### M7 — `e4epc-server` (self-hostable, no auth) + `.e4epc` file export

- New `e4epc-server` crate in the workspace: axum + tracing, an on-disk blob directory (path from a CLI flag / env var), and an in-memory or tiny-JSON index of `{ id → filename, size, uploaded }`. Routes:
  - `GET /` + `GET /assets/*` → the PWA static bundle (built by the existing Vite build, embedded with `rust-embed` or served from a directory).
  - `POST /clouds` → write the body to `<blob-dir>/<short-id>.e4epc`, return `{ id, url }`. Wide-open in v2 — the deployment story is "run it where only you / your LAN can reach it" (decision 5); v4 puts this behind auth.
  - `GET /c/:id.e4epc` → stream the blob (or 404). The `?cloud=` value the PWA reads.
  - `GET /healthz` → `200`. Request IDs + structured logs from day one so v4 doesn't have to retrofit them.
- Ships as a normal `cargo run -p e4epc-server -- --blob-dir … --bind 0.0.0.0:8080` binary; a one-paragraph "run it on your machine" section in the README (and a `systemd --user` unit example). No Docker yet — that's v4/B3.
- The Tauri app can *also* serve its last bake directly (a "Serve this cloud" toggle that runs the same axum router in-process) for a zero-setup demo when you don't want a separate process — optional, behind the same code path.
- `save_baked_cloud(path)` Tauri command: write the last bake to a user-chosen `.e4epc` file. The no-server fallback.
- mDNS / `_e4epc._tcp` advertisement (from DESIGN.md) stays **deferred** — a typed URL or a QR covers it; revisit only if "type this URL" is the friction point (decision 6).
- Tests: axum integration tests — `POST /clouds` then `GET /c/:id.e4epc` returns the bytes unchanged; unknown id 404s; `GET /` serves the bundle's `index.html`. A unit test for the share-URL builder. A round-trip test for `save_baked_cloud` ↔ `fileSource`.

**Exit:** `e4epc-server` runs from one command; `curl -F` a baked blob into it and `GET` it back unchanged; the README tells a teammate how to stand it up on their own box in under five minutes.

### M8 — Desktop "Share" command + QR

- Tauri command `share_cloud(server_url)`: POST the last bake to `<server_url>/clouds`, get back `{ id, url }`, build the viewer URL `<server_url>/?cloud=/c/<id>.e4epc`.
- Settings: the `e4epc-server` base URL (default `http://localhost:8080`, with a hint that phones need the machine's LAN address, not `localhost`). Surface the detected LAN IP via `local-ip-address` so the user can copy the right host.
- UI under the rendered cloud: a "Share" button → uploads → shows the URL as text and as a QR code (`qrcode` crate → canvas). A spinner while uploading; a clear error if the server is unreachable (it's *your* server — "is it running?" is the likely cause).
- Tests: a unit test for the URL/QR-payload builder; an integration test that `share_cloud` against a spun-up `e4epc-server` returns a URL that then `GET`s the cloud back.

**Exit:** with `e4epc-server` running on the LAN, "Share" in the desktop app produces a QR; scanning it on a phone on the same wifi loads the viewer and renders that cloud.

### M9 — Phone-friendly UX pass

- Touch input: tune OrbitControls for touch — one-finger orbit, two-finger pan, pinch zoom; bump rotate/zoom speeds off their mouse defaults; suppress the page's pull-to-refresh and double-tap-zoom over the canvas.
- Mobile layout: control bar collapsed to a single ⚙ button by default; prominent fullscreen button; tap-the-canvas toggles the UI overlay; correct `viewport` meta (`viewport-fit=cover`, `user-scalable=no`) so the canvas owns the screen.
- iOS Safari reality check: cap `devicePixelRatio` (≤ 2, likely ≤ 1.5 on phones) to keep fill rate sane; verify `requestFullscreen` behaviour (iOS historically allows it only on `<video>` — may need a CSS "fake fullscreen" fallback that hides Safari chrome); confirm the custom GLSL splat shader compiles on iOS WebGL (it already declares `mediump` — good).
- No service worker / offline support in v2: registration needs a secure context, which the plain-HTTP LAN deployment doesn't have. Asset filenames still get content-hashed now, so the service worker that arrives in v3/M10 has stable URLs to precache; nothing is registered yet. A phone re-loads from `e4epc-server` each session — fine on a LAN.
- Tests: vitest for the layout-mode selector and the UI-overlay toggle logic; a build assertion that asset filenames are content-hashed.

**Exit:** on a mid-range Android phone and on an iPhone, a 1–5M-point cloud loads from `e4epc-server` over plain-HTTP LAN, orbits smoothly at a usable framerate fullscreen.

### v2 ships when

- M6–M9 done.
- `e4epc-server` runs on your machine; a teammate on the same wifi opens a freshly baked cloud on their own phone by scanning the "Share" QR — no app install, no login, no verbal instructions — and can orbit it and flip to anaglyph.
- The same cloud, saved as `.e4epc` and AirDropped, opens via the PWA file picker on a phone that never touched the server.

---

## v3 — Immersive modes (Cardboard stereo + native Quest WebXR)

Goal: upgrade the v2 PWA from "flat view on a device" to "comfortable head-tracked stereo" — split-screen Cardboard on phones (gyro, 3-DOF) and a native WebXR session on Quest 2/3 (controllers, 6-DOF). Same bundle, same `e4epc-server`; this is mostly renderer and input work — plus one piece of plumbing the flat v2 path got away without.

**The HTTPS prerequisite.** WebXR (`immersive-vr`) and `DeviceOrientationEvent.requestPermission()` on iOS both require a *secure context*. `http://localhost` qualifies, but `http://192.168.x.x` does not — so the moment a phone or a Quest (which are never `localhost`) needs these, plain-HTTP-on-the-LAN stops working. Options, cheapest first: (a) a reverse tunnel that terminates TLS for you — `cloudflared tunnel`, `tailscale serve`, or `ngrok` — pointed at `e4epc-server` (also the natural bridge to "share with someone off my LAN"); (b) a self-signed cert on `e4epc-server` with a documented "accept the warning" step on each device (clunky on Quest); (c) bring v4 forward. Recommendation: (a). This is the one new infra task v3 adds, and it lands in M10 as a prerequisite, not buried in a milestone.

The platform split itself is forced by reality: Quest Browser implements WebXR cleanly; iOS Safari does not (and visionOS Safari's WebXR is gated and incomplete — explicitly out of scope). So phones get a hand-rolled stereo path that needs no WebXR; Quest gets the real thing.

### M10 — Device performance baseline + HTTPS for the LAN (spike + plumbing; gates M11 and M12)

- Performance: load the autzen fixture and a representative 5M-point cloud in the v2 PWA on (a) an iPhone, (b) a mid-range Android, (c) Quest Browser in flat mode. Record load time, steady FPS, and whether the tab survives the blob in memory.
- Decide a per-device splat budget and a decimation strategy if one is needed: the flat buffer has no LOD, so "render fewer splats" means subsampling at load (every Nth splat, or a one-time spatial thin) — cheap to add to `cloudSource` as an optional `maxSplats` cap. If a 5M cloud won't hold on iOS, the PWA thins to budget and says so in the info bar.
- HTTPS: stand up the chosen TLS path (a tunnel pointed at `e4epc-server`, per the prerequisite above), document it in the README, and confirm the PWA loads over `https://` on a phone and on the Quest.
- Service worker (now that there's a secure context): register a versioned service worker that precaches the content-hashed PWA assets (hashing was done in v2/M9), so a phone that has loaded once can re-open the same `?cloud=` URL offline (the blob is cacheable too); skip-waiting on each release so a stale cache can't strand a recipient on an old build.
- Output: a short `PERF.md` with the numbers and the chosen budgets; `cloudSource` gains the `maxSplats` cap wired to a per-device default; README has a "serve over HTTPS" section; the SW precache manifest references the hashed assets.

**Exit:** documented FPS and budget numbers for all three device classes; a `maxSplats` cap that visibly thins a cloud; the PWA reachable over HTTPS from a phone and a Quest on the LAN, and re-openable offline after the first load.

### M11 — Cardboard stereo for phones

- Hand-rolled split-screen renderer alongside the existing anaglyph effect: two `PerspectiveCamera`s with an eye-separation offset tuned to the ~5 m normalized scene rather than human IPD (same lesson as the anaglyph `eyeSep = 0.025` fix in `renderer.ts`), each rendered to one half of the canvas via `setViewport` / `setScissor`. An optional barrel-distortion post pass for the lenses — skip it in the first cut of this milestone if it costs too much fill rate.
- Head tracking: `DeviceOrientationEvent` (gyro-only, 3-DOF) drives the camera rig's orientation. iOS requires `DeviceOrientationEvent.requestPermission()` from a user gesture (and the secure context from M10) — surface a clear "Enable head tracking" tap. The rig only rotates; no positional tracking.
- Render-mode toggle in the PWA: mono / anaglyph / cardboard-stereo. Cardboard mode forces fullscreen + landscape and switches to a constrained orbit camera (no fly, capped zoom range) so the user can't get lost.
- Comfort: vignette during fast angular velocity; cap the far plane; lock the cloud at a comfortable apparent size and distance.
- Tests: vitest for the viewport-split math (canvas w/h → two viewport rects), the eye-offset computation, and the gyro → camera-quaternion mapping; a DOM test for the mode-toggle state machine and the iOS permission-gate flow.

**Exit:** dropped into a cheap Cardboard housing, a phone shows a stable, comfortable stereo view of a cloud; turning your head looks around it; no obvious judder or eye strain over a minute.

### M12 — Native Quest 2/3 (WebXR, 6-DOF)

- `renderer.xr.enabled = true`; an "Enter VR" button shown only when `navigator.xr?.isSessionSupported('immersive-vr')` resolves true (and the page is on HTTPS — M10). three.js handles the stereo views and the headset pose; we supply controller input and world placement.
- Place the normalized cloud in room space at a comfortable default (centre ~1.3 m up, ~1.5 m in front, fits in roughly a 1.5 m cube); a "recenter" controller button re-places it relative to the current head pose.
- Controller input: grip-drag to rotate the cloud, thumbstick to translate it (or to fly — pick one for M12, the other is polish), two-handed grip to scale (distance ratio between the controllers), A/X to teleport the cloud-anchor toward the pointed-at splat for inspecting large clouds. Reuse the axis-mapping ideas from `gamepad.ts` where they fit the XR input sources.
- Performance on Quest 2 (Snapdragon XR2, shared 6 GB): stereo plus millions of overlapping instanced quads is the tight spot — apply the M10 budget aggressively here, enable foveated rendering (`renderer.xr.setFoveation`), keep the splat shader at `mediump`, and render the discs opaque with depth-write so there is no per-frame sort (they are disks, not true Gaussians). If 60/72 fps still won't hold at a useful point count, that is the signal to revisit the deferred octree (decision 4) — but only then.
- Tests: vitest for the world-placement math (head pose → cloud transform), the two-handed-scale ratio, and the teleport target pick (ray vs. splat positions); the WebXR session lifecycle is integration-tested by hand on-device against a documented checklist.

**Exit:** in Quest Browser, "Enter VR" drops you into a room with the cloud floating in front of you; you can rotate, move, and scale it with the controllers; it holds a comfortable framerate at the M10 Quest budget.

### v3 ships when

- M10–M12 done.
- The phone-in-Cardboard and Quest paths each demo end-to-end from the HTTPS `e4epc-server` URL with no extra setup beyond the one permission tap (iOS gyro) or "Enter VR" click (Quest).
- The render-mode toggle (mono / anaglyph / cardboard / VR-when-available) is the only UI a device user needs to touch.

---

## v4 — Harden `e4epc-server`: accounts, admin, institutional deploy (deferred)

When self-hosting on a personal machine stops being enough — sharing routinely with people off your network, a persistent gallery, or "this should live on UCSD infra, not Chris's desktop" — `e4epc-server` grows up. It is the *same* crate from M7, not a rewrite: same routes, same blob layout, same PWA bundle. The additions:

- **B1 — Persistence + identity tables.** Move the M7 in-memory/JSON index to `sqlx` (SQLite): `clouds`, `users`, `oauth_state` (OIDC CSRF). `sqlx migrate` from day one of this milestone.
- **B2 — Authentik OIDC.** `POST /clouds` and the admin routes move behind auth. Admin browser: Authorization Code + PKCE → session cookie, OIDC tokens stored server-side keyed by session. Desktop "Share": Device Authorization Grant (RFC 8628) → `user_code` + `verification_uri` shown in-app, poll for tokens, store in `tauri-plugin-stronghold` (OS-keychain-backed). Identity from Authentik claims (`sub`, `email`, `name`); a configured group (e.g. `e4epc-admin`) in the `groups` claim maps to the admin role. Read (`GET /c/:id`) stays unauthenticated — the link is still the token.
- **B3 — Admin portal** at `/admin` in the same bundle: login (OIDC redirect); clouds list (id / owner / filename / size / uploaded / last accessed, hard-delete with a confirmation prompt); users cache (read-only — admin assignment lives in Authentik groups); "my uploads" for non-admins. Periodic disk-usage report on the dashboard (no automatic expiry per spec; the portal is the cleanup mechanism).
- **B4 — Containerize + ghcr + deploy.** Multi-stage Dockerfile (node build → rust build → `debian:slim`, target ≤ 50 MB), volume-mounted blob dir + SQLite file. CI on push to `main` builds + tests + pushes `ghcr.io/ucsd-e4e/e4e-point-cloud-viewer:latest` (`:vX.Y.Z` on tags). The desktop "Share" target URL just changes from your home box to the deployed hostname.

Carry-over risk if/when this is built: all upload and admin functionality depends on Authentik being reachable (reads stay unauthenticated, so existing links keep working) — degrade visibly, not opaquely.

---

## Cross-cutting risks

- **Secure-context wall on the LAN (v3).** WebXR, iOS gyro, and the offline service worker all need HTTPS or `localhost`; a phone/Quest hitting `http://192.168.x.x` gets none of them. v2 is deliberately HTTP-only (flat mono/anaglyph, no offline) and so is unaffected; v3 is gated on it. Resolved in M10 by a TLS-terminating tunnel in front of `e4epc-server` — cheap, and it doubles as the off-LAN sharing bridge.
- **Blob size with no LOD (v2/v3).** A 5M-point cloud is ~100 MB of `.e4epc` (~20 B/splat). Fine on a LAN; heavy on iOS Safari's per-tab memory and on Quest 2's shared RAM, and slow over the internet (a reason v4 is deferred, not just descoped). Mitigation is the M10 budget plus load-time decimation; the escape hatch is reviving the deferred octree (decision 4) if Quest perf forces it.
- **iOS WebGL / fullscreen / gyro-permission quirks (M9, M11).** Mobile Safari is the weakest WebGL target and gates fullscreen and `DeviceOrientationEvent`. Plan for a CSS fake-fullscreen fallback, a `devicePixelRatio` cap, and an explicit permission-tap UI.
- **No WebXR on iOS (M11/M12).** This is why phones get hand-rolled Cardboard stereo and only Quest gets a real `immersive-vr` session. visionOS Safari's WebXR is gated/partial and explicitly out of scope.
- **PWA service-worker invalidation (M10).** A stale cache strands a recipient on an old build. Asset filenames are content-hashed in v2/M9; the versioned service worker that registers them lands in v3/M10 (it needs the secure context that milestone establishes) and skip-waits on each release. v2 over plain HTTP has no offline mode at all, so no stale-cache risk there.
- **`e4epc-server` is unauthenticated in v2 (M7).** Anyone who can reach the port can upload and enumerate-by-guess. Acceptable because "reach the port" means your LAN or behind your tunnel's access controls; do not expose it bare to the internet before v4/B2. The README must say this.
- **Quest 2 fill rate in stereo (M12).** Millions of overlapping instanced quads, twice per frame. Foveation, `mediump`, opaque depth-write (no per-frame sort), and the M10 budget are the levers; octree revival is the last resort.
- **Desktop OIDC device-flow UX (v4/B2).** First-time login forces a context switch to a browser; it needs to be quick and unambiguous or users give up. Not a concern until v4.
- **kNN at 10M points (M3, retroactive).** Unchanged: `kiddo` has held; if real bakes blow past 30 s, swap to voxel-bucketed approximate NN.

## Open decisions

1. **Tauri vs. Electron** — Tauri *(resolved at M0).*
2. **Splat library vs. custom shader** — custom shader *(resolved at M4).*
3. **Recent-files persistence** — Tauri app-data dir *(resolved at M5).*
4. **Tiled octree** — *still deferred.* The flat buffer + `e4epc-server` delivery covers v2/v3. Revisit only if Quest 2 can't hold a useful point count at framerate after the M10 budget and M12's fill-rate mitigations — that is the one scenario that genuinely needs LOD.
5. **Where the cloud server runs (v2)** — `e4epc-server`, a standalone axum binary, **self-hosted on your own machine**, unauthenticated, LAN-reachable; not embedded in the Tauri app (it gets an optional in-process serve mode for demos) and not yet deployed to institutional infra *(resolved this pass; supersedes the earlier "deploy a Docker container with Authentik" v2 plan, which becomes v4 — a hardening of this same crate)*.
6. **mDNS discovery** — *deferred.* Typed URL / QR first; add `_e4epc._tcp` advertisement only if that is the friction point.
7. **HTTPS for devices (v3)** — TLS-terminating reverse tunnel (`cloudflared` / `tailscale serve` / `ngrok`) in front of `e4epc-server`, set up in M10; self-signed certs are the fallback. v2 is plain HTTP on purpose; HTTPS is what unblocks WebXR, iOS gyro, and the offline service worker, all of which are v3 *(resolved this pass)*.
8. **Auth provider (v4)** — Authentik over OIDC, if/when v4 happens.
9. **Hosting target (v4)** — containerized image at `ghcr.io/ucsd-e4e/...` on institutional infra, if/when v4 happens.
10. **Per-device splat budget** — set empirically in M10.

## Deferred optimizations

- **GPU compute for the bake (kNN + PCA normal).** The hot loop in [bake.rs](crates/e4epc-baker/src/bake.rs) — neighbor lookup + 3×3 covariance + eigendecomposition per point — is a textbook GPU workload. With rayon on a 4-core laptop, autzen (110k) bakes in ~2 s, so we are not gated on this. Revisit if real-world bake times for 10M+ point clouds become a UX problem. Implementation path: `wgpu` compute shader (same API as the M4 renderer, so toolchain reuse), keep LAZ decode and kdtree build on CPU, ship the kNN+PCA loop to the GPU. LAZ decode is sequential range-coding and is not GPU-friendly; tree construction is doable on GPU but complex and not where the time goes.
- **Tiled-octree LOD streaming.** Was the original internal format (see DESIGN.md); deferred indefinitely (decision 4). The one trigger to revisit: Quest 2 can't render a useful cloud at framerate even after M10 budgeting and M12's fill-rate mitigations. If revived, the Rust→WASM tile decoder shared with the desktop baker (see "Stack assumptions") is the intended shape.
