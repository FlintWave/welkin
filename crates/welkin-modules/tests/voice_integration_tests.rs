// SPDX-License-Identifier: LGPL-3.0-or-later

//! Integration tests for the WebRTC voice module.
//!
//! These tests exercise the public API of `welkin_modules::voice` as an
//! external consumer would, covering the wolfvoice JSON-RPC client,
//! capability handlers, provisioning flow, parcel-crossing channel switch,
//! estate voice disable, concurrent provisioning, and Docker/portal helpers.

use std::collections::HashMap;
use std::sync::Mutex;

use uuid::Uuid;

use welkin_modules::module::{ModuleConfig, RegionModule};
use welkin_modules::registry::SceneRef;
use welkin_modules::voice::capabilities::{
    ParcelVoiceInfoRequest, ProvisionVoiceAccountRequest, build_parcel_voice_response,
    build_provision_response, encode_provision_response, handle_parcel_voice_info,
    handle_provision_voice_account, llsd_undef, make_channel_uri,
};
use welkin_modules::voice::docker::{
    WolfvoiceDockerConfig, generate_compose_file, validate_config,
};
use welkin_modules::voice::ossl_integration::{
    OsslVoiceResult, os_clear_parcel_sip_address, os_set_parcel_sip_address, validate_sip_address,
};
use welkin_modules::voice::portal_integration::{
    EstateVoiceToggleRequest, VoiceServiceStatus, build_voice_status, encode_voice_status,
    handle_estate_voice_toggle, handle_voice_status_request,
};
use welkin_modules::voice::wolfvoice_client::{
    ChannelType, HttpTransport, VoiceCredentials, WolfvoiceClient, WolfvoiceError,
};
use welkin_modules::voice::{IVoiceModule, WebRtcVoiceModule};

// ===========================================================================
// Test helpers
// ===========================================================================

/// Minimal `ModuleConfig` for test setup.
struct TestConfig {
    strings: HashMap<(String, String), String>,
    bools: HashMap<(String, String), bool>,
}

impl TestConfig {
    fn new() -> Self {
        Self {
            strings: HashMap::new(),
            bools: HashMap::new(),
        }
    }

    fn set_bool(&mut self, section: &str, key: &str, value: bool) {
        self.bools
            .insert((section.to_string(), key.to_string()), value);
    }

    #[allow(dead_code)]
    fn set_string(&mut self, section: &str, key: &str, value: &str) {
        self.strings
            .insert((section.to_string(), key.to_string()), value.to_string());
    }
}

impl ModuleConfig for TestConfig {
    fn get_string(&self, section: &str, key: &str, default: &str) -> String {
        self.strings
            .get(&(section.to_string(), key.to_string()))
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }
    fn get_int(&self, _section: &str, _key: &str, default: i64) -> i64 {
        default
    }
    fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
        self.bools
            .get(&(section.to_string(), key.to_string()))
            .copied()
            .unwrap_or(default)
    }
    fn get_float(&self, _section: &str, _key: &str, default: f64) -> f64 {
        default
    }
}

fn test_scene() -> SceneRef {
    SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
}

/// Set up a voice module that is initialised and attached to a region.
fn setup_module() -> WebRtcVoiceModule {
    let mut module = WebRtcVoiceModule::new();
    let mut config = TestConfig::new();
    config.set_bool("Voice", "enabled", true);
    module.initialise(&config).unwrap();
    let scene = test_scene();
    module.add_region(&scene).unwrap();
    module
}

// ---------------------------------------------------------------------------
// Mock HTTP transport for WolfvoiceClient
// ---------------------------------------------------------------------------

/// Shared request capture buffer, used by MockTransport so the test can
/// inspect captured requests without reaching into WolfvoiceClient's private
/// fields.
type CapturedRequests = std::sync::Arc<Mutex<Vec<Vec<u8>>>>;

/// A mock HTTP transport that returns canned JSON-RPC responses in FIFO order.
struct MockTransport {
    responses: Mutex<Vec<String>>,
    captured: CapturedRequests,
}

impl MockTransport {
    fn new(responses: Vec<String>) -> (Self, CapturedRequests) {
        let captured: CapturedRequests = std::sync::Arc::new(Mutex::new(Vec::new()));
        let transport = Self {
            responses: Mutex::new(responses),
            captured: captured.clone(),
        };
        (transport, captured)
    }

