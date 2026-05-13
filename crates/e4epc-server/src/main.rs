//! `e4epc-server` binary — run it on your own machine to serve the viewer PWA
//! and the baked `.e4epc` clouds your desktop produces. See the crate docs and
//! `plan.md` for the why. No auth (v2): run it where only you / your LAN can
//! reach it.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "e4epc-server",
    version,
    about = "Serves the E4E point cloud viewer PWA and the baked .e4epc clouds it renders."
)]
struct Args {
    /// Address to bind. Use 0.0.0.0:8080 to reach it from other devices on your LAN.
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: SocketAddr,
    /// Directory holding the baked .e4epc blobs (created if it doesn't exist).
    #[arg(long, default_value = "clouds")]
    blob_dir: PathBuf,
    /// Directory holding the built PWA bundle — the Vite `dist/`.
    #[arg(long, default_value = "dist")]
    static_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    std::fs::create_dir_all(&args.blob_dir)?;
    if !args.static_dir.is_dir() {
        tracing::warn!(
            dir = %args.static_dir.display(),
            "static dir not found — run `npm run build`; serving the API only"
        );
    }

    let state = e4epc_server::AppState::new(args.blob_dir.clone(), args.static_dir.clone());
    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    tracing::info!(
        addr = %args.bind,
        blob_dir = %args.blob_dir.display(),
        static_dir = %args.static_dir.display(),
        "e4epc-server listening"
    );
    axum::serve(listener, e4epc_server::app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
