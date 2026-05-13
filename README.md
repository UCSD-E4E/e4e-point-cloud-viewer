# E4E Point Cloud Viewer

A cross-platform viewer for precomputed point clouds. The desktop app imports LAZ files, bakes them into oriented-disk splats, and renders them with anaglyph 3D support. Future releases add LAN-streamed Quest 2 and Cardboard viewers.

## Status

**MVP (M0–M5) shipped on desktop.** v2 underway:

- **M6** — browser build target: one bundle that mounts the desktop importer under Tauri and a renderer-only PWA in a plain browser, fed a baked `.e4epc` cloud from `?cloud=<url>` or a file.
- **M7** — [`e4epc-server`](crates/e4epc-server/): a small self-hostable HTTP server that serves the PWA bundle and the baked `.e4epc` blobs (unauthenticated — run it where only you / your LAN can reach it); plus a desktop **Save .e4epc** command for the no-server case.
- *Next:* M8 (desktop "Share" → upload + QR), M9 (phone UX pass).

v3 (Cardboard stereo + native Quest WebXR) and v4 (authenticated hosting — a hardening of `e4epc-server`) are scoped in [plan.md](plan.md) but not started.

## Stack

- **Desktop shell:** [Tauri 2](https://v2.tauri.app/) (Rust core + TypeScript frontend)
- **Viewer:** [three.js](https://threejs.org/) with a custom GLSL splat shader (no off-the-shelf splat library — see DESIGN.md for the rationale)
- **Baker:** Rust + [`las`](https://docs.rs/las) / [`laz`](https://docs.rs/laz) for ingestion, [`kiddo`](https://docs.rs/kiddo) for kNN, [`nalgebra`](https://nalgebra.org/) for PCA normal estimation, [`rayon`](https://docs.rs/rayon) for parallel bake

See [DESIGN.md](DESIGN.md) for the full architecture and [plan.md](plan.md) for the milestone breakdown.

## Running it

### One-time setup

System packages (Ubuntu / Debian — Tauri's webkit/dbus deps):

```bash
sudo apt install -y \
  libwebkit2gtk-4.1-dev libdbus-1-dev libxdo-dev \
  libayatana-appindicator3-dev librsvg2-dev libssl-dev \
  pkg-config build-essential
```

Toolchains: a recent Rust stable (≥ 1.85 for edition 2024) and Node ≥ 20. The repo's `rust-toolchain.toml` pins the channel; for Node we develop against the latest LTS (currently v26 via nvm).

```bash
npm install
```

### Dev

```bash
npm run tauri:dev
```

This starts Vite, compiles the Rust crate in dev mode, and launches the desktop window. An autzen LAZ fixture is bundled at [crates/e4epc-baker/tests/fixtures/autzen_trim.laz](crates/e4epc-baker/tests/fixtures/autzen_trim.laz) for quick testing.

To exercise the browser (PWA) path instead of the desktop shell:

```bash
npm run fixture     # bakes the autzen LAZ → public/fixtures/autzen.e4epc (gitignored)
npm run dev         # then open http://localhost:5173/?cloud=/fixtures/autzen.e4epc
```

`npm run dev` (no Tauri) serves the same `index.html`; without `__TAURI_INTERNALS__` it mounts the renderer-only viewer. `npm run fixture` uses [`crates/e4epc-baker/examples/bake.rs`](crates/e4epc-baker/examples/bake.rs) — the desktop app also writes `.e4epc` files via its **Save .e4epc** button after a bake.

### Production build

```bash
npm run tauri:build
```

### Serving baked clouds to other devices (`e4epc-server`)

[`e4epc-server`](crates/e4epc-server/) is a small standalone HTTP server you run on your own machine: it serves the built PWA bundle and the baked `.e4epc` blobs you upload to it. **It has no authentication** — the cloud id in the URL is a soft capability token, nothing more — so run it where only you or your LAN can reach it, never bare on the internet (that's what v4 is for; see [plan.md](plan.md)).

```bash
npm run build                      # produce dist/ (the PWA bundle the server serves)
npm run serve                      # → http://127.0.0.1:8080 ; blobs in ./clouds/ ; serves ./dist/
# reach it from your phone on the same wifi:
npm run serve -- --bind 0.0.0.0:8080
# (equivalently: cargo run -p e4epc-server -- --bind 0.0.0.0:8080 --blob-dir clouds --static-dir dist)
```

Routes: `GET /healthz` · `POST /clouds[?name=…]` (raw `.e4epc` body → `{ id, url }`) · `GET /c/{id}.e4epc` · everything else from `--static-dir`. `RUST_LOG=tower_http=debug` adds per-request logs.

```bash
# upload a baked cloud, then open the viewer at the printed URL
curl -s --data-binary @autzen.e4epc 'http://127.0.0.1:8080/clouds?name=autzen.e4epc'
# → {"id":"…","url":"/c/….e4epc"}  →  http://127.0.0.1:8080/?cloud=/c/….e4epc
```

Run it as a user service so it survives logout (`~/.config/systemd/user/e4epc-server.service`):

```ini
[Unit]
Description=E4E point cloud viewer server

[Service]
ExecStart=%h/path/to/e4e-point-cloud-viewer/target/release/e4epc-server \
  --bind 0.0.0.0:8080 \
  --blob-dir %h/e4epc-clouds \
  --static-dir %h/path/to/e4e-point-cloud-viewer/dist
Restart=on-failure

[Install]
WantedBy=default.target
```

```bash
cargo build --release -p e4epc-server
systemctl --user daemon-reload && systemctl --user enable --now e4epc-server
```

## Tests

```bash
cargo test --workspace      # Rust: baker, formats, desktop helpers
npm run test:run            # TypeScript: pure-logic helpers via vitest
```

We follow TDD for all behavior — write a failing test, then the minimum implementation. See [`crates/e4epc-baker/src/bake.rs`](crates/e4epc-baker/src/bake.rs) for a worked example. Pure scaffolding (Cargo manifests, Tauri config, etc.) is exempt.

## Layout

```
e4e-point-cloud-viewer/
├── Cargo.toml              # Rust workspace
├── package.json            # frontend deps + Tauri CLI
├── index.html              # entry HTML (importer-root for Tauri, viewer-root for the PWA)
├── public/fixtures/        # generated .e4epc clouds for dev (gitignored; see `npm run fixture`)
├── src/                    # frontend TypeScript
│   ├── main.ts             # entry: detect runtime, dynamic-import importerApp or viewerApp
│   ├── runtime.ts          # isTauriRuntime() — keeps @tauri-apps/* out of the browser bundle
│   ├── importerApp.ts      # desktop flow: open .laz → normalize → bake → view (Tauri IPC)
│   ├── viewerApp.ts        # browser PWA: render a baked .e4epc cloud only
│   ├── cloudSource.ts      # where the bytes come from: ?cloud=<url> or an opened file
│   ├── renderer.ts         # three.js splat renderer + anaglyph + controls
│   ├── splatCloud.ts       # binary .e4epc decoder
│   ├── summary.ts          # source-cloud display helpers
│   └── bake.ts             # bake-result display helpers
├── src-tauri/              # Tauri shell crate
│   ├── src/
│   │   ├── lib.rs          # IPC commands (summarize / normalize / bake / save_baked_cloud / recent)
│   │   └── recent.rs       # recent-files persistence logic
│   └── tauri.conf.json
├── crates/
│   ├── e4epc-baker/        # LAZ → SplatCloud pipeline
│   │   ├── src/{lib,bake,knn,normal,normalize}.rs
│   │   ├── examples/bake.rs    # LAZ → .e4epc dev utility (`npm run fixture`)
│   │   └── tests/          # integration tests + LAZ fixture
│   ├── e4epc-formats/      # shared types + binary .e4epc (de)serialization
│   └── e4epc-server/       # self-hostable HTTP server: serves the PWA + baked .e4epc blobs
│       ├── src/{lib,main}.rs
│       └── tests/api.rs
└── .github/workflows/ci.yml
```

## Pipeline

1. **Open** — file picker → `summarize_cloud` reads LAZ header (count, bbox, color presence)
2. **Normalize** — `compute_normalization` recenters bbox to origin, auto-scales longest axis to 5 m, applies user-chosen up-axis snap and optional Z inversion
3. **Bake** — `bake_cloud` reads all points, builds a kNN index, computes per-point normal (PCA on neighbors) and radius (mean kNN distance × 0.6), packs result as binary
4. **Render** — frontend decodes the binary buffer into typed-array views, uploads as instanced attributes, renders camera-facing billboard quads with per-fragment circle discard

Per-bake telemetry is emitted as `bake-progress` events for the UI progress bar.

## License

BSD 3-Clause. See [LICENSE](LICENSE). Copyright 2026 UC San Diego — Engineers for Exploration.
