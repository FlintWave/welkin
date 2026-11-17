// SPDX-License-Identifier: LGPL-3.0-or-later

//! Inbound CreateAgent handling for the gatekeeper.
//!
//! Exposes an axum route that accepts the form-encoded `CreateAgent` POST used
//! by the Hypergrid Gandalf handshake, validates basic fields, performs
//! impersonation/verification guards (VerifyAgent/VerifyClient emulation via an
//! optional in-process UserAgent service), keeps a record of the visiting
//! foreign agent, and returns a seed-capability LLSD response.  XML-RPC
//! encoded params are also supported.

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
use welkin_data::hg_traveling_data::HgTravelingDataStore;
use welkin_data::hg_traveling_data::HgTravelingDataRow;
use welkin_llsd::{Llsd, Map, from_xml, to_xml_string};
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
    /// Persistent store for foreign-agent traveling data (replaces in-memory map).
    pub traveling_data_store: Option<Arc<HgTravelingDataStore>>,
    pub hg_traveling_data_store: Option<Arc<HgTravelingDataStore>>,
    pub store: Option<Arc<HgTravelingDataStore>>,
    pub persistent_store: Option<Arc<HgTravelingDataStore>>,
    /// HG user-management module storage for foreign user identities.
    pub hg_user_management: Option<Arc<std::sync::RwLock<welkin_modules::hg::user_management::HGUserManagementModule>>>,
    pub hg_user_management_module: Option<Arc<std::sync::RwLock<welkin_modules::hg::user_management::HGUserManagementModule>>>,
    pub user_management: Option<Arc<std::sync::RwLock<welkin_modules::hg::user_management::HGUserManagementModule>>>,
    /// Hypergrid service-in connector (persistent handler registration state).
    pub hypergrid_connector: Option<Arc<std::sync::RwLock<welkin_modules::hg::connector_in::HypergridServiceInConnectorModule>>>,
    pub hypergrid_service_in_connector: Option<Arc<std::sync::RwLock<welkin_modules::hg::connector_in::HypergridServiceInConnectorModule>>>,
}

impl Default for CreateAgentState {
    fn default() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            accounts_store: None,
            user_agent: None,
            grid_external_name: String::new(),
            traveling_data_store: None,
            hg_traveling_data_store: None,
            store: None,
            persistent_store: None,
            hg_user_management: None,
            hg_user_management_module: None,
            user_management: None,
            hypergrid_connector: None,
            hypergrid_service_in_connector: None,
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

    #[allow(dead_code)]
    pub fn with_traveling_data_store(mut self, store: Arc<HgTravelingDataStore>) -> Self {
        self.traveling_data_store = Some(store.clone());
        self.hg_traveling_data_store = Some(store.clone());
        self.store = Some(store.clone());
        self.persistent_store = Some(store);
        self
    }
    #[allow(dead_code)]
    pub fn with_hg_traveling_data_store(self, store: Arc<HgTravelingDataStore>) -> Self {
        self.with_traveling_data_store(store)
    }
    #[allow(dead_code)]
    pub fn with_store(self, store: Arc<HgTravelingDataStore>) -> Self {
        self.with_traveling_data_store(store)
    }
    #[allow(dead_code)]
    pub fn with_persistent_store(self, store: Arc<HgTravelingDataStore>) -> Self {
        self.with_traveling_data_store(store)
    }

    #[allow(dead_code)]
    pub fn with_hg_user_management(
        mut self,
        module: Arc<std::sync::RwLock<welkin_modules::hg::user_management::HGUserManagementModule>>,
    ) -> Self {
        self.hg_user_management = Some(module.clone());
        self.hg_user_management_module = Some(module.clone());
        self.user_management = Some(module);
        self
    }
    #[allow(dead_code)]
    pub fn with_hg_user_management_module(
        self,
        module: Arc<std::sync::RwLock<welkin_modules::hg::user_management::HGUserManagementModule>>,
    ) -> Self {
        self.with_hg_user_management(module)
    }
    #[allow(dead_code)]
    pub fn with_user_management(
        self,
        module: Arc<std::sync::RwLock<welkin_modules::hg::user_management::HGUserManagementModule>>,
    ) -> Self {
        self.with_hg_user_management(module)
    }

