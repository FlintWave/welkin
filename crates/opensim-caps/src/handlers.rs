// SPDX-License-Identifier: GPL-3.0-or-later

use axum::{
    body::Bytes,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, MethodRouter},
};
use opensim_llsd::{from_xml, to_xml_string, Llsd, Map};

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

fn llsd_error_response(message: &str) -> Response {
    let mut map = Map::new();
    map.insert("state".into(), Llsd::String("error".into()));
    map.insert("message".into(), Llsd::String(message.into()));
    llsd_response(Llsd::Map(map))
}

pub async fn get_texture(Query(q): Query<TextureQuery>) -> impl IntoResponse {
    tracing::debug!(texture_id = ?q.texture_id, "GetTexture: asset service not wired");
    (StatusCode::NOT_FOUND, "texture not found")
}

pub async fn get_mesh(Query(q): Query<MeshQuery>) -> impl IntoResponse {
    tracing::debug!(mesh_id = ?q.mesh_id, "GetMesh: asset service not wired");
    (StatusCode::NOT_FOUND, "mesh not found")
}

pub async fn get_mesh2(Query(q): Query<MeshQuery>) -> impl IntoResponse {
    tracing::debug!(mesh_id = ?q.mesh_id, "GetMesh2: asset service not wired");
    (StatusCode::NOT_FOUND, "mesh not found")
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
    llsd_error_response("Upload service not implemented")
}

pub async fn update_notecard_agent_inventory(body: Bytes) -> Response {
    log_upload_body("UpdateNotecardAgentInventory", &body);
    llsd_error_response("Upload service not implemented")
}

pub async fn update_script_agent(body: Bytes) -> Response {
    log_upload_body("UpdateScriptAgent", &body);
    llsd_error_response("Upload service not implemented")
}

fn log_upload_body(cap_name: &str, body: &[u8]) {
    match from_xml(body) {
        Ok(llsd) => {
            let asset_type = llsd.get("asset_type");
            let name = llsd.get("name");
            tracing::debug!(%cap_name, ?asset_type, ?name, "upload request (stub)");
        }
        Err(e) => {
            tracing::debug!(%cap_name, ?e, "upload request with invalid LLSD body");
        }
    }
}

pub fn cap_routes() -> Vec<(&'static str, MethodRouter)> {
    vec![
        ("GetTexture", get(get_texture)),
        ("GetMesh", get(get_mesh)),
        ("GetMesh2", get(get_mesh2)),
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, Router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_router() -> Router {
        let mut router = Router::new();
        for (name, method_router) in cap_routes() {
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
    async fn test_get_texture_not_found() {
        let app = test_router();
        let req = Request::get("/GetTexture?texture_id=00000000-0000-0000-0000-000000000001")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_upload_returns_error() {
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
        assert_eq!(*llsd.get("state"), Llsd::String("error".into()));
        assert_eq!(
            *llsd.get("message"),
            Llsd::String("Upload service not implemented".into())
        );
    }

    #[tokio::test]
    async fn test_cap_routes_count() {
        let routes = cap_routes();
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
