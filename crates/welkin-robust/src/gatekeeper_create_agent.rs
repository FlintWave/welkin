// SPDX-License-Identifier: LGPL-3.0-or-later

//! Inbound CreateAgent handling for the gatekeeper.
//!
//! This is an initial skeleton: it exposes an axum route that accepts the
//! form-encoded `CreateAgent` POST used by the Hypergrid Gandalf handshake,
//! validates the basic fields, keeps a record of the visiting foreign agent,
//! and returns an (empty) seed-capability LLSD response.  Full handoff
//! (VerifyAgent / VerifyClient callbacks, UUID-impersonation checks, presence
//! creation, simulation CreateAgent) is intentionally left beyond this prefix.

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

// ---------------------------------------------------------------------------
// State and records
// ---------------------------------------------------------------------------

/// A minimal record of a foreign agent that has just arrived via Hypergrid.
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

/// Shared state for the inbound CreateAgent handler.
#[derive(Clone)]
pub struct CreateAgentState {
    /// session_id -> foreign agent record.
    pub records: Arc<RwLock<HashMap<Uuid, ForeignAgentRecord>>>,
}

impl Default for CreateAgentState {
    fn default() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: CreateAgentState) -> Router {
    // HG CreateAgent posts are conventionally sent to the base grid URL.
    Router::new()
        .route("/create_agent", post(handle_create_agent))
        .route("/", post(handle_create_agent))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

async fn handle_create_agent(State(state): State<CreateAgentState>, body: Bytes) -> Response {
    let text = String::from_utf8_lossy(&body);
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

    // ---- Verification (prefix only) --------------------------------------
    // TODO(handoff): call the home grid UserAgentService.VerifyAgent(session_id, token)
    // and VerifyClient(session_id, reported_ip).  The service-session token is
    // expected in params["service_session_id"] / params["service_token"].
    let _service_token = params
        .get("service_session_id")
        .or_else(|| params.get("service_token"));
    // TODO(handoff): check UUID impersonation against local accounts (AccountsStore).
    // TODO(handoff): enforce foreign-agent allow/deny rules (GatekeeperConfig).

    let record = ForeignAgentRecord {
        agent_id,
        session_id,
        secure_session_id,
        destination_uuid,
        region_handle,
        client_ip: params.get("ip_address").cloned().unwrap_or_default(),
        viewer: params.get("viewer").cloned().unwrap_or_default(),
    };

    state.records.write().unwrap().insert(session_id, record);
    tracing::info!(%agent_id, %session_id, "CreateAgent: foreign agent record persisted");

    // ---- Persistence via HGUserManagementModule --------------------------
    // HGUserManagementModule lives in welkin-modules and is not currently
    // exposed to the robust host.  When wired, foreign agent identity should
    // be added via module.add_user(agent_id, first, last, home_uri).
    // (Left as a handoff item; see root note in task.)

    // Seed-capability response (empty map is acceptable to the gandalf parser).
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

// ---------------------------------------------------------------------------
// Form parsing helpers
// ---------------------------------------------------------------------------

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
