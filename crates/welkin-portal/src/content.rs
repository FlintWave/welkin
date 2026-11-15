// SPDX-License-Identifier: GPL-3.0-or-later
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
struct ImportBody {
    region_id: String,
    source: String,
    path_or_id: String,
    #[serde(default)]
    #[allow(dead_code)]
    options: Option<ImportOptions>,
}

async fn handle_import_start(
    State(s): State<ContentServiceState>,
    Json(body): Json<ImportBody>,
) -> Response {
    // TODO: actually download and load the archive
    let id = uuid::Uuid::new_v4().to_string();
    let status = ImportStatus {
        id: id.clone(),
        state: ImportState::Pending,
        progress_pct: 0.0,
        region_name: body.region_id.clone(),
    };

    tracing::info!(
        import_id = %id,
        region = %body.region_id,
        source = %body.source,
        path = %body.path_or_id,
        "import requested"
    );

    s.imports.lock().await.push(status.clone());

    (StatusCode::ACCEPTED, Json(status)).into_response()
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
struct TerrainApplyBody {
    region_id: String,
    preset: TerrainPreset,
}

async fn handle_terrain_apply(Json(body): Json<TerrainApplyBody>) -> Response {
    // TODO: generate and apply terrain heightmap to region
    tracing::info!(
        region = %body.region_id,
        preset = ?body.preset,
        "terrain apply requested"
    );
    Json(serde_json::json!({"status": "accepted", "region_id": body.region_id})).into_response()
}

async fn handle_backups_list(State(s): State<ContentServiceState>) -> Response {
    let backups = s.backups.lock().await;
    Json(serde_json::json!({"schedules": &*backups})).into_response()
}

#[derive(Deserialize)]
struct BackupCreateBody {
    region_id: String,
    interval_hours: u32,
    retention_count: u32,
}

async fn handle_backups_create(
    State(s): State<ContentServiceState>,
    Json(body): Json<BackupCreateBody>,
) -> Response {
    // TODO: schedule actual OAR backups via region-simulator-core
    let schedule = BackupSchedule {
        region_id: body.region_id,
        interval_hours: body.interval_hours,
        retention_count: body.retention_count,
        last_backup: None,
        next_backup: None,
    };

    s.backups.lock().await.push(schedule.clone());
    (StatusCode::CREATED, Json(schedule)).into_response()
}

async fn handle_backup_now(
    State(s): State<ContentServiceState>,
    Path(region_id): Path<String>,
) -> Response {
    // TODO: trigger immediate OAR backup
    let backups = s.backups.lock().await;
    if backups.iter().any(|b| b.region_id == region_id) {
        tracing::info!(region = %region_id, "immediate backup requested");
        Json(serde_json::json!({"status": "accepted", "region_id": region_id})).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no backup schedule for region"})),
        )
            .into_response()
    }
}
