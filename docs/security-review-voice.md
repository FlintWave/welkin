# WebRTC Voice Security Review

Date: 2026-11-14
Reviewer: Claude (automated)
Scope: Voice module, wolfvoice client, capabilities, OSSL integration, portal integration, Docker configuration

## Overall Assessment

**CONDITIONAL PASS** -- The voice module has sound structural design with proper
input validation on the OSSL boundary and correct estate/parcel voice gating.
However, several gaps exist around authentication, rate limiting, and trust
boundary validation between the simulator and the wolfvoice sidecar.

## Findings

### [HIGH] No authentication on wolfvoice JSON-RPC connection

- **File**: `crates/opensim-modules/src/voice/wolfvoice_client.rs`
- **Lines**: 110-117, 121-145
- **Risk**: The `WolfvoiceClient` sends JSON-RPC requests to the wolfvoice
  sidecar over plain HTTP with no authentication token, API key, or mutual TLS.
  Any process on the network that can reach the sidecar endpoint (default
  `http://localhost:8090`) can provision voice accounts, create/destroy
  channels, and invoke health checks. In a Docker deployment, the sidecar port
  may be exposed beyond localhost.
- **Evidence**: `WolfvoiceClient::new()` takes only an endpoint URL and a
  transport. The `call()` method serialises a JSON-RPC request with no
  `Authorization` header, bearer token, or shared secret. The
  `WolfvoiceDockerConfig` in `docker.rs` has a `secret_key` field (line 15),
  and `validate_config` enforces it is non-empty, but the `WolfvoiceClient`
  never sends this key.
- **Recommendation**: Add an `auth_token: Option<String>` field to
  `WolfvoiceClient`. When set, include it as a `Bearer` token in the HTTP
  `Authorization` header on every request. Wire this to the `secret_key` from
  `WolfvoiceDockerConfig`. Alternatively, use HMAC-signed request bodies with
  the shared secret.

### [HIGH] No rate limiting on voice provisioning endpoints

- **File**: `crates/opensim-modules/src/voice/capabilities.rs`
- **Lines**: 126-138 (`handle_provision_voice_account`)
- **Risk**: The `ProvisionVoiceAccountRequest` capability handler has no rate
  limiting, throttling, or per-agent cooldown. A malicious or compromised
  viewer could flood the endpoint with provisioning requests, each of which
  triggers a JSON-RPC call to the wolfvoice sidecar. This enables:
  (1) denial-of-service against the sidecar,
  (2) credential enumeration if the sidecar assigns predictable usernames,
  (3) resource exhaustion on the simulator.
- **Evidence**: `handle_provision_voice_account` immediately calls
  `build_provision_response` with no checks on request frequency,
  agent authentication state, or deduplication.
- **Recommendation**: Add a per-agent rate limiter (e.g., token bucket) at the
  capability handler level. Cache provisioned credentials per agent UUID and
  return the cached copy if the agent already has valid credentials, avoiding
  repeated sidecar round-trips. A reasonable limit is 1 provision per agent per
  60 seconds.

### [MEDIUM] Credential isolation depends entirely on the wolfvoice sidecar

- **File**: `crates/opensim-modules/src/voice/capabilities.rs`
- **Lines**: 126-138
- **Risk**: The `handle_provision_voice_account` handler passes the requesting
  agent's UUID and name to the wolfvoice sidecar, then returns whatever
  credentials the sidecar provides. The simulator itself performs no isolation:
  it does not verify that the returned credentials are unique to the requesting
  agent, does not store a credential-to-agent mapping, and does not cache or
  deduplicate. If the sidecar has a bug that returns stale or wrong credentials,
  the simulator will faithfully pass them to the wrong agent.
- **Evidence**: The handler is a pure pass-through:
  `build_provision_response(creds)` constructs the LLSD from whatever the
  sidecar returned. No lookup or validation against the `agent_id` in the
  request.
- **Recommendation**: After provisioning, store the `(agent_id, username)`
  mapping in the `WebRtcVoiceModule`. On subsequent requests for the same
  agent, return the cached credentials. If the sidecar returns credentials
  whose username is already mapped to a different agent, log a security warning
  and reject the response.

### [MEDIUM] Wolfvoice trust boundary: response data not validated

- **File**: `crates/opensim-modules/src/voice/wolfvoice_client.rs`
- **Lines**: 147-165 (`provision_account`), 168-185 (`create_channel`)
- **Risk**: The `WolfvoiceClient` deserialises JSON-RPC responses directly into
  `VoiceCredentials` and `ChannelInfo` structs with no field-level validation.
  A compromised or buggy sidecar could return:
  - Excessively long `username`/`password`/`channel_uri` strings (memory DoS).
  - `channel_uri` values containing JavaScript or LLSD injection payloads that
    reach the viewer.
  - Empty or null fields that cause downstream panics or logic errors.