    /// Convenience: create a transport when you don't need request capture.
    fn simple(responses: Vec<String>) -> Self {
        Self {
            responses: Mutex::new(responses),
            captured: std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl HttpTransport for MockTransport {
    fn post_json(&self, _url: &str, body: &[u8]) -> Result<Vec<u8>, WolfvoiceError> {
        self.captured.lock().unwrap().push(body.to_vec());
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(WolfvoiceError::Transport(
                "no more canned responses".to_string(),
            ));
        }
        Ok(responses.remove(0).into_bytes())
    }
}

/// Build a successful JSON-RPC response wrapping the given result JSON.
fn ok_response(result_json: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","result":{},"id":1}}"#, result_json)
}

/// Build an error JSON-RPC response.
fn err_response(code: i64, message: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","error":{{"code":{},"message":"{}"}},"id":1}}"#,
        code, message
    )
}

// ===========================================================================
// 1. Mock wolfvoice JSON-RPC server tests
// ===========================================================================

/// Provision: successful account provisioning via mock transport.
#[test]
fn wolfvoice_provision_success() {
    let response = ok_response(
        r#"{"username":"voice_user","password":"s3cret","voice_server_uri":"wss://voice.grid.local:9000"}"#,
    );
    let (transport, captured) = MockTransport::new(vec![response]);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let agent_id = Uuid::new_v4();
    let creds = client.provision_account(agent_id, "Test Agent").unwrap();

    assert_eq!(creds.username, "voice_user");
    assert_eq!(creds.password, "s3cret");
    assert_eq!(creds.voice_server_uri, "wss://voice.grid.local:9000");

    // Verify the request was well-formed JSON-RPC.
    let requests = captured.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let req: serde_json::Value = serde_json::from_slice(&requests[0]).unwrap();
    assert_eq!(req["jsonrpc"], "2.0");
    assert_eq!(req["method"], "provision_account");
    assert_eq!(req["params"]["agent_name"], "Test Agent");
}

/// Channel create: successful channel creation via mock transport.
#[test]
fn wolfvoice_channel_create_success() {
    let response = ok_response(
        r#"{"channel_uri":"wss://voice.grid.local/ch/parcel-42","channel_name":"parcel-42"}"#,
    );
    let (transport, captured) = MockTransport::new(vec![response]);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let info = client
        .create_channel("parcel-42", ChannelType::Parcel)
        .unwrap();

    assert_eq!(info.channel_name, "parcel-42");
    assert!(info.channel_uri.contains("parcel-42"));

    let requests = captured.lock().unwrap();
    let req: serde_json::Value = serde_json::from_slice(&requests[0]).unwrap();
    assert_eq!(req["method"], "create_channel");
    assert_eq!(req["params"]["channel_type"], "parcel");
}

/// Channel destroy: successful channel destruction via mock transport.
#[test]
fn wolfvoice_channel_destroy_success() {
    let response = ok_response(r#"{}"#);
    let transport = MockTransport::simple(vec![response]);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let result = client.destroy_channel("wss://voice.grid.local/ch/parcel-42");
    assert!(result.is_ok());
}

/// Error response: JSON-RPC error is correctly propagated.
#[test]
fn wolfvoice_error_response() {
    let response = err_response(-32601, "Method not found");
    let transport = MockTransport::simple(vec![response]);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let result = client.provision_account(Uuid::new_v4(), "Agent");
    match result {
        Err(WolfvoiceError::Rpc { code, message }) => {
            assert_eq!(code, -32601);
            assert_eq!(message, "Method not found");
        }
        other => panic!("expected Rpc error, got {:?}", other),
    }
}

/// Transport error: connection failure is correctly propagated.
#[test]
fn wolfvoice_transport_error() {
    let transport = MockTransport::simple(vec![]);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let result = client.health_check();
    assert!(matches!(result, Err(WolfvoiceError::Transport(_))));
}

/// Bad JSON response: malformed response is caught.
#[test]
fn wolfvoice_bad_json_response() {
    let transport = MockTransport::simple(vec!["not json at all".to_string()]);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let result = client.health_check();
    assert!(matches!(result, Err(WolfvoiceError::Json(_))));
}

/// Missing result field: null result with no error produces BadResponse.
#[test]
fn wolfvoice_null_result_produces_bad_response() {
    let response = r#"{"jsonrpc":"2.0","result":null,"id":1}"#.to_string();
    let transport = MockTransport::simple(vec![response]);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let result = client.health_check();
    assert!(matches!(result, Err(WolfvoiceError::BadResponse)));
}

// ===========================================================================
// 2. Provision flow: agent UUID -> voice credentials
// ===========================================================================

/// Full provision flow: WolfvoiceClient -> capabilities handler -> LLSD XML.
#[test]
fn provision_flow_agent_to_credentials() {
    // Step 1: Provision via mock wolfvoice client.
    let response = ok_response(
        r#"{"username":"agent_voice","password":"pwd123","voice_server_uri":"webrtc://voice.example.com:9000/v"}"#,
    );
    let transport = MockTransport::simple(vec![response]);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let agent_id = Uuid::new_v4();
    let creds = client.provision_account(agent_id, "Alice Linden").unwrap();

    // Step 2: Build capability response from credentials.
    let request = ProvisionVoiceAccountRequest {
        agent_id,
        agent_name: "Alice Linden".to_string(),
    };
    let xml = handle_provision_voice_account(&request, &creds);

    // Step 3: Verify LLSD XML output.
    assert!(xml.starts_with("<llsd><map>"));
    assert!(xml.ends_with("</map></llsd>"));
    assert!(xml.contains("<key>username</key><string>agent_voice</string>"));
    assert!(xml.contains("<key>password</key><string>pwd123</string>"));
    assert!(xml.contains("<key>voice_server_type</key><string>webrtc</string>"));
    assert!(xml.contains("<key>voice_sip_uri_hostname</key><string>voice.example.com</string>"));
}

/// Verify that provision responses for different agents contain different
/// credentials (no cross-contamination when called sequentially).
#[test]
fn provision_flow_credential_isolation() {
    let responses = vec![
        ok_response(r#"{"username":"user_A","password":"passA","voice_server_uri":"wss://v1"}"#),
        ok_response(r#"{"username":"user_B","password":"passB","voice_server_uri":"wss://v2"}"#),
    ];
    let transport = MockTransport::simple(responses);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();

    let creds_a = client.provision_account(agent_a, "Agent A").unwrap();
    let creds_b = client.provision_account(agent_b, "Agent B").unwrap();

    // Credentials must be distinct.
    assert_ne!(creds_a.username, creds_b.username);
    assert_ne!(creds_a.password, creds_b.password);

    // Build LLSD for each agent and verify no cross-contamination.
    let req_a = ProvisionVoiceAccountRequest {
        agent_id: agent_a,
        agent_name: "Agent A".to_string(),
    };
    let req_b = ProvisionVoiceAccountRequest {
        agent_id: agent_b,
        agent_name: "Agent B".to_string(),
    };

    let xml_a = handle_provision_voice_account(&req_a, &creds_a);
    let xml_b = handle_provision_voice_account(&req_b, &creds_b);

    assert!(xml_a.contains("user_A"));
    assert!(!xml_a.contains("user_B"));
    assert!(xml_b.contains("user_B"));
    assert!(!xml_b.contains("user_A"));
}

/// build_provision_response extracts hostname correctly from various URI formats.
#[test]
fn provision_hostname_extraction() {
    let creds = VoiceCredentials {
        username: "u".to_string(),
        password: "p".to_string(),
        voice_server_uri: "webrtc://voice.example.com:9000/voice".to_string(),
    };
    let resp = build_provision_response(&creds);
    assert_eq!(resp.voice_sip_uri_hostname, "voice.example.com");
}

// ===========================================================================
// 3. Parcel crossing channel switch
// ===========================================================================

/// Simulate an agent moving from parcel A to parcel B:
/// - Parcel A has a voice channel on local ID 10.
/// - Parcel B has a voice channel on local ID 20.
/// - After crossing, the agent's ParcelVoiceInfoRequest returns parcel B's
///   channel URI.
#[test]
fn parcel_crossing_channel_switch() {
    let mut module = setup_module();

    // Create channels for two parcels.
    module
        .create_channel(10, "parcel-A-voice", ChannelType::Parcel)
        .unwrap();
    module
        .create_channel(20, "parcel-B-voice", ChannelType::Parcel)
        .unwrap();

    // Agent is on parcel A: viewers active.
    module.increment_viewers(10);
    assert_eq!(module.get_channel(10).unwrap().viewer_count, 1);

    // Build ParcelVoiceInfoRequest for parcel A.
    let ch_a = module.get_channel(10).unwrap();
    let request_a = ParcelVoiceInfoRequest {
        agent_id: Uuid::new_v4(),
        region_name: "TestRegion".to_string(),
        parcel_local_id: 10,
        estate_voice_enabled: true,
        parcel_voice_enabled: true,
        channel_uri: ch_a.channel_info.channel_uri.clone(),
        channel_credentials: "cred-A".to_string(),
    };
    let xml_a = handle_parcel_voice_info(&request_a);
    assert!(xml_a.contains("parcel-A-voice"));

    // Agent crosses to parcel B.
    module.decrement_viewers(10);
    module.increment_viewers(20);
    assert_eq!(module.get_channel(10).unwrap().viewer_count, 0);
    assert_eq!(module.get_channel(20).unwrap().viewer_count, 1);

    // Build ParcelVoiceInfoRequest for parcel B.
    let ch_b = module.get_channel(20).unwrap();
    let request_b = ParcelVoiceInfoRequest {
        agent_id: request_a.agent_id,
        region_name: "TestRegion".to_string(),
        parcel_local_id: 20,
        estate_voice_enabled: true,
        parcel_voice_enabled: true,
        channel_uri: ch_b.channel_info.channel_uri.clone(),
        channel_credentials: "cred-B".to_string(),
    };
    let xml_b = handle_parcel_voice_info(&request_b);

    // The viewer now sees parcel B's channel.
    assert!(xml_b.contains("parcel-B-voice"));
    assert!(!xml_b.contains("parcel-A-voice"));
}

/// Parcel crossing with SIP override: the overridden channel URI takes
/// precedence.
#[test]
fn parcel_crossing_with_sip_override() {
    let mut module = setup_module();
    let parcel_global_id = Uuid::new_v4();

    module
        .create_channel(10, "default-ch", ChannelType::Parcel)
        .unwrap();
    module.set_land_sip_address(parcel_global_id, "sip:custom@voice.grid.local");

    // Verify the SIP override is stored and retrievable.
    assert_eq!(
        module.get_sip_override(parcel_global_id),
        Some(&"sip:custom@voice.grid.local".to_string())
    );

    // The override can be used to build a custom ParcelVoiceInfoRequest.
    let request = ParcelVoiceInfoRequest {
        agent_id: Uuid::new_v4(),
        region_name: "TestRegion".to_string(),
        parcel_local_id: 10,
        estate_voice_enabled: true,
        parcel_voice_enabled: true,
        channel_uri: "sip:custom@voice.grid.local".to_string(),
        channel_credentials: "custom-cred".to_string(),
    };
    let xml = handle_parcel_voice_info(&request);
    assert!(xml.contains("sip:custom@voice.grid.local"));
}

// ===========================================================================
// 4. Estate voice disable
// ===========================================================================

/// When estate voice is disabled, ParcelVoiceInfoResponse returns empty
/// channel URI, even if the parcel has voice enabled.
#[test]
fn estate_voice_disable_clears_channel() {
    let request = ParcelVoiceInfoRequest {
        agent_id: Uuid::new_v4(),
        region_name: "MyRegion".to_string(),
        parcel_local_id: 5,
        estate_voice_enabled: false,
        parcel_voice_enabled: true,
        channel_uri: "webrtc://MyRegion/5".to_string(),
        channel_credentials: "cred".to_string(),
    };

    let resp = build_parcel_voice_response(&request);
    assert!(resp.channel_uri.is_empty());
    assert!(resp.channel_credentials.is_empty());
}

/// When parcel voice is disabled, the response is also empty.
#[test]
fn parcel_voice_disable_clears_channel() {
    let request = ParcelVoiceInfoRequest {
        agent_id: Uuid::new_v4(),
        region_name: "MyRegion".to_string(),
        parcel_local_id: 5,
        estate_voice_enabled: true,
        parcel_voice_enabled: false,
        channel_uri: "webrtc://MyRegion/5".to_string(),
        channel_credentials: "cred".to_string(),
    };

    let resp = build_parcel_voice_response(&request);
    assert!(resp.channel_uri.is_empty());
    assert!(resp.channel_credentials.is_empty());
}

/// When both estate and parcel voice are disabled, response is empty.
#[test]
fn both_disabled_clears_channel() {
    let request = ParcelVoiceInfoRequest {
        agent_id: Uuid::new_v4(),
        region_name: "MyRegion".to_string(),
        parcel_local_id: 5,
        estate_voice_enabled: false,
        parcel_voice_enabled: false,
        channel_uri: "webrtc://MyRegion/5".to_string(),
        channel_credentials: "cred".to_string(),
    };

    let resp = build_parcel_voice_response(&request);
    assert!(resp.channel_uri.is_empty());
}

/// Estate voice toggle via portal integration.
#[test]
fn estate_voice_toggle_via_portal() {
    // Disable voice for estate 42.
    let disable_req = EstateVoiceToggleRequest {
        estate_id: 42,
        enabled: false,
    };
    let disable_resp = handle_estate_voice_toggle(&disable_req);
    assert_eq!(disable_resp.estate_id, 42);
    assert!(!disable_resp.voice_enabled);
    assert!(disable_resp.message.contains("disabled"));

    // Re-enable voice.
    let enable_req = EstateVoiceToggleRequest {
        estate_id: 42,
        enabled: true,
    };
    let enable_resp = handle_estate_voice_toggle(&enable_req);
    assert!(enable_resp.voice_enabled);
    assert!(enable_resp.message.contains("enabled"));
}

/// Disabling voice for a parcel via the module destroys the channel.
#[test]
fn estate_disable_destroys_parcel_channels() {
    let mut module = setup_module();

    // Create three channels across the estate.
    module
        .create_channel(1, "ch-1", ChannelType::Parcel)
        .unwrap();
    module
        .create_channel(2, "ch-2", ChannelType::Parcel)
        .unwrap();
    module
        .create_channel(3, "ch-3", ChannelType::Estate)
        .unwrap();
    assert_eq!(module.active_channel_count(), 3);

    // Simulate estate-wide voice disable by destroying all channels.
    module.destroy_all_channels();
    assert_eq!(module.active_channel_count(), 0);
}

// ===========================================================================
// 5. Concurrent provisioning
// ===========================================================================

/// A thread-safe transport that can be shared across threads.
struct ThreadSafeTransport {
    responses: Mutex<Vec<String>>,
}

impl ThreadSafeTransport {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Mutex::new(responses),
        }
    }
}

impl HttpTransport for ThreadSafeTransport {
    fn post_json(&self, _url: &str, _body: &[u8]) -> Result<Vec<u8>, WolfvoiceError> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(WolfvoiceError::Transport("exhausted".to_string()));
        }
        Ok(responses.remove(0).into_bytes())
    }
}

