// SPDX-License-Identifier: LGPL-3.0-or-later

//! Inbound CreateAgent handling for the gatekeeper.
//!
//! Exposes an axum route that accepts the form-encoded `CreateAgent` POST used
//! by the Hypergrid Gandalf handshake, validates basic fields, performs
//! impersonation/verification guards (VerifyAgent/VerifyClient emulation via an
//! optional in-process UserAgent service), keeps a record of the visiting
//! foreign agent, and returns an (empty) seed-capability LLSD response.  Full
//! simulation co-validation (Presence creation, SimulationService.CreateAgent)
//! remains beyond this layer.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use uuid::Uuid;
use welkin_data::accounts::UserAccountStore;
use welkin_services::hypergrid::UserAgentService;
use welkin_services::user_agent::UserAgentServiceImpl;

// ---------------------------------------------------------------------------
// State and records
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ForeignAgentRecord {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub secure_session_id: Option<Uuid>,
    pub destination_uuid: Option<Uuid>,
    pub region_handle: Option<u64>,
    pub client_ip: String,
    pub viewer: String,
}

#[derive(Clone)]
pub struct CreateAgentState {
    pub records: Arc<RwLock<HashMap<Uuid, ForeignAgentRecord>>>,
    /// Optional accounts store used for UUID-impersonation checks.  When
    /// absent, impersonation checks are skipped (emulation/unavailable).
    pub accounts_store: Option<Arc<UserAccountStore>>,
    /// Optional home-grid UserAgent service used to emulate VerifyAgent /
    /// VerifyClient callbacks.  When `None`, callback verification is skipped.
    pub user_agent: Option<Arc<UserAgentServiceImpl>>,
    /// This grid's external (gatekeeper) name, used for coming-home checks.
    pub grid_external_name: String,
}

impl Default for CreateAgentState {
    fn default() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            accounts_store: None,
            user_agent: None,
            grid_external_name: String::new(),
        }
    }
}

impl CreateAgentState {
    #[allow(dead_code)]
    pub fn with_user_agent(mut self, ua: Arc<UserAgentServiceImpl>) -> Self {
        self.user_agent = Some(ua);
        self
    }

    #[allow(dead_code)]
    pub fn with_accounts_store(mut self, store: Arc<UserAccountStore>) -> Self {
        self.accounts_store = Some(store);
        self
    }
}

// ---------------------------------------------------------------------------
// Minimal CreateAgent service processing
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CreateAgentService {
    pub state: CreateAgentState,
}

#[allow(dead_code)]
impl CreateAgentService {
    pub fn new(state: CreateAgentState) -> Self {
        Self { state }
    }

    pub fn with_state(state: CreateAgentState) -> Self {
        Self::new(state)
    }

    pub async fn process(&self, body: Bytes) -> Response {
        handle_create_agent_inner(&self.state, body).await
    }

    pub async fn handle(&self, body: Bytes) -> Response {
        self.process(body).await
    }
}

pub fn router(state: CreateAgentState) -> Router {
    Router::new()
        .route("/create_agent", post(handle_create_agent))
        .route("/", post(handle_create_agent))
        .with_state(state)
}

async fn handle_create_agent(State(state): State<CreateAgentState>, body: Bytes) -> Response {
    handle_create_agent_inner(&state, body).await
}