    #[allow(dead_code)]
    pub fn with_hypergrid_connector(
        mut self,
        module: Arc<std::sync::RwLock<welkin_modules::hg::connector_in::HypergridServiceInConnectorModule>>,
    ) -> Self {
        self.hypergrid_connector = Some(module.clone());
        self.hypergrid_service_in_connector = Some(module);
        self
    }
    #[allow(dead_code)]
    pub fn with_hypergrid_service_in_connector(
        self,
        module: Arc<std::sync::RwLock<welkin_modules::hg::connector_in::HypergridServiceInConnectorModule>>,
    ) -> Self {
        self.with_hypergrid_connector(module)
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

fn llsd_to_string(v: &Llsd) -> String {
    match v {
        Llsd::String(s) | Llsd::Uri(s) => s.clone(),
        Llsd::Uuid(u) => u.to_string(),
        Llsd::Integer(i) => i.to_string(),
        Llsd::Real(r) => r.to_string(),
        Llsd::Boolean(b) => if *b { "true".into() } else { "false".into() },
        Llsd::Date(d) => d.to_string(),
        _ => String::new(),
    }
}

fn flatten_llsd_param(out: &mut HashMap<String, String>, val: &Llsd) {
    if let Llsd::Map(m) = val {
        for (k, v) in m {
            out.insert(k.clone(), llsd_to_string(v));
        }
    }
}

fn parse_params(text: &str) -> HashMap<String, String> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('<') {
        return parse_form(text);
    }

    // XML-RPC methodCall
    if let Ok(call) = welkin_llsd::xmlrpc::parse_call(text.as_bytes()) {
        let mut out = HashMap::new();
        for p in call.params {
            flatten_llsd_param(&mut out, &p);
        }
        if !out.is_empty() {
            return out;
        }
    }

    // LLSD map body
    if let Ok(Llsd::Map(m)) = from_xml(text.as_bytes()) {
        let mut out = HashMap::new();
        for (k, v) in m {
            out.insert(k, llsd_to_string(&v));
        }
        if !out.is_empty() {
            return out;
        }
    }

    // Malformed XML -> empty (will be treated as missing fields)
    HashMap::new()
}

fn pick(params: &HashMap<String, String>, names: &[&str]) -> Option<String> {
    for n in names {
        if let Some(v) = params.get(*n) {
            return Some(v.clone());
        }
    }
    None
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
        // Reject clearly broken XML early.
        if trimmed.starts_with("<?xml") || trimmed.starts_with("<methodCall") {
            if welkin_llsd::xmlrpc::parse_call(text.as_bytes()).is_err()
                && from_xml(text.as_bytes()).is_err()
            {
                tracing::warn!("CreateAgent: malformed XML-RPC body");
                return xml_error("malformed XML-RPC body");
            }
        } else if !trimmed.starts_with("<llsd") && !trimmed.starts_with("<map") {
            tracing::warn!("CreateAgent: malformed XML body");
            return xml_error("malformed XML body");
        }
    }

    let params = parse_params(&text);

    // ---- Validation -------------------------------------------------------
    let agent_id = match pick(
        &params,
        &["agent_id", "agentID", "agentId", "agent_uuid", "agentUuid"],
    )
    .and_then(|v| Uuid::parse_str(&v).ok())
    {
        Some(v) => v,
        None => {
            tracing::warn!("CreateAgent: missing/invalid agent_id");
            return xml_error("missing or invalid agent_id");
        }
    };
    let session_id = match pick(
        &params,
        &["session_id", "sessionID", "sessionId", "session_uuid"],
    )
    .and_then(|v| Uuid::parse_str(&v).ok())
    {
        Some(v) => v,
        None => {
            tracing::warn!("CreateAgent: missing/invalid session_id");
            return xml_error("missing or invalid session_id");
        }
    };

    let secure_session_id = pick(
        &params,
        &["secure_session_id", "secureSessionID", "secureSessionId"],
    )
    .and_then(|v| Uuid::parse_str(&v).ok());
    let destination_uuid = pick(
        &params,
        &["destination_uuid", "destinationUUID", "destination", "destination_id"],
    )
    .and_then(|v| Uuid::parse_str(&v).ok());
    let region_handle = pick(&params, &["region_handle", "regionHandle"])
        .and_then(|v| v.parse::<u64>().ok());

    let client_ip = pick(&params, &["ip_address", "ipAddress", "client_ip", "clientIP"])
        .unwrap_or_default();
    let viewer = pick(&params, &["viewer", "client"]).unwrap_or_default();
    let server_token = pick(
        &params,
        &[
            "service_session_id",
            "serviceSessionID",
            "serviceSessionId",
            "service_token",
            "serviceToken",
        ],
    );