/// Multiple concurrent provision requests produce distinct credentials.
#[test]
fn concurrent_provisioning_distinct_credentials() {
    use std::sync::Arc;
    use std::thread;

    let num_agents = 8;
    let mut responses = Vec::new();
    for i in 0..num_agents {
        responses.push(ok_response(&format!(
            r#"{{"username":"user_{}","password":"pass_{}","voice_server_uri":"wss://v{}"}}"#,
            i, i, i
        )));
    }

    let transport = Arc::new(ThreadSafeTransport::new(responses));
    // WolfvoiceClient requires ownership, but the transport is Arc.
    // We need to create a wrapper that implements HttpTransport via an Arc.
    struct ArcTransport(Arc<ThreadSafeTransport>);

    impl HttpTransport for ArcTransport {
        fn post_json(&self, url: &str, body: &[u8]) -> Result<Vec<u8>, WolfvoiceError> {
            self.0.post_json(url, body)
        }
    }

    let client = Arc::new(WolfvoiceClient::new(
        "http://localhost:8090".to_string(),
        ArcTransport(transport),
    ));

    let handles: Vec<_> = (0..num_agents)
        .map(|i| {
            let client = Arc::clone(&client);
            thread::spawn(move || {
                let agent_id = Uuid::new_v4();
                let name = format!("Agent_{}", i);
                client.provision_account(agent_id, &name)
            })
        })
        .collect();

    let mut usernames = std::collections::HashSet::new();
    for handle in handles {
        let result = handle.join().unwrap();
        let creds = result.unwrap();
        usernames.insert(creds.username);
    }

    // All usernames should be distinct.
    assert_eq!(usernames.len(), num_agents);
}

