// SPDX-License-Identifier: GPL-3.0-or-later

//! JSON-RPC 2.0 client for the wolfvoice voice sidecar.

use std::sync::atomic::{AtomicU64, Ordering};

use uuid::Uuid;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Credentials provisioned for an agent's voice session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VoiceCredentials {
    pub username: String,
    pub password: String,
    pub voice_server_uri: String,
}

/// Metadata for a voice channel managed by the wolfvoice sidecar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelInfo {
    pub channel_uri: String,
    pub channel_name: String,
}

/// The type of voice channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    Parcel,
    Estate,
    Group,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when communicating with the wolfvoice sidecar.
#[derive(Debug, thiserror::Error)]
pub enum WolfvoiceError {
    #[error("HTTP transport error: {0}")]
    Transport(String),
    #[error("JSON serialisation error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("unexpected response format")]
    BadResponse,
}

// ---------------------------------------------------------------------------
// JSON-RPC wire types (private)
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct JsonRpcRequest<P: serde::Serialize> {
    jsonrpc: &'static str,
    method: String,
    params: P,
    id: u64,
}

#[derive(serde::Deserialize)]
struct JsonRpcResponse<R> {
    #[allow(dead_code)]
    #[serde(default)]
    jsonrpc: Option<String>,
    result: Option<R>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: u64,
}

#[derive(serde::Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

// ---------------------------------------------------------------------------
// HttpTransport trait
// ---------------------------------------------------------------------------

/// Abstraction for HTTP POST, enabling test mocks.
///
/// Implementations must be thread-safe so the client can be shared across
/// async tasks.
pub trait HttpTransport: Send + Sync {
    /// Send a JSON body to the given URL and return the raw response bytes.
    fn post_json(&self, url: &str, body: &[u8]) -> Result<Vec<u8>, WolfvoiceError>;
}

// ---------------------------------------------------------------------------
// WolfvoiceClient
// ---------------------------------------------------------------------------

/// JSON-RPC 2.0 client for the wolfvoice voice sidecar.
///
/// Generic over the HTTP transport so it can be tested with mocks and
/// swapped for a real `reqwest`-based transport in production.
pub struct WolfvoiceClient<T: HttpTransport> {
    endpoint: String,
    transport: T,
    next_id: AtomicU64,
}

impl<T: HttpTransport> WolfvoiceClient<T> {
    /// Create a new client pointing at the given wolfvoice endpoint.
    pub fn new(endpoint: String, transport: T) -> Self {
        Self {
            endpoint,
            transport,
            next_id: AtomicU64::new(1),
        }
    }

