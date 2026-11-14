# webrtc-voice: WebRTC voice via wolfvoice sidecar

## Summary

Implement voice chat using WebRTC through the `wolfvoice` sidecar (Rust,
Apache-2.0). The server registers the two standard voice capabilities
(`ProvisionVoiceAccountRequest`, `ParcelVoiceInfoRequest`) and forwards
requests as JSON-RPC to wolfvoice, which handles WebRTC, TURN/STUN, and
audio mixing.

Neither Vivox (proprietary) nor FreeSWITCH are ported. Aurora Viewer
supports WebRTC voice only.

Reference: `FreeSwitchVoiceModule.cs` (900 lines) for the capability pattern,
`os-webrtc-janus` (C#, MPL-2.0) for the WebRTC capability handling.

## Scope

### In scope

- `WebRtcVoiceModule`: shared region module registering two voice capabilities.
- `ProvisionVoiceAccountRequest` handler: map agent UUID to voice credentials,
  forward to wolfvoice via JSON-RPC, return LLSD XML response to viewer.
- `ParcelVoiceInfoRequest` handler: check estate voice enabled, check parcel
  voice enabled, return WebRTC-format channel info (region name, channel URI).
- `IVoiceModule` trait: `set_land_sip_address` for `osSetParcelSIPAddress`
  OSSL function.
- wolfvoice JSON-RPC client: account provisioning, channel create/destroy,
  session management.
- Docker Compose service definition for wolfvoice sidecar.
- INI configuration: wolfvoice endpoint, TURN server, default voice settings.
- Integration tests with a mock wolfvoice JSON-RPC server.

### Out of scope

- Vivox integration (proprietary, not ported per ROADMAP decision 6).
- FreeSWITCH integration (replaced by WebRTC).
- Audio quality tuning (wolfvoice responsibility).
- TURN/STUN server deployment (operator infrastructure).

## Dependencies

- Region module framework (`region-modules` section 1)
- Capabilities system (`region-simulator-core`)
- Estate/parcel voice-enable flags (`region-modules` sections 2, 4)
- OSSL `osSetParcelSIPAddress` (`ossl-api`)

## Crates

- `reqwest` (already in workspace): HTTP client for JSON-RPC to wolfvoice.
- `serde_json` (already in workspace): JSON-RPC serialization.
- No new external crates required.

## Design

### Capability flow

```
Aurora Viewer → ProvisionVoiceAccountRequest (HTTP POST)
  → WebRtcVoiceModule checks region voice enabled
  → JSON-RPC to wolfvoice: provision_account(agent_uuid, display_name)
  → wolfvoice returns: {username, credential, server_uri}
  → Module returns LLSD XML: voice_credentials map

Aurora Viewer → ParcelVoiceInfoRequest (HTTP POST)
  → WebRtcVoiceModule checks estate + parcel voice flags
  → Constructs channel URI from region name + parcel local ID
  → Returns LLSD XML: {region_name, parcel_local_id, channel_uri}
```

### Key decisions

- V1: JSON-RPC over HTTP (not gRPC) to wolfvoice — simpler, no protobuf
  dependency, matches wolfvoice's documented API.
- V2: Channel URIs use `webrtc:region/parcel` format instead of SIP URIs.
  Aurora Viewer expects this format.
- V3: Voice credentials are ephemeral (per-session), not stored in the
  database. wolfvoice manages credential lifecycle.
- V4: Estate and parcel voice-enable checks reuse the existing flags from
  region-modules, no new database columns.

## Security considerations

- Voice credentials must not leak to other agents — per-agent provisioning
  with session-scoped tokens.
- JSON-RPC to wolfvoice should use a shared secret or mTLS, not open access.
- Rate-limit voice provisioning to prevent credential exhaustion.
