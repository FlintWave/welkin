# Hypergrid summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Decisions](decisions.md#h1) · Previous: [Social Modules](social-modules-summary.md) · Next: [WebRTC Voice](webrtc-voice-summary.md)

Status: accepted (20 of 20 tasks, 10 sections, 265 tests). Security review
verdict: CONDITIONAL PASS (CRITICAL RISK). Change: `openspec/changes/hypergrid/`.

## What was built

The Hypergrid (HG) system from OpenSimulator 0.9.3.0: ~11,171 lines of C#
across ~23 files in two layers (Robust services and region modules). Ported
as ~9,144 lines of Rust with 265 tests (215 unit + 50 integration).

HG enables cross-grid teleportation, inventory transfer, friend lists, IM,
and map linking between independent OpenSimulator grids.

| Section | Area | Tasks |
|---|---|---|
| 1 | Gatekeeper service: foreign visitor entry, access control | 1.1-1.2 |
| 2 | UserAgent service: outbound HG teleport, session verification | 2.1-2.2 |
| 3 | HG inventory: proxied access, suitcase folder restriction | 3.1-3.2 |
| 4 | HG assets and friends: access-controlled transfer, cross-grid friends | 4.1-4.2 |
| 5 | HG messaging: cross-grid IM routing | 5.1-5.2 |
| 6 | Hyperlink management: grid linking, HELO probe | 6.1-6.2 |
| 7 | Service connectors: HTTP clients for Gatekeeper, UserAgent, HGFriends | 7.1-7.2 |
| 8 | HG region modules: entity transfer, inventory, friends, lure, map, users | 8.1-8.3 |
| 9 | HypergridServiceInConnectorModule: expose HG endpoints in simulator | 9.1 |
| 10 | Integration tests and CRITICAL RISK security review | 10.1-10.2 |

## Architecture

```text
Foreign Grid                          Home Grid
┌─────────────┐   HTTP/XMLRPC   ┌─────────────────┐
│ UserAgent    │ ←──────────────→│ Gatekeeper       │
│ Service      │                 │ Service          │
└─────────────┘                  └────────┬────────┘
                                          │
                                 ┌────────▼────────┐
                                 │ Region Simulator │
                                 │ HGEntityTransfer │
                                 │ HGInventoryAccess│
                                 │ HGFriends        │
                                 │ HGAssetMapper    │
                                 └─────────────────┘
```

Two sides: the **Gatekeeper** (entry point for foreign visitors) and the
**UserAgent** service (manages outbound travel from the home grid). HG region
modules extend their base counterparts via trait composition.

### Module breakdown

| Module | Lines | Tests | Responsibility |
|---|---|---|---|
| `gatekeeper.rs` | 752 | 10 | Foreign visitor entry, hyperlink creation, access control |
| `user_agent.rs` | 844 | 17 | Outbound HG teleport, session verification, UUI |
| `hg_inventory.rs` | 618 | 12 | Proxied inventory, suitcase folder restriction |
| `hg_assets.rs` | 285 | 9 | Access-controlled asset transfer, export filtering |
| `hg_friends.rs` | 587 | 6 | Cross-grid friendship, secret verification |
| `hg/messaging.rs` | 641 | 11 | Cross-grid IM routing, HGMessageTransferModule |
| `hg_linker.rs` | 311 | 6 | Grid-to-grid region linking, console commands |
| `connectors/` | 811 | 10 | HTTP clients for Gatekeeper, UserAgent, HGFriends, HELO |
| `hg/entity_transfer.rs` | 578 | 9 | Cross-grid teleport, appearance restriction |
| `hg/inventory_access.rs` | 548 | 8 | Route inventory to local/remote, suitcase enforcement |
| `hg/` (lure, map, users) | — | 28 | Lures, map blocks, UUID-to-name for foreign users |
| `hg/connector_in.rs` | 326 | 5 | Expose HG endpoints in simulator |
| Integration tests | — | 50 | HG teleport round-trip, suitcase, IM, hyperlinks, friends |

## Key decisions

- **H1:** HG modules extend base modules via trait composition, not separate
  implementations. `HGEntityTransferModule` wraps `EntityTransferModule`.
- **H2:** UUI format (`uuid;griduri;name`) stored alongside local UUIDs.
  `HGUserManagementModule` resolves `first.last@griduri` to local cache.
- **H3:** Suitcase inventory uses path validation with a root-anchored check
  to prevent directory traversal to the full inventory tree.
- **H4:** Traveling agent sessions use cryptographically random tokens, not
  predictable values. Tokens are single-use and expire on logout.
- **H5:** HELO probe (`HeloServicesConnector`) discovers HG capability before
  attempting cross-grid operations, with timeout and retry.

## Tests

265 tests total (215 unit + 50 integration). Integration tests cover HG
teleport round-trip (outbound + inbound), suitcase inventory access, cross-grid
IM delivery, hyperlink creation/removal, and friend request across grids.

Full suite after this phase: 1,679 tests passing.

## Security review

> [!CAUTION]
> **CRITICAL RISK — CONDITIONAL PASS** — 3 critical, 3 high, 3 medium, 2 low
> findings. All CRITICAL and HIGH findings must be fixed before any
> public-facing deployment.
> See [security review](security-review-hypergrid.md) for details.

Full report: [security-review-hypergrid.md](security-review-hypergrid.md).

### Critical findings

| ID | Severity | Finding |
|---|---|---|
| F-01 | CRITICAL | Suitcase folder escape via `get_item` — no suitcase-tree check |
| F-02 | CRITICAL | `VerifyAgent` callback not implemented in gatekeeper |
| F-03 | CRITICAL | UUID impersonation check not implemented |

### High findings

| ID | Severity | Finding |
|---|---|---|
| F-04 | HIGH | Session token comparison is not constant-time |
| F-05 | HIGH | No session token expiration or TTL |
| F-06 | HIGH | No rate limiting on any Hypergrid endpoint |

### Medium and low

| ID | Severity | Finding |
|---|---|---|
| F-07 | MEDIUM | `assets_exist` leaks asset existence info |
| F-08 | MEDIUM | Friendship secret has only 32 bits of entropy |
| F-09 | MEDIUM | Friendship deletion uses weak pattern matching |
| F-10 | LOW | MAC/ID0 deny lists use substring matching (ported C# bug) |
| F-11 | LOW | No per-asset transfer-permission check (matches C# design) |

All 6 CRITICAL + HIGH findings require remediation before deployment. F-01
through F-03 must be fixed before any multi-user grid.

## Commits

| Hash | Description |
|---|---|
| `7892e6c` | Sections 1-9: services, connectors, and region modules (27 files, 9,144 lines, 215 tests) |
| `8e02eae` | Section 10: integration tests + CRITICAL security review |
