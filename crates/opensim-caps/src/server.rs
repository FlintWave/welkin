// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use opensim_llsd::{from_xml, to_xml_string, Llsd, Map};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::event_queue::EventQueue;

pub struct AgentCaps {
    pub agent_id: Uuid,
    pub seed_path: String,
    pub cap_urls: HashMap<String, String>,
    pub event_queue: EventQueue,
}

pub struct CapabilityServer {
    agents: Arc<RwLock<HashMap<Uuid, AgentCaps>>>,
    router: Arc<RwLock<Router>>,
}

impl CapabilityServer {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            router: Arc::new(RwLock::new(Router::new())),
        }
    }

    pub async fn register_agent(&self, agent_id: Uuid) -> String {
        let seed_path = format!("/{}", Uuid::new_v4());
        let agent_caps = AgentCaps {
            agent_id,
            seed_path: seed_path.clone(),
            cap_urls: HashMap::new(),
            event_queue: EventQueue::new(),
        };

        self.agents.write().await.insert(agent_id, agent_caps);

        let agents = Arc::clone(&self.agents);
        let captured_agent_id = agent_id;

        let handler = move |body: Bytes| {
            let agents = Arc::clone(&agents);
            async move {
                seed_cap_handler(agents, captured_agent_id, body).await
            }
        };

        let route = Router::new().route(&seed_path, post(handler));
        let mut router = self.router.write().await;
        *router = std::mem::take(&mut *router).merge(route);

        seed_path
    }

    pub async fn unregister_agent(&self, agent_id: Uuid) {
        self.agents.write().await.remove(&agent_id);
    }

    pub async fn register_cap<H, T>(&self, agent_id: Uuid, name: &str, handler: H) -> Option<String>
    where
        H: axum::handler::Handler<T, ()> + Clone + Send + 'static,
        T: 'static,
    {
        let cap_path = format!("/{}", Uuid::new_v4());
        {
            let mut agents = self.agents.write().await;
            let agent = agents.get_mut(&agent_id)?;
            agent.cap_urls.insert(name.to_string(), cap_path.clone());
        }

        let route = Router::new().route(&cap_path, post(handler));
        let mut router = self.router.write().await;
        *router = std::mem::take(&mut *router).merge(route);

        Some(cap_path)
    }

    pub async fn get_cap_url(&self, agent_id: Uuid, name: &str) -> Option<String> {
        let agents = self.agents.read().await;
        agents
            .get(&agent_id)?
            .cap_urls
            .get(name)
            .cloned()
    }

    pub fn into_router(self) -> Router {
        Arc::try_unwrap(self.router)
            .expect("router has multiple references")
            .into_inner()
    }

    pub async fn router(&self) -> Router {
        self.router.read().await.clone()
    }
}

impl Default for CapabilityServer {
    fn default() -> Self {
        Self::new()
    }
}

async fn seed_cap_handler(
    agents: Arc<RwLock<HashMap<Uuid, AgentCaps>>>,
    agent_id: Uuid,
    body: Bytes,
) -> impl IntoResponse {
    let requested = match from_xml(&body) {
        Ok(llsd) => llsd,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                "invalid LLSD".to_string(),
            )
                .into_response();
        }
    };

    let requested_names: Vec<String> = match &requested {
        Llsd::Array(arr) => arr.iter().map(|v| v.as_str().to_string()).collect(),
        _ => Vec::new(),
    };

    let agents = agents.read().await;
    let mut response_map = Map::new();

    if let Some(agent) = agents.get(&agent_id) {
        for name in &requested_names {
            if let Some(url) = agent.cap_urls.get(name) {
                response_map.insert(name.clone(), Llsd::String(url.clone()));
            }
        }
    }

    let xml = to_xml_string(&Llsd::Map(response_map));
    (
        StatusCode::OK,
        [("content-type", "application/llsd+xml")],
        xml,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn body_bytes(body: Body) -> Vec<u8> {
        body.collect().await.unwrap().to_bytes().to_vec()
    }

    #[tokio::test]
    async fn test_register_agent() {
        let server = CapabilityServer::new();
        let agent_id = Uuid::new_v4();
        let seed_path = server.register_agent(agent_id).await;
        assert!(seed_path.starts_with('/'));
        assert_eq!(seed_path.len(), 37); // /UUID
    }

    #[tokio::test]
    async fn test_seed_cap_returns_registered_caps() {
        let server = CapabilityServer::new();
        let agent_id = Uuid::new_v4();
        let seed_path = server.register_agent(agent_id).await;

        let dummy_handler = || async { "ok" };
        server.register_cap(agent_id, "FetchInventory2", dummy_handler.clone()).await;
        server.register_cap(agent_id, "GetTexture", dummy_handler).await;

        let fetch_url = server.get_cap_url(agent_id, "FetchInventory2").await.unwrap();
        let texture_url = server.get_cap_url(agent_id, "GetTexture").await.unwrap();

        let request_llsd = Llsd::Array(vec![
            Llsd::String("FetchInventory2".into()),
            Llsd::String("GetTexture".into()),
            Llsd::String("NonExistent".into()),
        ]);
        let request_body = to_xml_string(&request_llsd);

        let router = server.router().await;
        let req = axum::http::Request::builder()
            .method("POST")
            .uri(&seed_path)
            .header("content-type", "application/llsd+xml")
            .body(Body::from(request_body))
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = body_bytes(resp.into_body()).await;
        let parsed = from_xml(&bytes).unwrap();

        assert_eq!(parsed.get("FetchInventory2").as_str(), &fetch_url);
        assert_eq!(parsed.get("GetTexture").as_str(), &texture_url);
        assert!(parsed.get("NonExistent").is_undef());
    }

    #[tokio::test]
    async fn test_unregister_agent() {
        let server = CapabilityServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id).await;

        let dummy_handler = || async { "ok" };
        server.register_cap(agent_id, "TestCap", dummy_handler).await;

        server.unregister_agent(agent_id).await;
        assert!(server.get_cap_url(agent_id, "TestCap").await.is_none());
    }

    #[tokio::test]
    async fn test_register_cap() {
        let server = CapabilityServer::new();
        let agent_id = Uuid::new_v4();
        server.register_agent(agent_id).await;

        let dummy_handler = || async { "inventory response" };
        let cap_path = server
            .register_cap(agent_id, "FetchInventory2", dummy_handler)
            .await
            .unwrap();

        assert!(cap_path.starts_with('/'));
        let stored = server.get_cap_url(agent_id, "FetchInventory2").await.unwrap();
        assert_eq!(cap_path, stored);
    }

    #[tokio::test]
    async fn test_multiple_agents() {
        let server = CapabilityServer::new();
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();

        let seed_a = server.register_agent(agent_a).await;
        let seed_b = server.register_agent(agent_b).await;
        assert_ne!(seed_a, seed_b);

        let handler_a = || async { "a" };
        let handler_b = || async { "b" };
        server.register_cap(agent_a, "GetMesh", handler_a).await;
        server.register_cap(agent_b, "GetMesh", handler_b).await;

        let url_a = server.get_cap_url(agent_a, "GetMesh").await.unwrap();
        let url_b = server.get_cap_url(agent_b, "GetMesh").await.unwrap();
        assert_ne!(url_a, url_b);

        assert!(server.get_cap_url(agent_a, "GetMesh").await.is_some());
        assert!(server.get_cap_url(agent_b, "GetMesh").await.is_some());
        assert!(server.get_cap_url(agent_a, "NoSuchCap").await.is_none());
    }
}