    /// Internal helper: perform a JSON-RPC call and deserialise the result.
    fn call<P, R>(&self, method: &str, params: P) -> Result<R, WolfvoiceError>
    where
        P: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id,
        };
        let body = serde_json::to_vec(&request)?;
        let response_bytes = self.transport.post_json(&self.endpoint, &body)?;
        let response: JsonRpcResponse<R> = serde_json::from_slice(&response_bytes)?;

        if let Some(err) = response.error {
            return Err(WolfvoiceError::Rpc {
                code: err.code,
                message: err.message,
            });
        }

        response.result.ok_or(WolfvoiceError::BadResponse)
    }

    /// Provision voice credentials for an agent.
    pub fn provision_account(
        &self,
        agent_id: Uuid,
        agent_name: &str,
    ) -> Result<VoiceCredentials, WolfvoiceError> {
        #[derive(serde::Serialize)]
        struct Params<'a> {
            agent_id: Uuid,
            agent_name: &'a str,
        }
        self.call(
            "provision_account",
            Params {
                agent_id,
                agent_name,
            },
        )
    }

    /// Create a new voice channel on the sidecar.
    pub fn create_channel(
        &self,
        channel_name: &str,
        channel_type: ChannelType,
    ) -> Result<ChannelInfo, WolfvoiceError> {
        #[derive(serde::Serialize)]
        struct Params<'a> {
            channel_name: &'a str,
            channel_type: ChannelType,
        }
        self.call(
            "create_channel",
            Params {
                channel_name,
                channel_type,
            },
        )
    }

    /// Destroy an existing voice channel by its URI.
    pub fn destroy_channel(&self, channel_uri: &str) -> Result<(), WolfvoiceError> {
        #[derive(serde::Serialize)]
        struct Params<'a> {
            channel_uri: &'a str,
        }
        // The sidecar returns an empty object `{}` on success, which
        // deserialises to `serde_json::Value` with no issues.
        let _: serde_json::Value = self.call("destroy_channel", Params { channel_uri })?;
        Ok(())
    }

    /// Check whether the wolfvoice sidecar is healthy.
    ///
    /// Returns `Ok(true)` if the sidecar reports healthy, `Ok(false)` if
    /// it responds but reports unhealthy, and `Err` on transport/parse
    /// failures.
    pub fn health_check(&self) -> Result<bool, WolfvoiceError> {
        #[derive(serde::Deserialize)]
        struct HealthResult {
            healthy: bool,
        }
        #[derive(serde::Serialize)]
        struct EmptyParams {}
        let result: HealthResult = self.call("health_check", EmptyParams {})?;
        Ok(result.healthy)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // -- MockTransport -------------------------------------------------------

    /// A mock HTTP transport that returns canned JSON responses in FIFO order.
    struct MockTransport {
        responses: Mutex<Vec<String>>,
        /// Captured request bodies for assertion.
        captured_requests: Mutex<Vec<Vec<u8>>>,
    }

    impl MockTransport {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Mutex::new(responses),
                captured_requests: Mutex::new(Vec::new()),
            }
        }

        fn captured_requests(&self) -> Vec<Vec<u8>> {
            self.captured_requests.lock().unwrap().clone()
        }
    }

    impl HttpTransport for MockTransport {
        fn post_json(&self, _url: &str, body: &[u8]) -> Result<Vec<u8>, WolfvoiceError> {
            self.captured_requests.lock().unwrap().push(body.to_vec());
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

    // -- Test 1: provision_account success ------------------------------------

    #[test]
    fn provision_account_success() {
        let response = ok_response(
            r#"{"username":"agent_voice","password":"secret123","voice_server_uri":"wss://voice.example.com"}"#,
        );
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        let creds = client.provision_account(Uuid::nil(), "Test User").unwrap();
        assert_eq!(creds.username, "agent_voice");
        assert_eq!(creds.password, "secret123");
        assert_eq!(creds.voice_server_uri, "wss://voice.example.com");
    }

    // -- Test 2: create_channel parcel type -----------------------------------

    #[test]
    fn create_channel_parcel_type() {
        let response = ok_response(
            r#"{"channel_uri":"wss://voice.example.com/ch/parcel-1","channel_name":"parcel-1"}"#,
        );
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        let info = client
            .create_channel("parcel-1", ChannelType::Parcel)
            .unwrap();
        assert_eq!(info.channel_name, "parcel-1");
        assert!(info.channel_uri.contains("parcel-1"));
    }

    // -- Test 3: destroy_channel success --------------------------------------

    #[test]
    fn destroy_channel_success() {
        let response = ok_response(r#"{}"#);
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        let result = client.destroy_channel("wss://voice.example.com/ch/parcel-1");
        assert!(result.is_ok());
    }

    // -- Test 4: health_check healthy -----------------------------------------

    #[test]
    fn health_check_healthy() {
        let response = ok_response(r#"{"healthy":true}"#);
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        assert!(client.health_check().unwrap());
    }

    // -- Test 5: health_check unhealthy (returns false) -----------------------

    #[test]
    fn health_check_unhealthy() {
        let response = ok_response(r#"{"healthy":false}"#);
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        assert!(!client.health_check().unwrap());
    }

    // -- Test 6: JSON-RPC error response handled ------------------------------

    #[test]
    fn jsonrpc_error_response_handled() {
        let response = err_response(-32601, "Method not found");
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        let result = client.health_check();
        match result {
            Err(WolfvoiceError::Rpc { code, message }) => {
                assert_eq!(code, -32601);
                assert_eq!(message, "Method not found");
            }
            other => panic!("expected Rpc error, got {:?}", other),
        }
    }

    // -- Test 7: Transport error handled --------------------------------------

    #[test]
    fn transport_error_handled() {
        // Empty response queue triggers a Transport error in MockTransport.
        let transport = MockTransport::new(vec![]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        let result = client.health_check();
        assert!(matches!(result, Err(WolfvoiceError::Transport(_))));
    }

    // -- Test 8: Request serialization format correct --------------------------

    #[test]
    fn request_serialization_format() {
        let response = ok_response(r#"{"healthy":true}"#);
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        client.health_check().unwrap();

        let requests = client.transport.captured_requests();
        assert_eq!(requests.len(), 1);

        let req: serde_json::Value = serde_json::from_slice(&requests[0]).unwrap();
        assert_eq!(req["jsonrpc"], "2.0");
        assert_eq!(req["method"], "health_check");
        assert!(req["id"].is_u64());
        assert!(req["params"].is_object());
    }

    // -- Test 9: provision_account request fields correct ----------------------

    #[test]
    fn provision_account_request_fields() {
        let response =
            ok_response(r#"{"username":"u","password":"p","voice_server_uri":"wss://v"}"#);
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        let agent_id = Uuid::from_u128(0xABCD);
        client.provision_account(agent_id, "Alice Linden").unwrap();

        let requests = client.transport.captured_requests();
        let req: serde_json::Value = serde_json::from_slice(&requests[0]).unwrap();
        assert_eq!(req["method"], "provision_account");
        assert_eq!(
            req["params"]["agent_id"],
            "00000000-0000-0000-0000-00000000abcd"
        );
        assert_eq!(req["params"]["agent_name"], "Alice Linden");
    }

    // -- Test 10: create_channel request includes channel_type ----------------

    #[test]
    fn create_channel_request_includes_type() {
        let response = ok_response(r#"{"channel_uri":"wss://v/ch/1","channel_name":"estate-1"}"#);
        let transport = MockTransport::new(vec![response]);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        client
            .create_channel("estate-1", ChannelType::Estate)
            .unwrap();

        let requests = client.transport.captured_requests();
        let req: serde_json::Value = serde_json::from_slice(&requests[0]).unwrap();
        assert_eq!(req["method"], "create_channel");
        assert_eq!(req["params"]["channel_name"], "estate-1");
        assert_eq!(req["params"]["channel_type"], "estate");
    }

    // -- Test 11: sequential call IDs increment --------------------------------

    #[test]
    fn sequential_call_ids() {
        let responses = vec![
            ok_response(r#"{"healthy":true}"#),
            ok_response(r#"{"healthy":true}"#),
            ok_response(r#"{"healthy":true}"#),
        ];
        let transport = MockTransport::new(responses);
        let client = WolfvoiceClient::new("http://localhost:8090".to_string(), transport);

        client.health_check().unwrap();
        client.health_check().unwrap();
        client.health_check().unwrap();

        let requests = client.transport.captured_requests();
        let ids: Vec<u64> = requests
            .iter()
            .map(|r| {
                let v: serde_json::Value = serde_json::from_slice(r).unwrap();
                v["id"].as_u64().unwrap()
            })
            .collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }
}
