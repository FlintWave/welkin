// SPDX-License-Identifier: LGPL-3.0-or-later

//! User-Agent service implementation for Hypergrid.
//!
//! Runs on the agent's **home** grid and manages cross-grid identity,
//! travel records, and presence information.

use std::collections::HashMap;
use std::sync::RwLock;

use uuid::Uuid;

use crate::hypergrid::*;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the [`UserAgentServiceImpl`].
pub struct UserAgentConfig {
    /// This grid's gatekeeper URL, lowercased, ending with `/`.
    pub grid_name: String,
    /// The external IP address of this grid (for NAT detection).
    pub my_external_ip: String,
    /// When `true`, client-IP verification is skipped entirely.
    pub bypass_client_verification: bool,
    /// Minimum user-account level required for cross-grid contact resolution.
    pub level_outside_contacts: i32,
    /// Whether to expose detailed user profile information to foreign grids.
    pub show_details: bool,
}

// ---------------------------------------------------------------------------
// Service implementation
// ---------------------------------------------------------------------------

/// User-Agent service backed by an in-memory traveling-data store.
///
/// In production, the `traveling_data` map would be replaced by an
/// [`welkin_data::hg_traveling_data::HgTravelingDataStore`] (or equivalent
/// persistent store).  The in-memory variant keeps tests self-contained.
pub struct UserAgentServiceImpl {
    config: UserAgentConfig,
    /// Session-ID -> TravelingAgentInfo.
    traveling_data: RwLock<HashMap<Uuid, TravelingAgentInfo>>,
}

impl UserAgentServiceImpl {
    /// Create a new service with the given configuration.
    pub fn new(config: UserAgentConfig) -> Self {
        Self {
            config,
            traveling_data: RwLock::new(HashMap::new()),
        }
    }

    /// Insert a traveling-agent record directly (useful for tests and
    /// administrative tooling).
    pub fn store_travel_info(&self, info: TravelingAgentInfo) {
        let mut map = self.traveling_data.write().unwrap();
        map.insert(info.session_id, info);
    }

    /// Look up a traveling-agent record by session ID.
    fn get_travel_info(&self, session_id: Uuid) -> Option<TravelingAgentInfo> {
        let map = self.traveling_data.read().unwrap();
        map.get(&session_id).cloned()
    }

    /// Remove a traveling-agent record by session ID.
    fn remove_travel_info(&self, session_id: Uuid) -> bool {
        let mut map = self.traveling_data.write().unwrap();
        map.remove(&session_id).is_some()
    }
}

impl UserAgentService for UserAgentServiceImpl {
    fn login_agent_to_grid(
        &self,
        _source: Option<&GridRegionInfo>,
        circuit: &AgentCircuitInfo,
        gatekeeper: &GridRegionInfo,
        _final_destination: &GridRegionInfo,
        from_login: bool,
    ) -> HgResult<()> {
        // Generate a service-session token: gatekeeper_uri;random_uuid
        let service_token = format!("{};{}", gatekeeper.server_uri, Uuid::new_v4());

        let mut travel = TravelingAgentInfo {
            session_id: circuit.session_id,
            user_id: circuit.agent_id,
            grid_external_name: gatekeeper.server_uri.clone(),
            service_token,
            client_ip_address: String::new(),
        };

        // Preserve the client IP from any existing record, or set it on first
        // login.
        if let Some(existing) = self.get_travel_info(circuit.session_id) {
            if !from_login {
                travel.client_ip_address = existing.client_ip_address;
            } else {
                travel.client_ip_address = circuit.ip_address.clone();
            }
        } else if from_login {
            travel.client_ip_address = circuit.ip_address.clone();
        }

        self.store_travel_info(travel);

        // NOTE: The actual foreign-gatekeeper CreateAgent call is handled by
        // the connector layer (GatekeeperServiceConnector).  This service
        // only manages the local traveling-data record.
        Ok(())
    }

    fn logout_agent(&self, _user_id: Uuid, session_id: Uuid) -> HgResult<()> {
        tracing::debug!(%session_id, "user agent logout");
        self.remove_travel_info(session_id);
        Ok(())
    }

