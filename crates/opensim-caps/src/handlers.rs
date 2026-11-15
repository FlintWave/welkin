// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, MethodRouter},
};
use opensim_llsd::{from_xml, to_xml_string, Llsd, Map};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::event_queue::{EventQueue, EventQueueResponse};

#[derive(Debug, serde::Deserialize)]
pub struct TextureQuery {
    pub texture_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct MeshQuery {
    pub mesh_id: Option<String>,
}

fn llsd_response(llsd: Llsd) -> Response {
    let body = to_xml_string(&llsd);
    (
        StatusCode::OK,
        [("Content-Type", "application/llsd+xml")],
        body,
    )
        .into_response()
}

#[derive(Clone)]
pub struct AssetServiceState {
    pub client: reqwest::Client,
    pub asset_url: String,
}

async fn fetch_asset(state: &AssetServiceState, asset_id: &str) -> Result<Vec<u8>, StatusCode> {
    let url = format!("{}/assets/{asset_id}", state.asset_url);
    let resp = state
        .client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "asset service request failed");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(StatusCode::NOT_FOUND);
    }
    let resp = resp.error_for_status().map_err(|e| {
        tracing::warn!(error = %e, "asset service error");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| {
        tracing::warn!(error = %e, "failed to read asset bytes");
        StatusCode::SERVICE_UNAVAILABLE
    })
}

pub async fn get_texture(
    State(state): State<AssetServiceState>,
    Query(q): Query<TextureQuery>,
) -> Response {
    let Some(texture_id) = q.texture_id.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing texture_id").into_response();
    };
    tracing::debug!(texture_id, "GetTexture");
    match fetch_asset(&state, texture_id).await {
        Ok(data) => (StatusCode::OK, [("Content-Type", "image/x-j2c")], data).into_response(),
        Err(status) => status.into_response(),
    }
}

pub async fn get_mesh(
    State(state): State<AssetServiceState>,
    Query(q): Query<MeshQuery>,
) -> Response {
    let Some(mesh_id) = q.mesh_id.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing mesh_id").into_response();
    };
    tracing::debug!(mesh_id, "GetMesh");
    match fetch_asset(&state, mesh_id).await {
        Ok(data) => (StatusCode::OK, [("Content-Type", "application/vnd.ll.mesh")], data).into_response(),
        Err(status) => status.into_response(),
    }
}

pub async fn get_mesh2(
    State(state): State<AssetServiceState>,
    Query(q): Query<MeshQuery>,
) -> Response {
    let Some(mesh_id) = q.mesh_id.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing mesh_id").into_response();
    };
    tracing::debug!(mesh_id, "GetMesh2");
    match fetch_asset(&state, mesh_id).await {
        Ok(data) => (StatusCode::OK, [("Content-Type", "application/vnd.ll.mesh")], data).into_response(),
        Err(status) => status.into_response(),
    }
}

pub async fn simulator_features() -> Response {
    let mut map = Map::new();
    map.insert("MeshRezEnabled".into(), Llsd::Boolean(true));
    map.insert("MeshUploadEnabled".into(), Llsd::Boolean(true));
    map.insert("PhysicsShapeTypes".into(), Llsd::Boolean(true));
    map.insert("MaxMaterialsPerTransaction".into(), Llsd::Integer(50));
    map.insert("AnimatedObjects".into(), Llsd::Boolean(true));
    map.insert("MaxAgentGroupsBasic".into(), Llsd::Integer(42));
    map.insert("MaxAgentGroupsPremium".into(), Llsd::Integer(60));
    llsd_response(Llsd::Map(map))
}

pub async fn region_info() -> Response {
    let mut map = Map::new();
    map.insert("region_flags".into(), Llsd::Integer(0));
    map.insert("sim_access".into(), Llsd::Integer(13));
    llsd_response(Llsd::Map(map))
}

pub async fn agent_preferences() -> Response {
    llsd_response(agent_preferences_llsd())
}

pub async fn agent_preferences_post(body: Bytes) -> Response {
    match from_xml(&body) {
        Ok(llsd) => tracing::debug!(?llsd, "AgentPreferences POST"),
        Err(e) => tracing::debug!(?e, "AgentPreferences POST: invalid LLSD body"),
    }
    llsd_response(agent_preferences_llsd())
}

fn agent_preferences_llsd() -> Llsd {
    let mut access_prefs = Map::new();
    access_prefs.insert("max".into(), Llsd::String("A".into()));

    let mut map = Map::new();
    map.insert("hover_height".into(), Llsd::Real(0.0));
    map.insert("language".into(), Llsd::String("en".into()));
    map.insert("access_prefs".into(), Llsd::Map(access_prefs));
    Llsd::Map(map)
}