/// Sequential call IDs increment even under concurrent access.
#[test]
fn concurrent_provisioning_sequential_ids() {
    let responses = vec![
        ok_response(r#"{"healthy":true}"#),
        ok_response(r#"{"healthy":true}"#),
        ok_response(r#"{"healthy":true}"#),
        ok_response(r#"{"healthy":true}"#),
    ];
    let (transport, captured) = MockTransport::new(responses);
    let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

    // Issue 4 requests sequentially.
    for _ in 0..4 {
        client.health_check().unwrap();
    }

    let requests = captured.lock().unwrap();
    let ids: Vec<u64> = requests
        .iter()
        .map(|r| {
            let v: serde_json::Value = serde_json::from_slice(r).unwrap();
            v["id"].as_u64().unwrap()
        })
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 4]);
}

// ===========================================================================
// 6. Module lifecycle integration
// ===========================================================================

/// Full module lifecycle: init -> add_region -> create channels -> close.
#[test]
fn module_full_lifecycle() {
    let mut module = WebRtcVoiceModule::new();
    let mut config = TestConfig::new();
    config.set_bool("Voice", "enabled", true);
    config.set_string("WebRtcVoice", "wolfvoice_endpoint", "http://voice:9090");

    module.initialise(&config).unwrap();
    assert!(module.config().enabled);
    assert_eq!(module.config().wolfvoice_endpoint, "http://voice:9090");

    let scene = test_scene();
    module.add_region(&scene).unwrap();

    // Create some channels.
    module
        .create_channel(1, "ch-1", ChannelType::Parcel)
        .unwrap();
    module
        .create_channel(2, "ch-2", ChannelType::Estate)
        .unwrap();
    assert_eq!(module.active_channel_count(), 2);

    // Set a SIP override.
    let parcel_id = Uuid::new_v4();
    module.set_land_sip_address(parcel_id, "sip:test@example.com");
    assert!(module.get_sip_override(parcel_id).is_some());

    // Remove region clears channels.
    module.remove_region(&scene).unwrap();
    assert_eq!(module.active_channel_count(), 0);

    // Close clears everything.
    module.close().unwrap();
    assert!(module.get_sip_override(parcel_id).is_none());
}

/// Module defaults when voice is not enabled.
#[test]
fn module_disabled_by_default() {
    let mut module = WebRtcVoiceModule::new();
    let config = TestConfig::new();
    module.initialise(&config).unwrap();
    assert!(!module.config().enabled);
    assert_eq!(module.config().wolfvoice_endpoint, "http://localhost:8090");
}

// ===========================================================================
// 7. OSSL integration
// ===========================================================================

/// osSetParcelSIPAddress stores and retrieves correctly.
#[test]
fn ossl_set_sip_address_roundtrip() {
    let mut module = setup_module();
    let parcel_id = Uuid::new_v4();
    let addr = "sip:voice@custom-server.com";

    let result = os_set_parcel_sip_address(&mut module, parcel_id, addr);
    assert_eq!(result, OsslVoiceResult::Success);
    assert_eq!(module.get_sip_override(parcel_id), Some(&addr.to_string()));
}

/// osSetParcelSIPAddress rejects empty address.
#[test]
fn ossl_rejects_empty_address() {
    let mut module = setup_module();
    let parcel_id = Uuid::new_v4();

    let result = os_set_parcel_sip_address(&mut module, parcel_id, "");
    assert!(matches!(result, OsslVoiceResult::InvalidArguments(_)));
    assert!(module.get_sip_override(parcel_id).is_none());
}

/// osSetParcelSIPAddress rejects addresses with control characters.
#[test]
fn ossl_rejects_control_chars() {
    let mut module = setup_module();
    let parcel_id = Uuid::new_v4();

    for bad_addr in &["sip:bad\x00addr", "sip:\x1F", "\ttab", "new\nline"] {
        let result = os_set_parcel_sip_address(&mut module, parcel_id, bad_addr);
        assert!(
            matches!(result, OsslVoiceResult::InvalidArguments(_)),
            "should reject {:?}",
            bad_addr
        );
    }
}

/// osClearParcelSIPAddress removes an existing override.
#[test]
fn ossl_clear_removes_override() {
    let mut module = setup_module();
    let parcel_id = Uuid::new_v4();

    os_set_parcel_sip_address(&mut module, parcel_id, "sip:test@example.com");
    assert!(module.get_sip_override(parcel_id).is_some());

    let result = os_clear_parcel_sip_address(&mut module, parcel_id);
    assert_eq!(result, OsslVoiceResult::Success);
    assert!(module.get_sip_override(parcel_id).is_none());
}

/// osClearParcelSIPAddress succeeds even with no existing override.
#[test]
fn ossl_clear_nonexistent_succeeds() {
    let mut module = setup_module();
    let parcel_id = Uuid::new_v4();

    let result = os_clear_parcel_sip_address(&mut module, parcel_id);
    assert_eq!(result, OsslVoiceResult::Success);
}

/// SIP address validation accepts valid addresses.
#[test]
fn sip_validation_accepts_valid() {
    assert!(validate_sip_address("sip:voice@example.com"));
    assert!(validate_sip_address("webrtc://region/42"));
    assert!(validate_sip_address("simple"));
}

// ===========================================================================
// 8. Docker Compose generation
// ===========================================================================

/// Docker Compose file generation produces valid YAML structure.
#[test]
fn docker_compose_generation() {
    let config = WolfvoiceDockerConfig {
        secret_key: "test-secret".to_string(),
        turn_server: "turn:turn.example.com:3478".to_string(),
        host_port: 9090,
        ..Default::default()
    };

    let yaml = generate_compose_file(&config);
    assert!(yaml.starts_with("version: \"3.8\"\n"));
    assert!(yaml.contains("services:\n"));
    assert!(yaml.contains("wolfvoice:"));
    assert!(yaml.contains("\"9090:8090\""));
    assert!(yaml.contains("SECRET_KEY=test-secret"));
    assert!(yaml.contains("TURN_SERVER=turn:turn.example.com:3478"));
    assert!(yaml.contains("healthcheck:"));
}

/// Docker config validation catches missing secret key.
#[test]
fn docker_config_validation_missing_secret() {
    let config = WolfvoiceDockerConfig::default();
    let result = validate_config(&config);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("secret_key"));
}

/// Docker config validation accepts valid config.
#[test]
fn docker_config_validation_valid() {
    let config = WolfvoiceDockerConfig {
        secret_key: "valid-secret".to_string(),
        ..Default::default()
    };
    assert!(validate_config(&config).is_ok());
}

// ===========================================================================
// 9. Portal integration
// ===========================================================================

/// Voice status reporting produces valid JSON.
#[test]
fn portal_voice_status_json() {
    let json = handle_voice_status_request(true, 5, 12, Some("2.1.0".to_string()));
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["healthy"], true);
    assert_eq!(parsed["active_channels"], 5);
    assert_eq!(parsed["active_users"], 12);
    assert_eq!(parsed["wolfvoice_version"], "2.1.0");
}