    fn get_home_region(&self, user_id: Uuid) -> HgResult<(GridRegionInfo, Vector3, Vector3)> {
        // Stub -- in production this would query the GridUserService for the
        // user's home region, then the GridService for region details.
        Err(HypergridError::RegionNotFound(format!(
            "home region for user {user_id}"
        )))
    }

    fn get_server_urls(&self, _user_id: Uuid) -> HgResult<HashMap<String, String>> {
        // Stub -- in production this would look up the user account and
        // return ServiceURLs (HomeURI, InventoryServerURI, AssetServerURI,
        // etc.).
        Ok(HashMap::new())
    }

    fn get_user_info(&self, _user_id: Uuid) -> HgResult<HashMap<String, String>> {
        // Stub -- in production this would look up the user account and
        // return first/last name, flags, created date, title.
        let mut info = HashMap::new();
        info.insert("result".into(), "fail".into());
        info.insert(
            "message".into(),
            "UserAccountService is not available".into(),
        );
        Ok(info)
    }

    fn locate_user(&self, user_id: Uuid) -> HgResult<String> {
        let map = self.traveling_data.read().unwrap();
        for travel in map.values() {
            if travel.user_id == user_id
                && !travel
                    .grid_external_name
                    .eq_ignore_ascii_case(&self.config.grid_name)
            {
                return Ok(travel.grid_external_name.clone());
            }
        }
        Ok(String::new())
    }

    fn get_uui(&self, _requester_id: Uuid, _target_user_id: Uuid) -> HgResult<String> {
        // Stub -- in production this would look up the user account by
        // target_user_id and return "uuid;grid_name;FirstName LastName", or
        // search the friends list for foreign-user UUI.
        Ok(String::new())
    }

    fn get_uuid(&self, _first: &str, _last: &str) -> HgResult<Option<Uuid>> {
        // Stub -- in production this would look up the user account by name
        // and check user_level >= level_outside_contacts.
        Ok(None)
    }

    fn verify_agent(&self, session_id: Uuid, token: &str) -> HgResult<bool> {
        match self.get_travel_info(session_id) {
            Some(travel) => {
                let matches = travel.service_token == token;
                tracing::debug!(
                    %session_id,
                    matches,
                    "verify_agent: token comparison"
                );
                Ok(matches)
            }
            None => {
                tracing::debug!(
                    %session_id,
                    "verify_agent: no such session"
                );
                Ok(false)
            }
        }
    }

    fn verify_client(&self, session_id: Uuid, reported_ip: &str) -> HgResult<bool> {
        if self.config.bypass_client_verification {
            return Ok(true);
        }

        match self.get_travel_info(session_id) {
            Some(travel) => {
                let mut result = travel.client_ip_address == reported_ip;
                if !result && !self.config.my_external_ip.is_empty() {
                    result = reported_ip == self.config.my_external_ip;
                }
                tracing::debug!(
                    %session_id,
                    %reported_ip,
                    stored_ip = %travel.client_ip_address,
                    my_external_ip = %self.config.my_external_ip,
                    result,
                    "verify_client"
                );
                Ok(result)
            }
            None => Ok(false),
        }
    }