pub async fn map_layer() -> Response {
    llsd_response(Llsd::new_array())
}

pub async fn new_file_agent_inventory(body: Bytes) -> Response {
    log_upload_body("NewFileAgentInventory", &body);
    let new_asset = Uuid::new_v4().to_string();
    let new_item = Uuid::new_v4().to_string();
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(new_asset));
    map.insert("new_inventory_item".into(), Llsd::String(new_item));
    llsd_response(Llsd::Map(map))
}

pub async fn update_notecard_agent_inventory(body: Bytes) -> Response {
    log_upload_body("UpdateNotecardAgentInventory", &body);
    let new_asset = Uuid::new_v4().to_string();
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(new_asset));
    llsd_response(Llsd::Map(map))
}

pub async fn update_script_agent(body: Bytes) -> Response {
    log_upload_body("UpdateScriptAgent", &body);
    let new_asset = Uuid::new_v4().to_string();
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("complete".into()));
    map.insert("new_asset".into(), Llsd::String(new_asset));
    llsd_response(Llsd::Map(map))
}

pub async fn fetch_inventory_descendents2(body: Bytes) -> Response {
    let parsed = from_xml(&body).unwrap_or(Llsd::Undef);
    let folders = match parsed.get("folders") {
        Llsd::Array(arr) => arr.as_slice(),
        _ => &[],
    };

    let mut result_folders = Vec::new();
    for folder in folders {
        let folder_id = folder.get("folder_id").as_uuid();
        let owner_id = folder.get("owner_id").as_uuid();

        let mut entry = Map::new();
        entry.insert("agent_id".into(), Llsd::Uuid(owner_id));
        entry.insert("descendents".into(), Llsd::Integer(0));
        entry.insert("folder_id".into(), Llsd::Uuid(folder_id));
        entry.insert("items".into(), Llsd::Array(Vec::new()));
        entry.insert("categories".into(), Llsd::Array(Vec::new()));
        entry.insert("owner_id".into(), Llsd::Uuid(owner_id));
        entry.insert("version".into(), Llsd::Integer(1));
        result_folders.push(Llsd::Map(entry));
    }

    let mut root = Map::new();
    root.insert("folders".into(), Llsd::Array(result_folders));
    llsd_response(Llsd::Map(root))
}

pub async fn fetch_inventory2(body: Bytes) -> Response {
    let parsed = from_xml(&body).unwrap_or(Llsd::Undef);
    let agent_id = parsed.get("agent_id").as_uuid();

    let mut root = Map::new();
    root.insert("agent_id".into(), Llsd::Uuid(agent_id));
    root.insert("items".into(), Llsd::Array(Vec::new()));
    llsd_response(Llsd::Map(root))
}

pub async fn upload_baked_texture(body: Bytes) -> Response {
    let _ = body;
    let mut root = Map::new();
    root.insert("state".into(), Llsd::String("complete".into()));
    root.insert("new_asset".into(), Llsd::String(Uuid::new_v4().to_string()));
    llsd_response(Llsd::Map(root))
}

fn log_upload_body(cap_name: &str, body: &[u8]) {
    match from_xml(body) {
        Ok(llsd) => {
            let asset_type = llsd.get("asset_type");
            let name = llsd.get("name");
            tracing::debug!(%cap_name, ?asset_type, ?name, "upload request");
        }
        Err(e) => {
            tracing::debug!(%cap_name, ?e, "upload request with invalid LLSD body");
        }
    }
}

pub type EventQueueMap = Arc<RwLock<HashMap<Uuid, Arc<EventQueue>>>>;

pub async fn event_queue_get(
    State(eq_map): State<EventQueueMap>,
    body: Bytes,
) -> Response {
    let agent_id = match from_xml(&body) {
        Ok(llsd) => {
            let id_str = llsd.get("agent-id").as_str().to_string();
            id_str.parse::<Uuid>().ok()
        }
        Err(_) => None,
    };

    let eq = match agent_id {
        Some(id) => {
            let map = eq_map.read().await;
            map.get(&id).cloned()
        }
        None => {
            let map = eq_map.read().await;
            if map.len() == 1 {
                map.values().next().cloned()
            } else {
                None
            }
        }
    };

    let eq = match eq {
        Some(eq) => eq,
        None => {
            tracing::debug!(?agent_id, "EventQueueGet: no queue for agent");
            return StatusCode::NOT_FOUND.into_response();
        }
    };

    match eq.poll(Duration::from_secs(25)).await {
        EventQueueResponse::Events { events, .. } => (
            StatusCode::OK,
            [("content-type", "application/llsd+xml")],
            events,
        )
            .into_response(),
        EventQueueResponse::Timeout => StatusCode::BAD_GATEWAY.into_response(),
    }
}

