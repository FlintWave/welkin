// SPDX-License-Identifier: LGPL-3.0-or-later
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenceInfo {
    pub name: String,
    pub url: String,
    pub verified: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogueEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub download_url: String,
    pub licence: LicenceInfo,
    pub size_bytes: u64,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ImportOptions {
    #[serde(default)]
    pub merge: bool,
    pub displacement: Option<(f32, f32, f32)>,
    pub rotation: Option<f32>,
    #[serde(default)]
    pub skip_assets: bool,
    pub default_owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportState {
    Pending,
    Downloading,
    Loading,
    Complete,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStatus {
    pub id: String,
    pub state: ImportState,
    pub progress_pct: f32,
    pub region_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainPreset {
    Flat,
    Island,
    Archipelago,
    Mountain,
    Valley,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSchedule {
    pub region_id: String,
    pub interval_hours: u32,
    pub retention_count: u32,
    pub last_backup: Option<String>,
    pub next_backup: Option<String>,
}

#[derive(Clone)]
pub struct ContentServiceState {
    pub catalogue: Arc<Mutex<Vec<CatalogueEntry>>>,
    pub imports: Arc<Mutex<Vec<ImportStatus>>>,
    pub backups: Arc<Mutex<Vec<BackupSchedule>>>,
}

impl Default for ContentServiceState {
    fn default() -> Self {
        Self {
            catalogue: Arc::new(Mutex::new(Vec::new())),
            imports: Arc::new(Mutex::new(Vec::new())),
            backups: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

pub fn router(state: ContentServiceState) -> Router {
    Router::new()
        .route("/portal/api/content/catalogue", get(handle_catalogue))
        .route("/portal/api/content/import", post(handle_import_start))
        .route("/portal/api/content/import/{id}", get(handle_import_status))
        .route(
            "/portal/api/content/terrain/presets",
            get(handle_terrain_presets),
        )
        .route(
            "/portal/api/content/terrain/apply",
            post(handle_terrain_apply),
        )
        .route(
            "/portal/api/content/backups",
            get(handle_backups_list).post(handle_backups_create),
        )
        .route(
            "/portal/api/content/backups/{region_id}/now",
            post(handle_backup_now),
        )
        .with_state(state)
}

async fn handle_catalogue(State(s): State<ContentServiceState>) -> Response {
    let entries = s.catalogue.lock().await;
    let verified: Vec<&CatalogueEntry> = entries.iter().filter(|e| e.verified).collect();
    Json(serde_json::json!({"entries": verified})).into_response()
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ImportBody {
    region_id: String,
    source: String,
    path_or_id: String,
    #[serde(default)]
    #[allow(dead_code)]
    options: Option<ImportOptions>,
}

async fn handle_import_start(
    State(_s): State<ContentServiceState>,
    Json(_body): Json<ImportBody>,
) -> Response {
    return crate::error::not_implemented("importing and loading archives is not implemented yet");
}

async fn handle_import_status(
    State(s): State<ContentServiceState>,
    Path(id): Path<String>,
) -> Response {
    let imports = s.imports.lock().await;
    match imports.iter().find(|i| i.id == id) {
        Some(status) => Json(status).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_terrain_presets() -> Response {
    Json(serde_json::json!({
        "presets": [
            {"name": "flat", "description": "Flat terrain at sea level"},
            {"name": "island", "description": "Single island with sandy beaches"},
            {"name": "archipelago", "description": "Multiple small islands"},
            {"name": "mountain", "description": "Mountainous terrain with valleys"},
            {"name": "valley", "description": "Low valley surrounded by hills"}
        ]
    }))
    .into_response()
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct TerrainApplyBody {
    region_id: String,
    preset: TerrainPreset,
}

async fn handle_terrain_apply(Json(_body): Json<TerrainApplyBody>) -> Response {
    return crate::error::not_implemented("generating and applying terrain is not implemented yet");
}

async fn handle_backups_list(State(s): State<ContentServiceState>) -> Response {
    let backups = s.backups.lock().await;
    Json(serde_json::json!({"schedules": &*backups})).into_response()
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct BackupCreateBody {
    region_id: String,
    interval_hours: u32,
    retention_count: u32,
}

async fn handle_backups_create(
    State(_s): State<ContentServiceState>,
    Json(_body): Json<BackupCreateBody>,
) -> Response {
    return crate::error::not_implemented("scheduling backups is not implemented yet");
}

async fn handle_backup_now(
    State(_s): State<ContentServiceState>,
    Path(_region_id): Path<String>,
) -> Response {
    return crate::error::not_implemented("triggering immediate backups is not implemented yet");
}