    fn is_agent_coming_home(
        &self,
        session_id: Uuid,
        this_grid_external_name: &str,
    ) -> HgResult<bool> {
        match self.get_travel_info(session_id) {
            Some(travel) => Ok(travel
                .grid_external_name
                .eq_ignore_ascii_case(this_grid_external_name)),
            None => Ok(false),
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP handler (XML-RPC)
// ---------------------------------------------------------------------------
//
// The C# UserAgentServerConnector exposes all endpoints as XML-RPC methods.
// In the Rust port we provide a simplified axum-based POST handler that
// accepts XML-RPC requests and dispatches by method name.
//
// For protocol compatibility the handler reads the `methodName` element from
// the XML-RPC body and returns a well-formed XML-RPC response.  To keep the
// implementation lean (and avoid pulling in a full XML-RPC library) we use
// simple string-based XML construction.  A production deployment may choose
// to use a proper XML-RPC crate.
//
// NOTE: The agent-creation endpoint (homeagent/) is handled separately by
// the simulation service layer and is not part of this module.

use std::sync::Arc;

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};

/// Shared state for the User-Agent HTTP handler.
#[derive(Clone)]
pub struct UserAgentHandlerState {
    pub service: Arc<UserAgentServiceImpl>,
}

/// Build an axum [`Router`] serving the User-Agent XML-RPC endpoints.
pub fn router(state: UserAgentHandlerState) -> Router {
    Router::new()
        .route("/useragent", post(handle_xmlrpc))
        .with_state(state)
}

// ---- helpers ----

/// Build a minimal XML-RPC response containing a single `<struct>` with
/// string members.
fn xmlrpc_response(members: &[(&str, &str)]) -> Response {
    let mut body = String::from(
        "<?xml version=\"1.0\"?>\
         <methodResponse><params><param><value><struct>",
    );
    for (name, value) in members {
        body.push_str("<member><name>");
        body.push_str(name);
        body.push_str("</name><value><string>");
        body.push_str(&xml_escape(value));
        body.push_str("</string></value></member>");
    }
    body.push_str("</struct></value></param></params></methodResponse>");

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Extract a parameter value from the XML-RPC request body.
///
/// This is deliberately minimal -- it scans for
/// `<name>key</name><value><string>val</string></value>` or
/// `<name>key</name><value>val</value>` patterns.
fn extract_param(xml: &str, key: &str) -> Option<String> {
    // Look for <name>key</name><value>... patterns
    let name_tag = format!("<name>{key}</name>");
    let idx = xml.find(&name_tag)?;
    let rest = &xml[idx + name_tag.len()..];

    // Skip to <value>
    let val_start = rest.find("<value>")?;
    let after_value = &rest[val_start + 7..];

    // The value might be wrapped in <string>...</string> or bare text
    let content = if let Some(s) = after_value.strip_prefix("<string>") {
        let end = s.find("</string>")?;
        &s[..end]
    } else {
        let end = after_value.find("</value>")?;
        &after_value[..end]
    };

    Some(content.to_string())
}

/// Extract the method name from an XML-RPC request body.
fn extract_method_name(xml: &str) -> Option<String> {
    let start = xml.find("<methodName>")?;
    let rest = &xml[start + 12..];
    let end = rest.find("</methodName>")?;
    Some(rest[..end].to_string())
}

async fn handle_xmlrpc(State(state): State<UserAgentHandlerState>, body: Bytes) -> Response {
    let xml = String::from_utf8_lossy(&body);
    let method = match extract_method_name(&xml) {
        Some(m) => m,
        None => {
            return xmlrpc_response(&[("result", "false")]);
        }
    };

    tracing::debug!(method = %method, "user_agent xmlrpc request");

    match method.as_str() {
        "verify_agent" => {
            let session_id = extract_param(&xml, "sessionID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());
            let token = extract_param(&xml, "token").unwrap_or_default();

            let success = state
                .service
                .verify_agent(session_id, &token)
                .unwrap_or(false);
            xmlrpc_response(&[("result", &success.to_string())])
        }

        "verify_client" => {
            let session_id = extract_param(&xml, "sessionID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());
            let token = extract_param(&xml, "token").unwrap_or_default();

            let success = state
                .service
                .verify_client(session_id, &token)
                .unwrap_or(false);
            xmlrpc_response(&[("result", &success.to_string())])
        }

        "get_home_region" => {
            let user_id = extract_param(&xml, "userID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());

            match state.service.get_home_region(user_id) {
                Ok((region, position, look_at)) => xmlrpc_response(&[
                    ("result", "true"),
                    ("uuid", &region.region_id.to_string()),
                    ("x", &region.region_loc_x.to_string()),
                    ("y", &region.region_loc_y.to_string()),
                    ("size_x", &region.region_size_x.to_string()),
                    ("size_y", &region.region_size_y.to_string()),
                    ("region_name", &region.region_name),
                    ("hostname", &region.external_host_name),
                    ("internal_port", &region.internal_port.to_string()),
                    ("server_uri", &region.server_uri),
                    ("position", &position.to_string()),
                    ("lookAt", &look_at.to_string()),
                ]),
                Err(_) => xmlrpc_response(&[("result", "false")]),
            }
        }

        "agent_is_coming_home" => {
            let session_id = extract_param(&xml, "sessionID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());
            let grid_name = extract_param(&xml, "externalName").unwrap_or_default();

            let success = state
                .service
                .is_agent_coming_home(session_id, &grid_name)
                .unwrap_or(false);
            xmlrpc_response(&[("result", &success.to_string())])
        }

        "logout_agent" => {
            let session_id = extract_param(&xml, "sessionID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());
            let user_id = extract_param(&xml, "userID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());

            let _ = state.service.logout_agent(user_id, session_id);
            xmlrpc_response(&[("result", "true")])
        }

        "get_user_info" => {
            let user_id = extract_param(&xml, "userID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());

            match state.service.get_user_info(user_id) {
                Ok(info) if !info.is_empty() => {
                    let pairs: Vec<(String, String)> = info.into_iter().collect();
                    let refs: Vec<(&str, &str)> = pairs
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    xmlrpc_response(&refs)
                }
                _ => xmlrpc_response(&[("result", "failure")]),
            }
        }

        "get_server_urls" => {
            let user_id = extract_param(&xml, "userID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());

            match state.service.get_server_urls(user_id) {
                Ok(urls) if !urls.is_empty() => {
                    // C# prefixes each key with "SRV_"
                    let pairs: Vec<(String, String)> = urls
                        .into_iter()
                        .map(|(k, v)| (format!("SRV_{k}"), v))
                        .collect();
                    let refs: Vec<(&str, &str)> = pairs
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    xmlrpc_response(&refs)
                }
                _ => xmlrpc_response(&[("result", "No Service URLs")]),
            }
        }

        "locate_user" => {
            let user_id = extract_param(&xml, "userID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());

            match state.service.locate_user(user_id) {
                Ok(url) if !url.is_empty() => xmlrpc_response(&[("URL", &url)]),
                _ => xmlrpc_response(&[("result", "Unable to locate user")]),
            }
        }

        "get_uui" => {
            let user_id = extract_param(&xml, "userID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());
            let target_user_id = extract_param(&xml, "targetUserID")
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or(Uuid::nil());

            match state.service.get_uui(user_id, target_user_id) {
                Ok(uui) if !uui.is_empty() => xmlrpc_response(&[("UUI", &uui)]),
                _ => xmlrpc_response(&[("result", "User unknown")]),
            }
        }

        "get_uuid" => {
            let first = extract_param(&xml, "first").unwrap_or_default();
            let last = extract_param(&xml, "last").unwrap_or_default();

            let uuid = state
                .service
                .get_uuid(&first, &last)
                .unwrap_or(None)
                .unwrap_or(Uuid::nil());
            xmlrpc_response(&[("UUID", &uuid.to_string())])
        }

        _ => {
            tracing::warn!(method = %method, "unknown user_agent method");
            xmlrpc_response(&[("result", "false")])
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(bypass: bool) -> UserAgentConfig {
        UserAgentConfig {
            grid_name: "http://mygrid.example:9000/".into(),
            my_external_ip: "203.0.113.1".into(),
            bypass_client_verification: bypass,
            level_outside_contacts: 0,
            show_details: true,
        }
    }

    fn make_service(bypass: bool) -> UserAgentServiceImpl {
        UserAgentServiceImpl::new(make_config(bypass))
    }

    fn make_travel(
        session: Uuid,
        user: Uuid,
        grid: &str,
        token: &str,
        ip: &str,
    ) -> TravelingAgentInfo {
        TravelingAgentInfo {
            session_id: session,
            user_id: user,
            grid_external_name: grid.into(),
            service_token: token.into(),
            client_ip_address: ip.into(),
        }
    }

    // ---- verify_agent ----

    #[test]
    fn test_verify_agent_valid() {
        let svc = make_service(false);
        let session = Uuid::new_v4();
        let token = "http://foreign.example:9000/;abc-123";
        svc.store_travel_info(make_travel(
            session,
            Uuid::new_v4(),
            "http://foreign.example:9000/",
            token,
            "10.0.0.1",
        ));

        assert!(svc.verify_agent(session, token).unwrap());
    }

    #[test]
    fn test_verify_agent_invalid() {
        let svc = make_service(false);
        let session = Uuid::new_v4();
        svc.store_travel_info(make_travel(
            session,
            Uuid::new_v4(),
            "http://foreign.example:9000/",
            "correct-token",
            "10.0.0.1",
        ));

        assert!(!svc.verify_agent(session, "wrong-token").unwrap());
    }

    #[test]
    fn test_verify_agent_no_session() {
        let svc = make_service(false);
        assert!(!svc.verify_agent(Uuid::new_v4(), "any-token").unwrap());
    }

    // ---- verify_client ----

    #[test]
    fn test_verify_client_bypass() {
        let svc = make_service(true);
        // Even with no travel data, bypass returns true.
        assert!(svc.verify_client(Uuid::new_v4(), "1.2.3.4").unwrap());
    }

    #[test]
    fn test_verify_client_matching_ip() {
        let svc = make_service(false);
        let session = Uuid::new_v4();
        svc.store_travel_info(make_travel(
            session,
            Uuid::new_v4(),
            "http://foreign.example:9000/",
            "tok",
            "192.168.1.100",
        ));

        assert!(svc.verify_client(session, "192.168.1.100").unwrap());
        assert!(!svc.verify_client(session, "10.0.0.1").unwrap());
    }

    #[test]
    fn test_verify_client_nat_ip() {
        let svc = make_service(false);
        let session = Uuid::new_v4();
        svc.store_travel_info(make_travel(
            session,
            Uuid::new_v4(),
            "http://foreign.example:9000/",
            "tok",
            "10.0.0.1",
        ));

        // The reported IP matches my_external_ip (NAT case).
        assert!(svc.verify_client(session, "203.0.113.1").unwrap());
    }

    #[test]
    fn test_verify_client_no_session() {
        let svc = make_service(false);
        assert!(!svc.verify_client(Uuid::new_v4(), "1.2.3.4").unwrap());
    }

    // ---- is_agent_coming_home ----

    #[test]
    fn test_is_agent_coming_home_true() {
        let svc = make_service(false);
        let session = Uuid::new_v4();
        svc.store_travel_info(make_travel(
            session,
            Uuid::new_v4(),
            "http://mygrid.example:9000/",
            "tok",
            "10.0.0.1",
        ));

        assert!(
            svc.is_agent_coming_home(session, "http://mygrid.example:9000/")
                .unwrap()
        );
        // Case-insensitive.
        assert!(
            svc.is_agent_coming_home(session, "HTTP://MYGRID.EXAMPLE:9000/")
                .unwrap()
        );
    }

    #[test]
    fn test_is_agent_coming_home_false() {
        let svc = make_service(false);
        let session = Uuid::new_v4();
        svc.store_travel_info(make_travel(
            session,
            Uuid::new_v4(),
            "http://foreign.example:9000/",
            "tok",
            "10.0.0.1",
        ));

        assert!(
            !svc.is_agent_coming_home(session, "http://mygrid.example:9000/")
                .unwrap()
        );
    }

    #[test]
    fn test_is_agent_coming_home_no_session() {
        let svc = make_service(false);
        assert!(
            !svc.is_agent_coming_home(Uuid::new_v4(), "http://mygrid.example:9000/")
                .unwrap()
        );
    }

    // ---- locate_user ----

    #[test]
    fn test_locate_user_on_foreign_grid() {
        let svc = make_service(false);
        let user = Uuid::new_v4();
        let session = Uuid::new_v4();
        svc.store_travel_info(make_travel(
            session,
            user,
            "http://foreign.example:9000/",
            "tok",
            "10.0.0.1",
        ));

        let url = svc.locate_user(user).unwrap();
        assert_eq!(url, "http://foreign.example:9000/");
    }

    #[test]
    fn test_locate_user_on_home_grid() {
        let svc = make_service(false);
        let user = Uuid::new_v4();
        let session = Uuid::new_v4();
        // Same grid as config.grid_name -- user is "home".
        svc.store_travel_info(make_travel(
            session,
            user,
            "http://mygrid.example:9000/",
            "tok",
            "10.0.0.1",
        ));

        let url = svc.locate_user(user).unwrap();
        assert!(url.is_empty(), "expected empty, got: {url}");
    }

    #[test]
    fn test_locate_user_unknown() {
        let svc = make_service(false);
        let url = svc.locate_user(Uuid::new_v4()).unwrap();
        assert!(url.is_empty());
    }

    // ---- logout_agent ----

    #[test]
    fn test_logout_agent() {
        let svc = make_service(false);
        let session = Uuid::new_v4();
        let user = Uuid::new_v4();
        svc.store_travel_info(make_travel(
            session,
            user,
            "http://foreign.example:9000/",
            "tok",
            "10.0.0.1",
        ));

        // Verify present.
        assert!(svc.verify_agent(session, "tok").unwrap());

        // Logout.
        svc.logout_agent(user, session).unwrap();

        // Verify gone.
        assert!(!svc.verify_agent(session, "tok").unwrap());
    }

    // ---- login_agent_to_grid ----

    #[test]
    fn test_login_agent_to_grid_creates_travel_info() {
        let svc = make_service(false);
        let session = Uuid::new_v4();
        let agent = Uuid::new_v4();

        let circuit = AgentCircuitInfo {
            agent_id: agent,
            session_id: session,
            secure_session_id: Uuid::new_v4(),
            first_name: "Test".into(),
            last_name: "User".into(),
            start_pos: Vector3::default(),
            service_session_id: String::new(),
            service_urls: HashMap::new(),
            ip_address: "10.0.0.42".into(),
            viewer: "TestViewer".into(),
            channel: "test".into(),
            mac: "aa:bb:cc:dd:ee:ff".into(),
            id0: "id0".into(),
            teleport_flags: 0,
        };

        let gatekeeper = GridRegionInfo {
            region_id: Uuid::new_v4(),
            region_name: "GK Region".into(),
            region_handle: 0,
            server_uri: "http://foreign.example:9000/".into(),
            external_host_name: "foreign.example".into(),
            internal_port: 9000,
            region_loc_x: 1000,
            region_loc_y: 1000,
            region_size_x: 256,
            region_size_y: 256,
        };

        let dest = gatekeeper.clone();

        svc.login_agent_to_grid(None, &circuit, &gatekeeper, &dest, true)
            .unwrap();

        // A travel record should exist.
        let travel = svc.get_travel_info(session).unwrap();
        assert_eq!(travel.user_id, agent);
        assert_eq!(travel.grid_external_name, "http://foreign.example:9000/");
        assert!(
            travel
                .service_token
                .starts_with("http://foreign.example:9000/;")
        );
        assert_eq!(travel.client_ip_address, "10.0.0.42");
    }

    // ---- XML-RPC parameter extraction ----

    #[test]
    fn test_extract_param_string_wrapped() {
        let xml = r#"<methodCall><methodName>verify_agent</methodName>
            <params><param><value><struct>
            <member><name>sessionID</name><value><string>abc-123</string></value></member>
            <member><name>token</name><value><string>my-token</string></value></member>
            </struct></value></param></params></methodCall>"#;

        assert_eq!(extract_param(xml, "sessionID"), Some("abc-123".into()));
        assert_eq!(extract_param(xml, "token"), Some("my-token".into()));
        assert_eq!(extract_param(xml, "missing"), None);
    }

    #[test]
    fn test_extract_method_name() {
        let xml = r#"<?xml version="1.0"?><methodCall><methodName>get_home_region</methodName><params></params></methodCall>"#;
        assert_eq!(extract_method_name(xml), Some("get_home_region".into()));
    }
}
