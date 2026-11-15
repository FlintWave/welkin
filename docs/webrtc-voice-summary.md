# WebRTC voice summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Decisions](decisions.md#wv1) · Previous: [Hypergrid](hypergrid-summary.md) · Next: [Cutover](cutover-summary.md)

Status: accepted (11 of 11 tasks, 4 sections, 102 tests). Security review
verdict: CONDITIONAL PASS. Change: `openspec/changes/webrtc-voice/`.

## What was built

WebRTC voice chat via the `wolfvoice` sidecar (Rust, Apache-2.0), replacing
the Vivox (proprietary) and FreeSWITCH modules from C#. Reference:
`FreeSwitchVoiceModule.cs` (~900 lines) for the capability pattern,
`os-webrtc-janus` (C#, MPL-2.0) for WebRTC capability handling. Ported as
~2,079 lines of Rust with 102 tests (59 unit + 43 integration).

| Section | Area | Tasks |
|---|---|---|
| 1 | Voice module foundation: IVoiceModule trait, config, wolfvoice client | 1.1-1.2 |
| 2 | Voice capabilities: provision, parcel info, channel lifecycle | 2.1-2.3 |
| 3 | Integration: OSSL wiring, Docker Compose, grid-portal | 3.1-3.3 |
| 4 | Testing and security review | 4.1-4.3 |

## Architecture

```text
Aurora Viewer
    │
    ├─ ProvisionVoiceAccountRequest (HTTP POST, capability)
    │       → WebRtcVoiceModule checks region voice enabled
    │       → JSON-RPC to wolfvoice: provision_account(agent_uuid, name)
    │       → Returns LLSD XML: voice_credentials
    │
    └─ ParcelVoiceInfoRequest (HTTP POST, capability)
            → WebRtcVoiceModule checks estate + parcel voice flags
            → Constructs channel URI: webrtc:region/parcel
            → Returns LLSD XML: channel_uri, region_name
```

wolfvoice runs as a Docker sidecar and handles WebRTC, TURN/STUN, and audio
mixing. The server only implements the two voice capabilities and the
JSON-RPC bridge — no audio processing in the simulator.

### Module breakdown

| Module | Lines | Tests | Responsibility |
|---|---|---|---|
| `voice/mod.rs` | 663 | 12 | WebRtcVoiceModule, IVoiceModule trait, channel lifecycle |
| `voice/wolfvoice_client.rs` | 471 | 11 | JSON-RPC client: provision, channel create/destroy, health |
| `voice/capabilities.rs` | 390 | 15 | ProvisionVoiceAccountRequest, ParcelVoiceInfoRequest |
| `voice/ossl_integration.rs` | 192 | 8 | Wire to `osSetParcelSIPAddress` |
| `voice/docker.rs` | 176 | 6 | Docker Compose service definition, health check |
| `voice/portal_integration.rs` | 185 | 7 | Admin dashboard voice status, estate voice toggle |
| Integration tests | — | 43 | Mock wolfvoice, provision flow, parcel crossing, concurrency |

## Key decisions

- **V1:** JSON-RPC over HTTP to wolfvoice, not gRPC — simpler, no protobuf
  dependency, matches wolfvoice's documented API.
- **V2:** Channel URIs use `webrtc:region/parcel` format instead of SIP URIs.
  Aurora Viewer expects this format.
- **V3:** Voice credentials are ephemeral (per-session), not stored in the
  database. wolfvoice manages credential lifecycle.
- **V4:** Estate and parcel voice-enable checks reuse existing flags from
  region-modules — no new database columns.

## Tests

102 tests total (59 unit + 43 integration). Integration tests use a mock
wolfvoice JSON-RPC server (`MockTransport` + `CapturedRequests`) to verify
the full provision flow, parcel crossing channel switch, estate voice disable,
and concurrent provisioning.

## Security review

> [!WARNING]
> **CONDITIONAL PASS** — 0 critical, 2 high, 3 medium, 3 low tracked for remediation.
> See [security review](security-review-voice.md) for details.

Full report: [security-review-voice.md](security-review-voice.md).

| ID | Severity | Finding |
|---|---|---|
| H-1 | HIGH | wolfvoice JSON-RPC auth — shared secret or mTLS required, not open access |
| H-2 | HIGH | Voice provisioning rate limiting missing — credential exhaustion possible |

Both HIGH findings must be resolved before production deployment. The wolfvoice
sidecar must not accept unauthenticated JSON-RPC in a multi-user grid.

## Commits

| Hash | Description |
|---|---|
| `bd09d91` | Sections 1-3: WebRTC voice module, wolfvoice client, capabilities, integration |
| `8e02eae` | Section 4: integration tests, security review, phase accepted |