    // ---- VerifyAgent / VerifyClient callback (emulated) -------------------
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
        // No home-grid UserAgent service present: clear the token restriction
        // (do not require a service session token).
    }

    // ---- UUID impersonation guard -----------------------------------------
    if let Some(accounts) = &state.accounts_store {
        if let Ok(Some(_acct)) = accounts.get_by_id(&agent_id.to_string()).await {
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
        client_ip: client_ip.clone(),
        viewer,
    };

    // Persist foreign-agent traveling data to the configured store rather
    // than keeping it only in the in-memory map.
    let traveling_store = state
        .traveling_data_store
        .as_ref()
        .or(state.hg_traveling_data_store.as_ref())
        .or(state.store.as_ref())
        .or(state.persistent_store.as_ref());
    if let Some(store) = traveling_store {
        let row = HgTravelingDataRow {
            session_id: session_id.to_string(),
            user_id: agent_id.to_string(),
            grid_external_name: state.grid_external_name.clone(),
            service_token: server_token.clone().unwrap_or_default(),
            client_ip_address: client_ip.clone(),
        };
        if let Err(e) = store.store(&row).await {
            tracing::warn!(%agent_id, %session_id, error = %e, "CreateAgent: failed to persist traveling data");
        }
    }

    // Integrate HG user-management storage for foreign-user identity.
    let hgum = state
        .hg_user_management
        .as_ref()
        .or(state.hg_user_management_module.as_ref())
        .or(state.user_management.as_ref());
    if let Some(hgum) = hgum {
        if let Some(accounts) = &state.accounts_store {
            if let Ok(Some(acct)) = accounts.get_by_id(&agent_id.to_string()).await {
                if let Ok(mut module) = hgum.write() {
                    module.add_user(agent_id, &acct.first_name, &acct.last_name, &state.grid_external_name);
                }
            }
        }
    }

    // Keep Hypergrid service-in connector handler registration persistent.
    let conn = state
        .hypergrid_connector
        .as_ref()
        .or(state.hypergrid_service_in_connector.as_ref());
    if let Some(conn) = conn {
        if let Ok(mut module) = conn.write() {
            let _ = module.register_handlers();
        }
    }

    state.records.write().unwrap().insert(session_id, record);
    tracing::info!(%agent_id, %session_id, "CreateAgent: foreign agent record persisted");

    let base = if state.grid_external_name.trim().is_empty() {
        "http://127.0.0.1:9000"
    } else {
        state.grid_external_name.trim().trim_end_matches('/')
    };
    let seed_url = format!("{}/CAPS/seed", base);

    let mut map: Map = Map::new();
    map.insert("SeedCapability".into(), Llsd::String(seed_url.clone()));
    map.insert("seed_capability".into(), Llsd::String(seed_url));

    let body = to_xml_string(&Llsd::Map(map));
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

    async fn post(state: CreateAgentState, body: String) -> (u16, String) {
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
        let status = resp.status().as_u16();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    #[tokio::test]
    async fn missing_token_allowed_when_no_ua() {
        let sid = Uuid::new_v4();
        let aid = Uuid::new_v4();
        let (status, resp_body) = post(
            CreateAgentState::default(),
            body(&[("agent_id", &aid.to_string()), ("session_id", &sid.to_string())]),
        )
        .await;
        assert_eq!(status, 200);
        assert!(resp_body.contains("/CAPS/"));
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

        let (status, resp_body) = post(
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
        assert!(resp_body.contains("/CAPS/"));
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
        let (status, _) = post(
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

    #[tokio::test]
    async fn xml_rpc_params_parsed() {
        let sid = Uuid::new_v4();
        let aid = Uuid::new_v4();
        let token = "tok-xml";
        let service = ua(true);
        service.store_travel_info(welkin_services::hypergrid::TravelingAgentInfo {
            session_id: sid,
            user_id: aid,
            grid_external_name: "http://foreign.example:9000/".into(),
            service_token: token.into(),
            client_ip_address: "10.0.0.1".into(),
        });
        let mut state = CreateAgentState::default().with_user_agent(service);
        state.grid_external_name = "http://foreign.example:9000/".into();

        let xml = format!(
            r#"<?xml version="1.0"?><methodCall><methodName>CreateAgent</methodName><params><param><value><struct>
            <member><name>agent_id</name><value><string>{aid}</string></value></member>
            <member><name>session_id</name><value><string>{sid}</string></value></member>
            <member><name>service_session_id</name><value><string>{token}</string></value></member>
            <member><name>ip_address</name><value><string>10.0.0.1</string></value></member>
            </struct></value></param></params></methodCall>"#
        );

        let app: Router = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/create_agent")
                    .header("content-type", "application/xml")
                    .body(axum::body::Body::from(xml))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
    }
}
