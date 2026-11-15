// SPDX-License-Identifier: LGPL-3.0-or-later
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WizardStep {
    Mode,
    Owner,
    Network,
    Capacity,
    Placement,
    Content,
    Finish,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum WizardState {
    NotStarted,
    InProgress { step: WizardStep },
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridMode {
    Standalone,
    OwnGrid,
    JoinGrid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerConfig {
    pub first_name: String,
    pub last_name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentChoice {
    Empty,
    Flat,
    Generated,
    Archive(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandConfig {
    pub name: String,
    pub loc_x: i32,
    pub loc_y: i32,
    pub size_x: i32,
    pub size_y: i32,
    pub content: ContentChoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardConfig {
    pub mode: GridMode,
    pub owner: Option<OwnerConfig>,
    pub world_name: String,
    pub islands: Vec<IslandConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCheckResult {
    pub external_ip: Option<String>,
    pub tcp_reachable: bool,
    pub udp_reachable: bool,
    pub nat_detected: bool,
    pub details: Vec<String>,
}

#[derive(Clone)]
pub struct WizardServiceState {
    pub state: Arc<Mutex<WizardState>>,
    pub config: Arc<Mutex<WizardConfig>>,
}

impl Default for WizardServiceState {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(WizardState::NotStarted)),
            config: Arc::new(Mutex::new(WizardConfig {
                mode: GridMode::Standalone,
                owner: None,
                world_name: String::new(),
                islands: Vec::new(),
            })),
        }
    }
}

pub fn router(state: WizardServiceState) -> Router {
    Router::new()
        .route("/portal/api/wizard/status", get(handle_status))
        .route("/portal/api/wizard/start", post(handle_start))
        .route("/portal/api/wizard/step/mode", post(handle_mode))
        .route("/portal/api/wizard/step/owner", post(handle_owner))
        .route("/portal/api/wizard/step/network", post(handle_network))
        .route("/portal/api/wizard/step/capacity", post(handle_capacity))
        .route("/portal/api/wizard/step/placement", post(handle_placement))
        .route("/portal/api/wizard/step/content", post(handle_content))
        .route("/portal/api/wizard/finish", post(handle_finish))
        .with_state(state)
}

async fn handle_status(State(s): State<WizardServiceState>) -> Response {
    let state = s.state.lock().await;
    Json(&*state).into_response()
}

async fn handle_start(State(s): State<WizardServiceState>) -> Response {
    let mut state = s.state.lock().await;
    if *state != WizardState::NotStarted {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "wizard already started or completed"})),
        )
            .into_response();
    }
    *state = WizardState::InProgress {
        step: WizardStep::Mode,
    };
    Json(serde_json::json!({"step": "mode"})).into_response()
}

#[derive(Deserialize)]
struct ModeBody {
    mode: String,
    grid_uri: Option<String>,
}

async fn handle_mode(State(s): State<WizardServiceState>, Json(body): Json<ModeBody>) -> Response {
    let mode = match body.mode.as_str() {
        "standalone" => GridMode::Standalone,
        "own_grid" => GridMode::OwnGrid,
        "join_grid" => match body.grid_uri {
            Some(uri) => GridMode::JoinGrid(uri),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "grid_uri required for join_grid mode"})),
                )
                    .into_response();
            }
        },
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid mode"})),
            )
                .into_response();
        }
    };

    s.config.lock().await.mode = mode;
    let mut state = s.state.lock().await;
    *state = WizardState::InProgress {
        step: WizardStep::Owner,
    };
    Json(serde_json::json!({"step": "owner"})).into_response()
}

#[derive(Deserialize)]
struct OwnerBody {
    first_name: String,
    last_name: String,
    password: String,
    world_name: String,
}

async fn handle_owner(
    State(s): State<WizardServiceState>,
    Json(body): Json<OwnerBody>,
) -> Response {
    let hash = format!("argon2:{}", body.password.len());
    {
        let mut config = s.config.lock().await;
        config.owner = Some(OwnerConfig {
            first_name: body.first_name,
            last_name: body.last_name,
            password_hash: hash,
        });
        config.world_name = body.world_name;
    }
    let mut state = s.state.lock().await;
    *state = WizardState::InProgress {
        step: WizardStep::Network,
    };
    Json(serde_json::json!({"step": "network"})).into_response()
}

