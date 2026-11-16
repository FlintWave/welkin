// SPDX-License-Identifier: LGPL-3.0-or-later

//! Gatekeeper service implementation.
//!
//! The gatekeeper is the foreign-facing entry point of a grid.  It handles the
//! initial hyperlink handshake (`link_region` / `get_region` in XML-RPC) and
//! the agent-login flow for incoming cross-grid teleports.

use std::sync::{Arc, RwLock};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use regex::Regex;
use uuid::Uuid;

use crate::hypergrid::*;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the gatekeeper service.
pub struct GatekeeperConfig {
    /// This grid's gatekeeper URL, with trailing slash (e.g.
    /// `http://mygrid.example:9000/`).
    pub gatekeeper_url: String,
    /// Additional URLs that should be accepted as aliases for this gatekeeper
    /// when verifying service-session tokens.
    pub gatekeeper_aliases: Vec<String>,
    /// Scope ID for grid-service queries.
    pub scope_id: Uuid,
    /// When `false`, all teleports are redirected to the default gateway
    /// region regardless of the requested destination.
    pub allow_teleports_to_any_region: bool,
    /// Whether foreign (non-local) agents are allowed to enter.
    pub foreign_agents_allowed: bool,
    /// Home URIs that are *excepted* from the allow rule -- i.e. these
    /// origins are denied even when `foreign_agents_allowed` is `true`.
    pub foreigns_allowed_exceptions: Vec<String>,
    /// Home URIs that are *excepted* from the deny rule -- i.e. these
    /// origins are allowed even when `foreign_agents_allowed` is `false`.
    pub foreigns_disallowed_exceptions: Vec<String>,
    /// If set, only viewers matching this regex may enter.
    pub allowed_clients: Option<Regex>,
    /// If set, viewers matching this regex are denied entry.
    pub denied_clients: Option<Regex>,
    /// Comma-separated list of denied MAC addresses (case-insensitive
    /// substring match).
    pub denied_macs: String,
    /// Comma-separated list of denied ID0 values (case-insensitive substring
    /// match).
    pub denied_id0s: String,
}

// ---------------------------------------------------------------------------
// Service implementation
// ---------------------------------------------------------------------------

/// Real implementation of [`GatekeeperService`].
pub struct GatekeeperServiceImpl {
    config: GatekeeperConfig,
    /// Cached default gateway region (set by `link_local_region` when no
    /// specific region is requested, or pre-populated from config).
    default_region: RwLock<Option<GridRegionInfo>>,
}

impl GatekeeperServiceImpl {
    /// Create a new gatekeeper service with the given configuration and an
    /// optional pre-populated default region.
    pub fn new(config: GatekeeperConfig, default_region: Option<GridRegionInfo>) -> Self {
        Self {
            config,
            default_region: RwLock::new(default_region),
        }
    }

    /// Verify that the service-session token was generated for *this* grid.
    ///
    /// The token has the form `gatekeeper_url;random_uuid`.  The gatekeeper
    /// URL embedded in the token must match our own URL or one of the
    /// configured aliases.
    fn check_address(&self, service_token: &str) -> bool {
        let parts: Vec<&str> = service_token.splitn(2, ';').collect();
        if parts.len() < 2 {
            return false;
        }

        let req_grid = normalize_url(parts[0]);
        if req_grid.is_empty() {
            tracing::debug!(token_grid = parts[0], "malformed grid address in token");
            return false;
        }

        let our_url = normalize_url(&self.config.gatekeeper_url);
        if req_grid == our_url {
            return true;
        }

        for alias in &self.config.gatekeeper_aliases {
            if req_grid == normalize_url(alias) {
                return true;
            }
        }

        false
    }

    /// Check whether the agent's home URI appears in the given exception list.
    fn is_exception(home_uri: &str, exceptions: &[String]) -> bool {
        if exceptions.is_empty() {
            return false;
        }

        let user_url = normalize_url(home_uri.trim());
        if user_url.is_empty() {
            return false;
        }

        for exc in exceptions {
            if user_url == normalize_url(exc) {
                return true;
            }
        }

        false
    }

    /// Build the viewer name string from the circuit (matching C#'s
    /// `Util.GetViewerName`).
    fn get_viewer_name(circuit: &AgentCircuitInfo) -> String {
        if circuit.channel.is_empty() {
            circuit.viewer.clone()
        } else {
            format!("{} {}", circuit.channel, circuit.viewer)
        }
    }
}