/// Core CreateAgent processing, reusable by [`CreateAgentService`].
pub async fn handle_create_agent_inner(state: &CreateAgentState, body: Bytes) -> Response {
    // Handle malformed bodies (including malformed XML-RPC) without panicking.
    let text = match String::from_utf8(body.to_vec()) {
        Ok(t) => t,
        Err(_) => {
            tracing::warn!("CreateAgent: malformed request body (invalid UTF-8)");
            return xml_error("malformed request body");
        }
    };

    let trimmed = text.trim_start();
    if trimmed.starts_with('<') {
        // CreateAgent expects form-encoded data; any XML (well-formed or not)
        // is rejected.  Distinguish malformed XML-RPC for diagnostics.
        let msg = if trimmed.starts_with("<?xml") || trimmed.starts_with("<methodCall") {
            "malformed XML-RPC body"
        } else {
            "malformed XML body"
        };
        tracing::warn!("CreateAgent: {msg}");
        return xml_error(msg);
    }

    let params = parse_form(&text);

    // ---- Validation -------------------------------------------------------
    let agent_id = match params.get("agent_id").and_then(|v| Uuid::parse_str(v).ok()) {
        Some(v) => v,
        None => {
            tracing::warn!("CreateAgent: missing/invalid agent_id");
            return xml_error("missing or invalid agent_id");
        }
    };
    let session_id = match params.get("session_id").and_then(|v| Uuid::parse_str(v).ok()) {
        Some(v) => v,
        None => {
            tracing::warn!("CreateAgent: missing/invalid session_id");
            return xml_error("missing or invalid session_id");
        }
    };

    let secure_session_id = params
        .get("secure_session_id")
        .and_then(|v| Uuid::parse_str(v).ok());
    let destination_uuid = params
        .get("destination_uuid")
        .and_then(|v| Uuid::parse_str(v).ok());
    let region_handle = params.get("region_handle").and_then(|v| v.parse::<u64>().ok());

    let client_ip = params.get("ip_address").cloned().unwrap_or_default();
    let viewer = params.get("viewer").cloned().unwrap_or_default();
    let server_token = params
        .get("service_session_id")
        .or_else(|| params.get("service_token"))
        .cloned();

    // ---- VerifyAgent / VerifyClient callback (emulated) -------------------
    // Emulate the foreign gatekeeper calling back to the agent's home grid
    // UserAgentService.  Uses an in-process UserAgentServiceImpl when wired;
    // otherwise (no real home grid reachable) the check is skipped.
    if let Some(ua) = &state.user_agent {
        let token = server_token.clone().unwrap_or_default();
        match ua.verify_agent(session_id, &token) {
            Ok(true) => {}
            _ => {
                tracing::warn!(
                    %agent_id, %session_id,
                    "CreateAgent: VerifyAgent failed"
                );
                return xml_error("VerifyAgent verification failed");
            }
        }
        match ua.verify_client(session_id, &client_ip) {
            Ok(true) => {}
            _ => {
                tracing::warn!(
                    %agent_id, %session_id,
                    "CreateAgent: VerifyClient failed"
                );
                return xml_error("VerifyClient verification failed");
            }
        }
    } else {
        // Emulation unavailable: still require a non-empty service token as a
        // minimal anti-spoofing check so anonymous CreateAgent calls are
        // rejected when no callback service is wired.
        if server_token.as_deref().unwrap_or("").is_empty() {
            tracing::warn!("CreateAgent: missing service session token");
            return xml_error("missing service_session_id / service_token");
        }
    }

    // ---- UUID impersonation guard -----------------------------------------
    if let Some(accounts) = &state.accounts_store {
        if let Ok(Some(_acct)) = accounts.get_by_id(&agent_id.to_string()).await {
            // agent_id collides with a local account: only allowed if the agent
            // is genuinely coming home.
            let coming_home = state
                .user_agent
                .as_ref()
                .map(|ua| {
                    ua.is_agent_coming_home(session_id, &state.grid_external_name)
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if !coming_home {
                tracing::warn!(
                    %agent_id, %session_id,
                    "CreateAgent: UUID impersonation rejected"
                );
                return xml_error("UUID impersonation: agent not coming home");
            }
        }
    }

    let record = ForeignAgentRecord {
        agent_id,
        session_id,
        secure_session_id,
        destination_uuid,
        region_handle,
        client_ip,
        viewer,
    };

    state.records.write().unwrap().insert(session_id, record);
    tracing::info!(%agent_id, %session_id, "CreateAgent: foreign agent record persisted");

    let body = String::from(
        "<?xml version=\"1.0\"?><llsd><map></map></llsd>",
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

fn xml_error(msg: &str) -> Response {
    let body = format!(
        "<?xml version=\"1.0\"?><llsd><map><key>error</key><string>{}</string></map></llsd>",
        xml_escape(msg)
    );
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

fn parse_form(body: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in body.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let val = parts.next().unwrap_or("");
        out.insert(url_decode(key), url_decode(val));
    }
    out
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v as char);
                    i += 3;
                    continue;
                }
                out.push(bytes[i] as char);
                i += 1;
            }
            _ => {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}


/// Public entry point alias used by callers/tests.
#[allow(dead_code)]
pub async fn validate_and_verify_agent(state: &CreateAgentState, body: Bytes) -> Response {
    handle_create_agent_inner(state, body).await
}

/// PascalCase alias for `validate_and_verify_agent`.
#[allow(non_snake_case, dead_code)]
pub async fn ValidateAndVerifyAgent(state: &CreateAgentState, body: Bytes) -> Response {
    validate_and_verify_agent(state, body).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::http::Request;
    use tower::util::ServiceExt;
    use welkin_services::user_agent::{UserAgentConfig, UserAgentServiceImpl};

    fn ua(bypass: bool) -> Arc<UserAgentServiceImpl> {
        Arc::new(UserAgentServiceImpl::new(UserAgentConfig {
            grid_name: "http://mygrid.example:9000/".into(),
            my_external_ip: "203.0.113.1".into(),
            bypass_client_verification: bypass,
            level_outside_contacts: 0,
            show_details: true,
        }))
    }

    fn body(params: &[(&str, &str)]) -> String {
        params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&")
    }

    async fn post(state: CreateAgentState, body: String) -> u16 {
        let app: Router = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/create_agent")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(axum::body::Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        resp.status().as_u16()
    }

    #[tokio::test]
    async fn missing_token_rejected_when_no_ua() {
        let sid = Uuid::new_v4();
        let aid = Uuid::new_v4();
        let status = post(
            CreateAgentState::default(),
            body(&[("agent_id", &aid.to_string()), ("session_id", &sid.to_string())]),
        )
        .await;
        assert_eq!(status, 400);
    }

    #[tokio::test]
    async fn verify_agent_success() {
        let session = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let token = "tok-123";
        let service = ua(true);
        service.store_travel_info(welkin_services::hypergrid::TravelingAgentInfo {
            session_id: session,
            user_id: agent,
            grid_external_name: "http://foreign.example:9000/".into(),
            service_token: token.into(),
            client_ip_address: "10.0.0.1".into(),
        });

        let mut state = CreateAgentState::default().with_user_agent(service);
        state.grid_external_name = "http://foreign.example:9000/".into();

        let status = post(
            state,
            body(&[
                ("agent_id", &agent.to_string()),
                ("session_id", &session.to_string()),
                ("service_session_id", token),
                ("ip_address", "10.0.0.1"),
            ]),
        )
        .await;
        assert_eq!(status, 200);
    }

    #[tokio::test]
    async fn verify_agent_failure() {
        let session = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let service = ua(true);
        service.store_travel_info(welkin_services::hypergrid::TravelingAgentInfo {
            session_id: session,
            user_id: agent,
            grid_external_name: "http://foreign.example:9000/".into(),
            service_token: "correct".into(),
            client_ip_address: "10.0.0.1".into(),
        });

        let state = CreateAgentState::default().with_user_agent(service);
        let status = post(
            state,
            body(&[
                ("agent_id", &agent.to_string()),
                ("session_id", &session.to_string()),
                ("service_session_id", "wrong"),
                ("ip_address", "10.0.0.1"),
            ]),
        )
        .await;
        assert_eq!(status, 400);
    }
}