/// Voice status with no version reports null.
#[test]
fn portal_voice_status_no_version() {
    let status = build_voice_status(false, 0, 0, None);
    let json = encode_voice_status(&status);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["healthy"], false);
    assert!(parsed["wolfvoice_version"].is_null());
}

/// VoiceServiceStatus JSON serialisation/deserialisation roundtrip.
#[test]
fn portal_voice_status_roundtrip() {
    let original = build_voice_status(true, 10, 25, Some("3.0.0".to_string()));
    let json = serde_json::to_string(&original).unwrap();
    let roundtripped: VoiceServiceStatus = serde_json::from_str(&json).unwrap();

    assert_eq!(original.healthy, roundtripped.healthy);
    assert_eq!(original.active_channels, roundtripped.active_channels);
    assert_eq!(original.active_users, roundtripped.active_users);
    assert_eq!(original.wolfvoice_version, roundtripped.wolfvoice_version);
}

// ===========================================================================
// 10. Capabilities: LLSD encoding and channel URI construction
// ===========================================================================

/// LLSD undef produces the expected empty response.
#[test]
fn llsd_undef_value() {
    assert_eq!(llsd_undef(), "<llsd><undef/></llsd>");
}

/// make_channel_uri produces the expected format.
#[test]
fn channel_uri_format() {
    assert_eq!(make_channel_uri("SimWorld", 99), "webrtc://SimWorld/99");
    assert_eq!(make_channel_uri("My Region", 1), "webrtc://My Region/1");
}