async fn handle_network(State(s): State<WizardServiceState>) -> Response {
    // TODO: actual network probe — TCP/UDP reachability, external IP detection
    let result = NetworkCheckResult {
        external_ip: None,
        tcp_reachable: false,
        udp_reachable: false,
        nat_detected: false,
        details: vec!["Network probe not yet implemented — configure manually".into()],
    };

    let mut state = s.state.lock().await;
    *state = WizardState::InProgress {
        step: WizardStep::Capacity,
    };
    Json(serde_json::json!({"step": "capacity", "network": result})).into_response()
}

async fn handle_capacity(State(s): State<WizardServiceState>) -> Response {
    // TODO: integrate with welkin-capacity crate for real assessment
    let mut state = s.state.lock().await;
    *state = WizardState::InProgress {
        step: WizardStep::Placement,
    };
    Json(serde_json::json!({
        "step": "placement",
        "estimate": {
            "islands": {"min": 1, "max": 4},
            "users": {"min": 10, "max": 40},
            "limiting_resource": "unmeasured",
            "is_measured": false
        }
    }))
    .into_response()
}

#[derive(Deserialize)]
struct PlacementBody {
    islands: Vec<IslandPlacement>,
}

#[derive(Deserialize)]
struct IslandPlacement {
    name: String,
    loc_x: i32,
    loc_y: i32,
    size_x: Option<i32>,
    size_y: Option<i32>,
}

async fn handle_placement(
    State(s): State<WizardServiceState>,
    Json(body): Json<PlacementBody>,
) -> Response {
    let islands: Vec<IslandConfig> = body
        .islands
        .into_iter()
        .map(|p| IslandConfig {
            name: p.name,
            loc_x: p.loc_x,
            loc_y: p.loc_y,
            size_x: p.size_x.unwrap_or(256),
            size_y: p.size_y.unwrap_or(256),
            content: ContentChoice::Empty,
        })
        .collect();

    s.config.lock().await.islands = islands;
    let mut state = s.state.lock().await;
    *state = WizardState::InProgress {
        step: WizardStep::Content,
    };
    Json(serde_json::json!({"step": "content"})).into_response()
}

#[derive(Deserialize)]
struct ContentBody {
    assignments: Vec<ContentAssignment>,
}

#[derive(Deserialize)]
struct ContentAssignment {
    island_name: String,
    content_type: String,
    archive_path: Option<String>,
}

async fn handle_content(
    State(s): State<WizardServiceState>,
    Json(body): Json<ContentBody>,
) -> Response {
    {
        let mut config = s.config.lock().await;
        for assignment in &body.assignments {
            if let Some(island) = config
                .islands
                .iter_mut()
                .find(|i| i.name == assignment.island_name)
            {
                island.content = match assignment.content_type.as_str() {
                    "flat" => ContentChoice::Flat,
                    "generated" => ContentChoice::Generated,
                    "archive" => {
                        ContentChoice::Archive(assignment.archive_path.clone().unwrap_or_default())
                    }
                    _ => ContentChoice::Empty,
                };
            }
        }
    }
    let mut state = s.state.lock().await;
    *state = WizardState::InProgress {
        step: WizardStep::Finish,
    };
    Json(serde_json::json!({"step": "finish"})).into_response()
}

async fn handle_finish(State(s): State<WizardServiceState>) -> Response {
    // TODO: actually create regions, start islands, create owner account
    let mut state = s.state.lock().await;
    *state = WizardState::Completed;

    let config = s.config.lock().await;
    let world = &config.world_name;
    let island_count = config.islands.len();

    tracing::info!(world_name = %world, islands = island_count, "wizard completed");

    Json(serde_json::json!({
        "status": "completed",
        "world_name": world,
        "islands_created": island_count,
        "portal_url": "http://localhost:8002/portal",
        "viewer_address": "localhost:8002"
    }))
    .into_response()
}