- **Evidence**: `serde_json::from_slice` deserialises directly. No
  `max_length`, pattern, or character-set validation on any field of
  `VoiceCredentials` or `ChannelInfo`.
- **Recommendation**: Add validation after deserialisation:
  - Reject credentials where `username` or `password` exceeds 256 bytes.
  - Reject `channel_uri` values exceeding 2048 bytes or containing control
    characters.
  - Reject `voice_server_uri` values that do not start with a recognised scheme
    (`wss://`, `webrtc://`).
  - The `xml_escape` function in `capabilities.rs` (line 93) provides defence
    in depth for the LLSD output path, which is good, but validation at the
    source is still needed.

### [MEDIUM] Docker Compose secret_key written in plain text to YAML

- **File**: `crates/opensim-modules/src/voice/docker.rs`
- **Lines**: 48 (`SECRET_KEY={}`)
- **Risk**: The `generate_compose_service` function writes the `secret_key`
  directly into the Docker Compose YAML as a plain-text environment variable.
  If this file is committed to version control, the secret is exposed. Docker
  Compose supports `secrets` and `env_file` directives that avoid embedding
  secrets in the YAML.
- **Evidence**: Line 48: `yaml.push_str(&format!("      - SECRET_KEY={}\n", config.secret_key));`
- **Recommendation**: Generate the Compose file using Docker secrets or
  `env_file` references instead of inline environment variables. Add a comment
  or documentation warning against committing the generated file to version
  control.

### [LOW] SIP address validation is minimal

- **File**: `crates/opensim-modules/src/voice/ossl_integration.rs`
- **Lines**: 51-59 (`validate_sip_address`)
- **Risk**: The `validate_sip_address` function rejects only empty strings and
  strings containing control characters. It does not validate SIP URI format
  (RFC 3261) or restrict the scheme. An OSSL script could set a parcel's SIP
  address to any arbitrary string (e.g., `javascript:alert(1)` or an
  excessively long string).
- **Evidence**: The function body checks `is_empty()` and `is_control()` only.
- **Recommendation**: Add validation for:
  - Maximum length (e.g., 512 characters).
  - Recognised URI scheme prefix (`sip:`, `webrtc://`, or the wolfvoice scheme).
  - Rejection of `javascript:`, `data:`, and other browser-interpretable
    schemes that could reach a viewer's embedded browser.

### [LOW] No TLS enforcement on wolfvoice endpoint

- **File**: `crates/opensim-modules/src/voice/mod.rs`
- **Lines**: 59 (default `http://localhost:8090`)
- **Risk**: The default endpoint uses `http://` (unencrypted). While localhost
  traffic is typically safe, in deployments where the sidecar runs on a
  different host (including Docker bridge networks), credentials and channel
  data travel in plain text.
- **Evidence**: `WebRtcVoiceConfig::default()` sets
  `wolfvoice_endpoint: "http://localhost:8090"`.
- **Recommendation**: Log a warning at startup when the configured endpoint
  uses `http://` with a non-loopback address. Consider requiring `https://` for
  non-localhost endpoints.

### [LOW] TURN credentials stored in module config without protection

- **File**: `crates/opensim-modules/src/voice/mod.rs`
- **Lines**: 46-47 (`turn_username`, `turn_credential`)
- **Risk**: TURN server credentials are stored as plain `String` fields in
  `WebRtcVoiceConfig` and loaded from the INI configuration. These credentials
  could be leaked through debug logging (the struct derives `Debug`) or
  serialisation.
- **Evidence**: `#[derive(Debug, Clone)]` on `WebRtcVoiceConfig` means
  `{:?}` formatting will include the `turn_credential` value.
- **Recommendation**: Replace `turn_credential` with a wrapper type that
  redacts its `Debug` output (e.g., `SecretString` from the `secrecy` crate,
  or a custom newtype that prints `[REDACTED]`).

## Summary

| Severity | Count | Key Theme |
|----------|-------|-----------|
| HIGH     | 2     | No auth on wolfvoice connection; no rate limiting |
| MEDIUM   | 3     | Credential isolation, response validation, secret in YAML |
| LOW      | 3     | SIP validation, TLS enforcement, credential debug exposure |

The two HIGH findings should be addressed before production deployment. The
MEDIUM findings should be resolved before any multi-tenant or grid-facing
deployment. The LOW findings are hardening improvements that can be addressed
in subsequent iterations.
