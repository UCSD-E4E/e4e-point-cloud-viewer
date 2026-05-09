# E4E Point Cloud Viewer

A cross-platform viewer for precomputed point clouds. The desktop app imports LAZ files, bakes them into oriented-disk splats, and renders them with anaglyph 3D support. Future releases add LAN-streamed Quest 2 and Cardboard viewers.

## Status

**MVP (M0–M5) shipped on desktop.** v2 (Quest 2 + LAN streaming) and v3 (Cardboard) are scoped in [plan.md](plan.md) but not started.

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

This starts Vite, compiles the Rust crate in dev mode, and launches the desktop window. A baked autzen fixture is bundled at [crates/e4epc-baker/tests/fixtures/autzen_trim.laz](crates/e4epc-baker/tests/fixtures/autzen_trim.laz) for quick testing.

### Production build

```bash
npm run tauri:build
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
├── index.html              # entry HTML
├── src/                    # frontend TypeScript
│   ├── main.ts             # UI wiring
│   ├── renderer.ts         # three.js splat renderer + anaglyph + controls
│   ├── splatCloud.ts       # binary IPC decoder
│   ├── summary.ts          # source-cloud display helpers
│   └── bake.ts             # bake-result display helpers
├── src-tauri/              # Tauri shell crate
│   ├── src/
│   │   ├── lib.rs          # IPC commands
│   │   └── recent.rs       # recent-files persistence logic
│   └── tauri.conf.json
├── crates/
│   ├── e4epc-baker/        # LAZ → SplatCloud pipeline
│   │   ├── src/{lib,bake,knn,normal,normalize}.rs
│   │   └── tests/          # integration tests + LAZ fixture
│   └── e4epc-formats/      # shared types + binary serialization
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