pub fn cap_routes(asset_state: AssetServiceState) -> Vec<(&'static str, MethodRouter)> {
    vec![
        ("GetTexture", get(get_texture).with_state(asset_state.clone())),
        ("GetMesh", get(get_mesh).with_state(asset_state.clone())),
        ("GetMesh2", get(get_mesh2).with_state(asset_state)),
        ("SimulatorFeatures", get(simulator_features)),
        ("RegionInfo", get(region_info)),
        (
            "AgentPreferences",
            get(agent_preferences).post(agent_preferences_post),
        ),
        ("MapLayer", get(map_layer)),
        ("NewFileAgentInventory", post(new_file_agent_inventory)),
        (
            "UpdateNotecardAgentInventory",
            post(update_notecard_agent_inventory),
        ),
        ("UpdateScriptAgent", post(update_script_agent)),
        (
            "FetchInventoryDescendents2",
            post(fetch_inventory_descendents2),
        ),
        ("FetchInventory2", post(fetch_inventory2)),
        ("UploadBakedTexture", post(upload_baked_texture)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, Router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_asset_state() -> AssetServiceState {
        AssetServiceState {
            client: reqwest::Client::new(),
            asset_url: "http://localhost:99999".to_string(),
        }
    }

    fn test_router() -> Router {
        let mut router = Router::new();
        for (name, method_router) in cap_routes(test_asset_state()) {
            router = router.route(&format!("/{name}"), method_router);
        }
        router
    }

    async fn body_bytes(body: Body) -> Vec<u8> {
        body.collect().await.unwrap().to_bytes().to_vec()
    }

    #[tokio::test]
    async fn test_simulator_features_response() {
        let app = test_router();
        let req = Request::get("/SimulatorFeatures")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let llsd = from_xml(&bytes).unwrap();
        assert_eq!(*llsd.get("MeshRezEnabled"), Llsd::Boolean(true));
        assert_eq!(*llsd.get("MeshUploadEnabled"), Llsd::Boolean(true));
        assert_eq!(*llsd.get("PhysicsShapeTypes"), Llsd::Boolean(true));
        assert_eq!(*llsd.get("MaxMaterialsPerTransaction"), Llsd::Integer(50));
        assert_eq!(*llsd.get("AnimatedObjects"), Llsd::Boolean(true));
        assert_eq!(*llsd.get("MaxAgentGroupsBasic"), Llsd::Integer(42));
        assert_eq!(*llsd.get("MaxAgentGroupsPremium"), Llsd::Integer(60));
    }

    #[tokio::test]
    async fn test_region_info_response() {
        let app = test_router();
        let req = Request::get("/RegionInfo")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let llsd = from_xml(&bytes).unwrap();
        assert_eq!(*llsd.get("region_flags"), Llsd::Integer(0));
        assert_eq!(*llsd.get("sim_access"), Llsd::Integer(13));
    }

    #[tokio::test]
    async fn test_get_texture_service_unavailable() {
        let app = test_router();
        let req = Request::get("/GetTexture?texture_id=00000000-0000-0000-0000-000000000001")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // Asset service at unreachable test URL returns 503
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_get_texture_missing_param() {
        let app = test_router();
        let req = Request::get("/GetTexture")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_upload_returns_complete() {
        let upload_body = to_xml_string(&Llsd::new_map());
        let app = test_router();
        let req = Request::post("/NewFileAgentInventory")
            .header("Content-Type", "application/llsd+xml")
            .body(Body::from(upload_body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let llsd = from_xml(&bytes).unwrap();
        assert_eq!(*llsd.get("state"), Llsd::String("complete".into()));
        assert!(matches!(llsd.get("new_asset"), Llsd::String(s) if !s.is_empty()));
        assert!(matches!(llsd.get("new_inventory_item"), Llsd::String(s) if !s.is_empty()));
    }

    #[tokio::test]
    async fn test_cap_routes_count() {
        let routes = cap_routes(test_asset_state());
        assert_eq!(routes.len(), 10);
    }

    #[tokio::test]
    async fn test_agent_preferences_get() {
        let app = test_router();
        let req = Request::get("/AgentPreferences")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let llsd = from_xml(&bytes).unwrap();
        assert_eq!(*llsd.get("hover_height"), Llsd::Real(0.0));
        assert_eq!(*llsd.get("language"), Llsd::String("en".into()));
        let prefs = llsd.get("access_prefs");
        assert_eq!(*prefs.get("max"), Llsd::String("A".into()));
    }
}
