//! `e4epc-server`: serves the E4E point cloud viewer PWA bundle and the baked
//! `.e4epc` clouds it renders.
//!
//! Self-hostable and unauthenticated by design (v2): you run it where only you
//! or your LAN can reach it, and the cloud id in the URL acts as a soft
//! capability token. The authenticated, containerized version is v4 — a
//! hardening of this same crate, not a rewrite. See `plan.md`.
//!
//! Routes:
//! - `GET  /healthz`        — liveness, `200 ok`
//! - `POST /clouds[?name=]` — store a packed `.e4epc` body, returns `{ id, url }`
//! - `GET  /c/{id}.e4epc`   — stream a stored blob (the `?cloud=` value the PWA reads)
//! - everything else        — the static PWA bundle (`--static-dir`, the Vite `dist/`)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path as PathParam, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

/// Random id length (base62). ~95 bits — unguessable enough for a soft token.
const ID_LEN: usize = 16;

/// Upload size cap (`axum` defaults to 2 MiB, far too small for a real cloud —
/// 100 MB-plus is normal). 512 MiB ≈ a 16M-splat cloud; past that you want the
/// deferred octree, not a bigger blob.
const MAX_UPLOAD_BYTES: usize = 512 * 1024 * 1024;

/// What we know about a stored cloud. Currently written on upload and not yet
/// read back (the listing endpoint that uses it lands behind auth in v4), but
/// it's the foundation the admin portal joins against, so it's here from day one.
#[derive(Debug, Clone, Serialize)]
pub struct CloudMeta {
    /// Original/display filename if the uploader sent `?name=`, else `{id}.e4epc`.
    pub filename: String,
    pub size: u64,
    pub splat_count: usize,
    pub uploaded_unix_ms: u64,
}

struct Inner {
    blob_dir: PathBuf,
    static_dir: PathBuf,
    index: Mutex<BTreeMap<String, CloudMeta>>,
}

/// Shared server state. Cheap to clone (one `Arc`), as axum requires.
#[derive(Clone)]
pub struct AppState(Arc<Inner>);

impl AppState {
    #[must_use]
    pub fn new(blob_dir: PathBuf, static_dir: PathBuf) -> Self {
        Self(Arc::new(Inner {
            blob_dir,
            static_dir,
            index: Mutex::new(BTreeMap::new()),
        }))
    }

    fn blob_dir(&self) -> &Path {
        &self.0.blob_dir
    }

    fn static_dir(&self) -> &Path {
        &self.0.static_dir
    }

    fn index(&self) -> &Mutex<BTreeMap<String, CloudMeta>> {
        &self.0.index
    }
}

/// Build the router. Split out from `main` so tests can drive it directly.
pub fn app(state: AppState) -> Router {
    let serve_dir = ServeDir::new(state.static_dir());
    Router::new()
        .route("/healthz", get(health))
        .route(
            "/clouds",
            post(upload_cloud).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES)),
        )
        .route("/c/{name}", get(get_cloud))
        .fallback_service(serve_dir)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
        .with_state(state)
}

#[allow(clippy::unused_async)] // axum handlers must be `async fn`.
async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct UploadParams {
    name: Option<String>,
}

#[derive(Serialize)]
struct UploadResponse {
    id: String,
    url: String,
}

async fn upload_cloud(
    State(state): State<AppState>,
    Query(params): Query<UploadParams>,
    body: Bytes,
) -> Result<Json<UploadResponse>, (StatusCode, &'static str)> {
    let splat_count = e4epc_formats::packed_cloud_splat_count(&body)
        .ok_or((StatusCode::BAD_REQUEST, "body is not a valid .e4epc buffer"))?;

    let id = new_id();
    let path = state.blob_dir().join(format!("{id}.e4epc"));
    tokio::fs::write(&path, &body).await.map_err(|err| {
        tracing::error!(%err, path = %path.display(), "failed to write blob");
        (StatusCode::INTERNAL_SERVER_ERROR, "failed to store cloud")
    })?;

    let filename = params.name.unwrap_or_else(|| format!("{id}.e4epc"));
    let size = body.len() as u64;
    tracing::info!(%id, splats = splat_count, bytes = size, "stored cloud");
    state.index().lock().unwrap().insert(
        id.clone(),
        CloudMeta {
            filename,
            size,
            splat_count,
            uploaded_unix_ms: now_unix_ms(),
        },
    );

    Ok(Json(UploadResponse {
        url: format!("/c/{id}.e4epc"),
        id,
    }))
}

async fn get_cloud(
    State(state): State<AppState>,
    PathParam(name): PathParam<String>,
) -> Result<Vec<u8>, StatusCode> {
    if !is_valid_blob_name(&name) {
        return Err(StatusCode::NOT_FOUND);
    }
    match tokio::fs::read(state.blob_dir().join(&name)).await {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!(%err, %name, "failed to read blob");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// A blob filename is `<base62>.e4epc` — alphanumerics only before the
/// extension, so a request can never escape the blob directory.
fn is_valid_blob_name(name: &str) -> bool {
    match name.strip_suffix(".e4epc") {
        Some(stem) => !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_alphanumeric()),
        None => false,
    }
}

fn new_id() -> String {
    rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(ID_LEN)
        .map(char::from)
        .collect()
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::is_valid_blob_name;

    #[test]
    fn valid_blob_names() {
        assert!(is_valid_blob_name("abc123.e4epc"));
        assert!(is_valid_blob_name("ZxQ9aQ2mZ1pK7Vr0.e4epc"));
    }

    #[test]
    fn invalid_blob_names() {
        assert!(!is_valid_blob_name(".e4epc")); // empty stem
        assert!(!is_valid_blob_name("..e4epc")); // stem is "."
        assert!(!is_valid_blob_name("abc.txt")); // wrong extension
        assert!(!is_valid_blob_name("abc")); // no extension
        assert!(!is_valid_blob_name("a/b.e4epc")); // slash
        assert!(!is_valid_blob_name("a..b.e4epc")); // dots in stem
        assert!(!is_valid_blob_name("a b.e4epc")); // space
    }
}
