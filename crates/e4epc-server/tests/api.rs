//! End-to-end checks against the router: store a blob, get it back unchanged,
//! reject garbage and bad names, serve the static bundle.

use std::path::Path;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use e4epc_formats::{Splat, SplatCloud, pack_splat_cloud};
use e4epc_server::{AppState, app};
use tower::ServiceExt; // `oneshot`

fn packed_cloud(n: usize) -> Vec<u8> {
    pack_splat_cloud(&SplatCloud {
        splats: vec![
            Splat {
                position: [1.0, 2.0, 3.0],
                color: [10, 20, 30, 255],
                normal: [0.0, 0.0, 1.0],
                radius: 0.5,
            };
            n
        ],
        bbox_min: [0.0; 3],
        bbox_max: [0.0; 3],
    })
}

fn state_under(tmp: &Path) -> AppState {
    let blob_dir = tmp.join("clouds");
    let static_dir = tmp.join("static");
    std::fs::create_dir_all(&blob_dir).unwrap();
    std::fs::create_dir_all(&static_dir).unwrap();
    std::fs::write(
        static_dir.join("index.html"),
        b"<!doctype html><title>e4epc viewer</title>",
    )
    .unwrap();
    AppState::new(blob_dir, static_dir)
}

async fn body_of(resp: Response) -> Vec<u8> {
    to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

#[tokio::test]
async fn healthz_is_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let resp = app(state_under(tmp.path()))
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_of(resp).await, b"ok");
}

#[tokio::test]
async fn upload_then_download_round_trips_the_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_under(tmp.path());
    let cloud = packed_cloud(3);

    let resp = app(state.clone())
        .oneshot(
            Request::post("/clouds?name=autzen.e4epc")
                .header("content-type", "application/octet-stream")
                .body(Body::from(cloud.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body_of(resp).await).unwrap();
    let url = json["url"].as_str().unwrap().to_owned();
    assert!(
        url.starts_with("/c/") && url.contains(".e4epc"),
        "url = {url}"
    );
    assert!(!json["id"].as_str().unwrap().is_empty());

    let resp = app(state)
        .oneshot(Request::get(&url).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/octet-stream"),
    );
    assert_eq!(body_of(resp).await, cloud);
}

#[tokio::test]
async fn download_unknown_id_is_404() {
    let tmp = tempfile::tempdir().unwrap();
    let resp = app(state_under(tmp.path()))
        .oneshot(
            Request::get("/c/doesnotexist0000.e4epc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn download_rejects_a_bad_name() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_under(tmp.path());
    for bad in [
        "/c/..e4epc",
        "/c/.e4epc",
        "/c/has%20space.e4epc",
        "/c/wrong.txt",
        "/c/x",
    ] {
        let resp = app(state.clone())
            .oneshot(Request::get(bad).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "for {bad}");
    }
}

#[tokio::test]
async fn upload_rejects_a_malformed_body() {
    let tmp = tempfile::tempdir().unwrap();
    let resp = app(state_under(tmp.path()))
        .oneshot(
            Request::post("/clouds")
                .body(Body::from(vec![1u8, 2, 3])) // too short to be a valid .e4epc
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn root_serves_the_static_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let resp = app(state_under(tmp.path()))
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(body_of(resp).await).unwrap();
    assert!(body.contains("e4epc viewer"), "served: {body}");
}
