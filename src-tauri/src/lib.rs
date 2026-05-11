mod recent;

use std::path::PathBuf;

use e4epc_baker::Summary;
use e4epc_baker::normalize::{
    UpAxis, auto_normalize, transformed_bbox, with_up_axis, with_z_inverted,
};
use e4epc_formats::{Normalization, pack_splat_cloud};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager};

const TARGET_LONGEST_EXTENT_M: f64 = 5.0;
const BAKE_K: usize = 12;
const RECENT_MAX: usize = 10;
const RECENT_FILE: &str = "recent.json";

fn recent_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(RECENT_FILE))
}

#[tauri::command]
fn get_recent_files(app: AppHandle) -> Result<Vec<String>, String> {
    let path = recent_path(&app)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tauri::command]
fn add_recent_file(app: AppHandle, path: String) -> Result<Vec<String>, String> {
    let json_path = recent_path(&app)?;
    if let Some(dir) = json_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let existing: Vec<String> = if json_path.exists() {
        let bytes = std::fs::read(&json_path).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).unwrap_or_default()
    } else {
        vec![]
    };
    let updated = recent::add_recent(existing, path, RECENT_MAX);
    let bytes = serde_json::to_vec(&updated).map_err(|e| e.to_string())?;
    std::fs::write(&json_path, bytes).map_err(|e| e.to_string())?;
    Ok(updated)
}

#[tauri::command]
fn summarize_cloud(path: PathBuf) -> Result<Summary, String> {
    e4epc_baker::summarize_laz(path).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct NormalizationResult {
    normalization: Normalization,
    normalized_bbox_min: [f64; 3],
    normalized_bbox_max: [f64; 3],
}

#[tauri::command]
fn compute_normalization(summary: Summary, up_axis: UpAxis, invert_z: bool) -> NormalizationResult {
    let mut n = auto_normalize(&summary, TARGET_LONGEST_EXTENT_M);
    n = with_up_axis(n, up_axis);
    if invert_z {
        n = with_z_inverted(n);
    }
    let (normalized_bbox_min, normalized_bbox_max) = transformed_bbox(&summary, &n);
    NormalizationResult {
        normalization: n,
        normalized_bbox_min,
        normalized_bbox_max,
    }
}

#[derive(Serialize, Clone)]
struct BakeProgress {
    done: usize,
    total: usize,
}

#[tauri::command]
async fn bake_cloud(
    app: AppHandle,
    path: PathBuf,
    summary: Summary,
    up_axis: UpAxis,
    invert_z: bool,
) -> Result<Response, String> {
    let mut n = auto_normalize(&summary, TARGET_LONGEST_EXTENT_M);
    n = with_up_axis(n, up_axis);
    if invert_z {
        n = with_z_inverted(n);
    }

    // The bake is CPU-bound (LAZ decode + kNN + per-point PCA). Run it on the
    // blocking thread pool so the async runtime and the main thread stay free
    // to dispatch `bake-progress` events to the webview.
    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let iter = e4epc_baker::iter_points(&path).map_err(|e| e.to_string())?;
        let raw = iter
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let app_for_progress = app.clone();
        let cloud = e4epc_baker::bake_cloud_from_points_with_progress(
            raw,
            &n,
            BAKE_K,
            move |done, total| {
                let _ = app_for_progress.emit("bake-progress", BakeProgress { done, total });
            },
        );

        Ok(pack_splat_cloud(&cloud))
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(Response::new(bytes))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            summarize_cloud,
            compute_normalization,
            bake_cloud,
            get_recent_files,
            add_recent_file,
        ])
        .run(tauri::generate_context!())
        .expect("tauri runtime failed to start");
}