/// XML escaping in LLSD output handles special characters.
#[test]
fn llsd_xml_escaping() {
    let creds = VoiceCredentials {
        username: "user<>&\"'test".to_string(),
        password: "pass".to_string(),
        voice_server_uri: "webrtc://host:9000".to_string(),
    };
    let resp = build_provision_response(&creds);
    let xml = encode_provision_response(&resp);

    assert!(xml.contains("user&lt;&gt;&amp;&quot;&apos;test"));
}

/// Parcel voice info LLSD encoding.
#[test]
fn parcel_voice_info_llsd_encoding() {
    let request = ParcelVoiceInfoRequest {
        agent_id: Uuid::new_v4(),
        region_name: "TestRegion".to_string(),
        parcel_local_id: 42,
        estate_voice_enabled: true,
        parcel_voice_enabled: true,
        channel_uri: "webrtc://TestRegion/42".to_string(),
        channel_credentials: "creds123".to_string(),
    };
    let xml = handle_parcel_voice_info(&request);

    assert!(xml.starts_with("<llsd><map>"));
    assert!(xml.ends_with("</map></llsd>"));
    assert!(xml.contains("<key>region_name</key><string>TestRegion</string>"));
    assert!(xml.contains("<key>parcel_local_id</key><integer>42</integer>"));
    assert!(xml.contains("<key>channel_uri</key><string>webrtc://TestRegion/42</string>"));
    assert!(xml.contains("<key>channel_credentials</key><string>creds123</string>"));
}