/// Normalise a URL for comparison: lowercase, ensure trailing slash.
fn normalize_url(url: &str) -> String {
    let mut s = url.to_lowercase();
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

impl GatekeeperService for GatekeeperServiceImpl {
    fn link_local_region(&self, region_name: &str) -> HgResult<LinkedRegionInfo> {
        // When teleports to arbitrary regions are disabled, or the caller did
        // not specify a region name, return the default gateway region.
        if !self.config.allow_teleports_to_any_region || region_name.is_empty() {
            let guard = self.default_region.read().unwrap();
            let region = guard.as_ref().ok_or_else(|| {
                HypergridError::RegionNotFound(
                    "Grid setup problem. Try specifying a particular region here.".into(),
                )
            })?;

            let external_name = if region_name.is_empty() {
                self.config.gatekeeper_url.clone()
            } else {
                format!("{} {}", self.config.gatekeeper_url, region_name)
            };

            let region_image = format!(
                "{}index.php?method=regionImage{}",
                region.server_uri,
                region.region_id.as_simple()
            );

            return Ok(LinkedRegionInfo {
                region_id: region.region_id,
                region_handle: region.region_handle,
                external_name,
                image_url: region_image,
                size_x: region.region_size_x,
                size_y: region.region_size_y,
            });
        }

        // In the full implementation we would query the grid service for
        // `region_name` here.  For now, fall back to the default region if
        // we have one, otherwise error out.
        let guard = self.default_region.read().unwrap();
        let region = guard.as_ref().ok_or_else(|| {
            HypergridError::RegionNotFound(format!("Region not found: {region_name}"))
        })?;

        let external_name = format!("{} {}", self.config.gatekeeper_url, region_name);
        let region_image = format!(
            "{}index.php?method=regionImage{}",
            region.server_uri,
            region.region_id.as_simple()
        );

        Ok(LinkedRegionInfo {
            region_id: region.region_id,
            region_handle: region.region_handle,
            external_name,
            image_url: region_image,
            size_x: region.region_size_x,
            size_y: region.region_size_y,
        })
    }

    fn get_hyperlink_region(
        &self,
        _region_id: Uuid,
        _agent_id: Uuid,
        _agent_home_uri: Option<&str>,
    ) -> HgResult<(GridRegionInfo, Option<String>)> {
        if !self.config.allow_teleports_to_any_region {
            let guard = self.default_region.read().unwrap();
            let region = guard.as_ref().ok_or_else(|| {
                HypergridError::RegionNotFound("No default gateway region configured".into())
            })?;
            return Ok((
                region.clone(),
                Some("Teleporting to the default region.".into()),
            ));
        }

        // In the full implementation we would query the grid service by
        // region_id.  For now, return an error.
        Err(HypergridError::RegionNotFound(
            "Region lookup not yet wired to grid service".into(),
        ))
    }

    fn login_agent(
        &self,
        _source: Option<&GridRegionInfo>,
        circuit: &AgentCircuitInfo,
        _destination: &GridRegionInfo,
    ) -> HgResult<()> {
        let cur_viewer = Self::get_viewer_name(circuit);

        // -- Check allowed clients --
        if let Some(ref re) = self.config.allowed_clients
            && !re.is_match(&cur_viewer)
        {
            return Err(HypergridError::AccessDenied(format!(
                "Login failed: client {cur_viewer} is not allowed"
            )));
        }

        // -- Check denied clients --
        if let Some(ref re) = self.config.denied_clients
            && re.is_match(&cur_viewer)
        {
            return Err(HypergridError::AccessDenied(format!(
                "Login failed: client {cur_viewer} is denied"
            )));
        }

        // -- Check denied MACs --
        if !self.config.denied_macs.is_empty() {
            let macs_lower = self.config.denied_macs.to_lowercase();
            let mac_lower = circuit.mac.to_lowercase();
            if !mac_lower.is_empty() && macs_lower.contains(&mac_lower) {
                return Err(HypergridError::AccessDenied(format!(
                    "Login failed: client with Mac {} is denied",
                    circuit.mac
                )));
            }
        }

        // -- Check denied ID0s --
        if !self.config.denied_id0s.is_empty() {
            let id0s_lower = self.config.denied_id0s.to_lowercase();
            let id0_lower = circuit.id0.to_lowercase();
            if !id0_lower.is_empty() && id0s_lower.contains(&id0_lower) {
                return Err(HypergridError::AccessDenied(format!(
                    "Login failed: client with id0 {} is denied",
                    circuit.id0
                )));
            }
        }

        // -- Authenticate: verify the service-session token addresses this
        //    grid --
        if !self.check_address(&circuit.service_session_id) {
            return Err(HypergridError::VerificationFailed {
                session_id: circuit.session_id,
                reason: "Service-session token does not match this grid".into(),
            });
        }

        if circuit.ip_address.is_empty() {
            return Err(HypergridError::VerificationFailed {
                session_id: circuit.session_id,
                reason: "Agent did not provide a client IP address".into(),
            });
        }

        // -- Foreign agent allow/deny with exceptions --
        // We treat all agents as foreign for now (no local user-account
        // lookup wired yet).  In the full implementation this block only runs
        // when the agent is not a local account.
        {
            let home_uri = circuit
                .service_urls
                .get("HomeURI")
                .map(String::as_str)
                .unwrap_or("");

            let mut allowed = self.config.foreign_agents_allowed;

            if self.config.foreign_agents_allowed
                && Self::is_exception(home_uri, &self.config.foreigns_allowed_exceptions)
            {
                allowed = false;
            }

            if !self.config.foreign_agents_allowed
                && Self::is_exception(home_uri, &self.config.foreigns_disallowed_exceptions)
            {
                allowed = true;
            }

            if !allowed {
                return Err(HypergridError::AccessDenied(
                    "Destination does not allow visitors from your world".into(),
                ));
            }
        }

        // NOTE: the full implementation would additionally:
        //   - Call VerifyAgent on the home grid's UserAgentService
        //   - Check for UUID impersonation against local accounts
        //   - Check BansService
        //   - Create a presence record
        //   - Call SimulationService.CreateAgent to launch the agent
        // These require wiring up additional service dependencies.

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// HTTP handler (axum)
// ---------------------------------------------------------------------------

/// Shared state for the gatekeeper HTTP handler.
#[derive(Clone)]
pub struct GatekeeperHandlerState {
    pub service: Arc<GatekeeperServiceImpl>,
}

/// Build an axum [`Router`] that exposes the gatekeeper's XML-RPC endpoints.
pub fn router(state: GatekeeperHandlerState) -> Router {
    Router::new()
        .route("/xmlrpc/gatekeeper", post(handle_xmlrpc))
        .with_state(state)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Build an XML-RPC response wrapping a `<params><param><value><struct>` of
/// `<member>` elements.
fn xmlrpc_response(members: &[(&str, &str)]) -> Response {
    let mut body = String::from(
        "<?xml version=\"1.0\"?>\
         <methodResponse><params><param><value><struct>",
    );
    for (name, value) in members {
        body.push_str("<member><name>");
        body.push_str(&xml_escape(name));
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

/// Extract a named `<string>` value from within a `<struct>` in an XML-RPC
/// request body.  This is a simplified parser that handles the common case.
fn extract_xmlrpc_string(xml: &str, name: &str) -> Option<String> {
    // Look for <member><name>NAME</name><value>...<string>VALUE</string>...
    // This is intentionally simple; a real XML parser is overkill for the
    // handful of fields we need.
    let name_tag = format!("<name>{name}</name>");
    let idx = xml.find(&name_tag)?;
    let after = &xml[idx + name_tag.len()..];

    // Try <value><string>...</string>
    if let Some(start) = after.find("<string>") {
        let start = start + "<string>".len();
        let rest = &after[start..];
        if let Some(end) = rest.find("</string>") {
            return Some(rest[..end].to_string());
        }
    }

    // Try <value>...</value> (plain text without <string> wrapper)
    if let Some(start) = after.find("<value>") {
        let start = start + "<value>".len();
        let rest = &after[start..];
        if let Some(end) = rest.find("</value>") {
            let text = rest[..end].trim();
            // Strip <string> tags if present
            let text = text.strip_prefix("<string>").unwrap_or(text);
            let text = text.strip_suffix("</string>").unwrap_or(text);
            return Some(text.to_string());
        }
    }

    None
}

/// Detect which XML-RPC method is being called.
fn extract_xmlrpc_method(xml: &str) -> Option<String> {
    let start = xml.find("<methodName>")? + "<methodName>".len();
    let rest = &xml[start..];
    let end = rest.find("</methodName>")?;
    Some(rest[..end].trim().to_string())
}

async fn handle_xmlrpc(State(state): State<GatekeeperHandlerState>, body: Bytes) -> Response {
    let xml = String::from_utf8_lossy(&body);
    let method = extract_xmlrpc_method(&xml).unwrap_or_default();

    match method.as_str() {
        "link_region" => handle_link_region(&state, &xml),
        "get_region" => handle_get_region(&state, &xml),
        _ => {
            tracing::warn!(method, "unknown gatekeeper XML-RPC method");
            xmlrpc_response(&[("result", "false")])
        }
    }
}

fn handle_link_region(state: &GatekeeperHandlerState, xml: &str) -> Response {
    let region_name = extract_xmlrpc_string(xml, "region_name").unwrap_or_default();

    match state.service.link_local_region(&region_name) {
        Ok(info) => {
            let handle_str = info.region_handle.to_string();
            let size_x = info.size_x.to_string();
            let size_y = info.size_y.to_string();
            let uuid = info.region_id.to_string();
            xmlrpc_response(&[
                ("result", "true"),
                ("uuid", &uuid),
                ("handle", &handle_str),
                ("size_x", &size_x),
                ("size_y", &size_y),
                ("region_image", &info.image_url),
                ("external_name", &info.external_name),
            ])
        }
        Err(e) => {
            tracing::warn!(error = %e, "link_region failed");
            xmlrpc_response(&[("result", "false")])
        }
    }
}

fn handle_get_region(state: &GatekeeperHandlerState, xml: &str) -> Response {
    let region_id_str = extract_xmlrpc_string(xml, "region_uuid").unwrap_or_default();
    let region_id = region_id_str.parse::<Uuid>().unwrap_or(Uuid::nil());

    let agent_id_str = extract_xmlrpc_string(xml, "agent_id").unwrap_or_default();
    let agent_id = agent_id_str.parse::<Uuid>().unwrap_or(Uuid::nil());

    let agent_home_uri = extract_xmlrpc_string(xml, "agent_home_uri");

    match state
        .service
        .get_hyperlink_region(region_id, agent_id, agent_home_uri.as_deref())
    {
        Ok((region, message)) => {
            let uuid = region.region_id.to_string();
            let x = region.region_loc_x.to_string();
            let y = region.region_loc_y.to_string();
            let sx = region.region_size_x.to_string();
            let sy = region.region_size_y.to_string();
            let port = region.internal_port.to_string();

            let mut members: Vec<(&str, &str)> = vec![
                ("result", "true"),
                ("uuid", &uuid),
                ("x", &x),
                ("y", &y),
                ("size_x", &sx),
                ("size_y", &sy),
                ("region_name", &region.region_name),
                ("hostname", &region.external_host_name),
                ("internal_port", &port),
                ("server_uri", &region.server_uri),
            ];
            if let Some(ref msg) = message {
                members.push(("message", msg));
            }
            xmlrpc_response(&members)
        }
        Err(e) => {
            let msg = e.to_string();
            xmlrpc_response(&[("result", "false"), ("message", &msg)])
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Helper: build a default config for testing.
    fn test_config() -> GatekeeperConfig {
        GatekeeperConfig {
            gatekeeper_url: "http://mygrid.example:9000/".into(),
            gatekeeper_aliases: vec!["http://mygrid-alias.example:9000/".into()],
            scope_id: Uuid::nil(),
            allow_teleports_to_any_region: true,
            foreign_agents_allowed: true,
            foreigns_allowed_exceptions: Vec::new(),
            foreigns_disallowed_exceptions: Vec::new(),
            allowed_clients: None,
            denied_clients: None,
            denied_macs: String::new(),
            denied_id0s: String::new(),
        }
    }

    /// Helper: build a default region for testing.
    fn test_region() -> GridRegionInfo {
        GridRegionInfo {
            region_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            region_name: "Test Region".into(),
            region_handle: 1099511628032,
            server_uri: "http://mygrid.example:9000/".into(),
            external_host_name: "mygrid.example".into(),
            internal_port: 9000,
            region_loc_x: 256000,
            region_loc_y: 256000,
            region_size_x: 256,
            region_size_y: 256,
        }
    }

    /// Helper: build a minimal agent circuit for testing.
    fn test_circuit() -> AgentCircuitInfo {
        let mut service_urls = HashMap::new();
        service_urls.insert("HomeURI".into(), "http://foreigngrid.example:9000/".into());

        AgentCircuitInfo {
            agent_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            secure_session_id: Uuid::new_v4(),
            first_name: "Test".into(),
            last_name: "User".into(),
            start_pos: Vector3::default(),
            service_session_id: format!("http://mygrid.example:9000/;{}", Uuid::new_v4()),
            service_urls,
            ip_address: "192.168.1.100".into(),
            viewer: "Firestorm 6.5.0".into(),
            channel: "Firestorm-Release".into(),
            mac: "aa:bb:cc:dd:ee:ff".into(),
            id0: "test-id0".into(),
            teleport_flags: 0,
        }
    }

    #[test]
    fn test_link_local_region_default() {
        let svc = GatekeeperServiceImpl::new(test_config(), Some(test_region()));
        let result = svc.link_local_region("").unwrap();
        assert_eq!(result.region_id, test_region().region_id);
        // external_name should just be the gatekeeper URL when name is empty
        assert_eq!(result.external_name, "http://mygrid.example:9000/");
        assert!(result.image_url.contains("regionImage"));
    }

    #[test]
    fn test_check_address_valid() {
        let svc = GatekeeperServiceImpl::new(test_config(), None);
        let token = format!("http://mygrid.example:9000/;{}", Uuid::new_v4());
        assert!(svc.check_address(&token));
    }

    #[test]
    fn test_check_address_valid_alias() {
        let svc = GatekeeperServiceImpl::new(test_config(), None);
        let token = format!("http://mygrid-alias.example:9000/;{}", Uuid::new_v4());
        assert!(svc.check_address(&token));
    }

    #[test]
    fn test_check_address_invalid() {
        let svc = GatekeeperServiceImpl::new(test_config(), None);
        let token = format!("http://evil.example:9000/;{}", Uuid::new_v4());
        assert!(!svc.check_address(&token));
    }

    #[test]
    fn test_check_address_malformed() {
        let svc = GatekeeperServiceImpl::new(test_config(), None);
        assert!(!svc.check_address("no-semicolon-here"));
    }

    #[test]
    fn test_denied_client() {
        let mut config = test_config();
        config.denied_clients = Some(Regex::new("(?i)copybot").unwrap());
        let svc = GatekeeperServiceImpl::new(config, Some(test_region()));

        let mut circuit = test_circuit();
        circuit.viewer = "CopyBot 1.0".into();
        circuit.channel = "".into();

        let result = svc.login_agent(None, &circuit, &test_region());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("denied"), "expected denied error, got: {err}");
    }

    #[test]
    fn test_allowed_client() {
        let mut config = test_config();
        config.allowed_clients = Some(Regex::new("(?i)firestorm").unwrap());
        let svc = GatekeeperServiceImpl::new(config, Some(test_region()));

        // Matching viewer: should succeed
        let circuit = test_circuit();
        assert!(svc.login_agent(None, &circuit, &test_region()).is_ok());

        // Non-matching viewer: should be denied
        let mut bad_circuit = test_circuit();
        bad_circuit.viewer = "UnknownViewer 1.0".into();
        bad_circuit.channel = "".into();
        let result = svc.login_agent(None, &bad_circuit, &test_region());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not allowed"),
            "expected 'not allowed' error, got: {err}"
        );
    }

    #[test]
    fn test_foreign_agent_exception() {
        // Foreign agents allowed, but with an exception for a specific grid
        let mut config = test_config();
        config.foreigns_allowed_exceptions = vec!["http://banned-grid.example:9000/".into()];
        let svc = GatekeeperServiceImpl::new(config, Some(test_region()));

        // Agent from the banned grid
        let mut circuit = test_circuit();
        circuit
            .service_urls
            .insert("HomeURI".into(), "http://banned-grid.example:9000/".into());
        let result = svc.login_agent(None, &circuit, &test_region());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("does not allow visitors"),
            "expected foreign deny, got: {err}"
        );

        // Agent from a different grid: should succeed
        let ok_circuit = test_circuit();
        assert!(svc.login_agent(None, &ok_circuit, &test_region()).is_ok());
    }

    #[test]
    fn test_denied_macs() {
        let mut config = test_config();
        config.denied_macs = "aa:bb:cc:dd:ee:ff,11:22:33:44:55:66".into();
        let svc = GatekeeperServiceImpl::new(config, Some(test_region()));

        let circuit = test_circuit();
        let result = svc.login_agent(None, &circuit, &test_region());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Mac"), "expected Mac denied error, got: {err}");
    }

    #[test]
    fn test_login_agent_basic() {
        let svc = GatekeeperServiceImpl::new(test_config(), Some(test_region()));
        let circuit = test_circuit();
        let result = svc.login_agent(None, &circuit, &test_region());
        assert!(result.is_ok(), "basic login should succeed: {result:?}");
    }
}