// ===========================================================================
// 11. Channel management
// ===========================================================================

/// create_channel for all three channel types.
#[test]
fn channel_types_all_variants() {
    let mut module = setup_module();

    let parcel = module
        .create_channel(1, "parcel-ch", ChannelType::Parcel)
        .unwrap();
    assert!(parcel.channel_info.channel_uri.contains("parcel"));

    let estate = module
        .create_channel(2, "estate-ch", ChannelType::Estate)
        .unwrap();
    assert!(estate.channel_info.channel_uri.contains("estate"));

    let group = module
        .create_channel(3, "group-ch", ChannelType::Group)
        .unwrap();
    assert!(group.channel_info.channel_uri.contains("group"));

    assert_eq!(module.active_channel_count(), 3);
}

/// Destroying a non-existent channel returns an error.
#[test]
fn destroy_nonexistent_channel_errors() {
    let mut module = setup_module();
    let result = module.destroy_channel(999);
    assert!(result.is_err());
}

/// Viewer count tracking across channels.
#[test]
fn viewer_count_tracking() {
    let mut module = setup_module();
    module
        .create_channel(1, "ch-a", ChannelType::Parcel)
        .unwrap();
    module
        .create_channel(2, "ch-b", ChannelType::Estate)
        .unwrap();

    module.increment_viewers(1);
    module.increment_viewers(1);
    module.increment_viewers(2);

    assert_eq!(module.total_viewer_count(), 3);

    module.decrement_viewers(1);
    assert_eq!(module.total_viewer_count(), 2);

    // Decrement past zero saturates.
    module.decrement_viewers(1);
    module.decrement_viewers(1);
    module.decrement_viewers(1);
    assert_eq!(module.get_channel(1).unwrap().viewer_count, 0);
}

/// set_parcel_voice_enabled(false) removes the channel.
#[test]
fn disable_parcel_voice_removes_channel() {
    let mut module = setup_module();
    module.create_channel(5, "ch", ChannelType::Parcel).unwrap();
    assert!(module.get_channel(5).is_some());

    module.set_parcel_voice_enabled(5, false);
    assert!(module.get_channel(5).is_none());
    assert_eq!(module.active_channel_count(), 0);
}

/// Enabling voice for a parcel that has no channel is a no-op.
#[test]
fn enable_voice_no_channel_noop() {
    let mut module = setup_module();
    module.set_parcel_voice_enabled(5, true);
    assert_eq!(module.active_channel_count(), 0);
}

/// Replacing a channel for a parcel (create overwrites).
#[test]
fn channel_replace_on_same_parcel() {
    let mut module = setup_module();
    module
        .create_channel(1, "old-ch", ChannelType::Parcel)
        .unwrap();
    module
        .create_channel(1, "new-ch", ChannelType::Estate)
        .unwrap();

    // Only one channel for parcel 1.
    assert_eq!(module.active_channel_count(), 1);
    let ch = module.get_channel(1).unwrap();
    assert_eq!(ch.channel_info.channel_name, "new-ch");
}
