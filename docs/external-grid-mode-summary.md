# external-grid-mode summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Decisions](decisions.md#e1) · Previous: [Archives](archives-summary.md)

Status: accepted (12/12 tasks checked in `openspec/changes/external-grid-mode/tasks.md`),
with two live-grid tasks deferred to Phase 7 cutover. Commits `8f35477`,
`5ec6dce`, `91a1147`, `0b0f587`, `e46a2dd`. Second review verdict:
CONDITIONAL PASS.

## What was built

A region can join a grid somebody else runs. Which grid is configuration, not
code.

### Grid profiles (`welkin-config/src/profile.rs`, `8f35477`)

- `GridProfile` enum: `Standalone` (all services on localhost), `OwnGrid`
  (the Rust `robust`, configurable URLs) and `Custom(name)` (any grid, loaded
  from a named TOML file such as `profiles/osgrid.toml`).
- `ServiceUrls` names every remote service; `localhost(port)` and
  `all_same(base)` build common cases.
- `load_profile` reads a profile; `GridProfileConfig::to_config` produces the
  runtime `Config`, with per-profile overrides applied through an override
  chain (physics engine, script limits, voice, search, groups, profiles,
  offline messaging).
- `import_grid_ini` reads a C# `GridCommon.ini`/`OpenSim.ini` and writes a
  profile. The OSgrid profile is generated this way from OSgrid's published
  build, so addresses are never typed from memory.

### Remote connectors (`welkin-services`, `5ec6dce`)

- `remote/asset.rs`, `remote/grid.rs`, `remote/inventory.rs`: HTTP/XML
  clients for the grid's asset, grid and inventory services.
- `cache.rs`: local asset cache with expiry.
- `registration.rs`: region register (protocol version 1), deregister on
  shutdown, and neighbour notification, as the C# connector does it.

### Hypergrid traits (`welkin-services/src/hypergrid.rs`, `91a1147`)

Six async traits with stub implementations and 4 tests: `GatekeeperService`,
`UserAgentService`, `HypergridAssetService`, `HypergridInventoryService`,
`HypergridFriendsService`, `HypergridInstantMessageService`. Wire types
(`GridRegionInfo`, `TravelingAgentInfo`, `AgentCircuitInfo`, `HgAsset`, and
others) sit beside them. These are protocol surfaces only; no live Hypergrid
traffic is implemented.

### OSgrid research (`docs/osgrid-compatibility.md`)

Source-only research. Status: UNKNOWN whether OSgrid accepts regions on
third-party software. No contact with the administrators has been made; the
document carries an operator checklist of questions to ask. The setup rules
(unique region names, free coordinates, estate owner is the operator's OSgrid
avatar, per-region TCP and UDP ports, public external host name) are encoded
as wizard checks.

### Wizard checks (`welkin-services/src/wizard.rs`, 13 tests)

`check_region_name`, `check_coordinates`, `check_estate_owner`, `check_ports`
and `run_all_checks`, returning a `WizardReport`. Checks work against
`GridConnector` and `AccountConnector` traits, so they run against a local
store or a remote connector.

### Security review (`docs/security-review-remote-writes.md`, `e46a2dd`)

Threat model: the remote grid is hostile. Findings: 1 CRITICAL, 4 HIGH,
6 MEDIUM, 3 LOW, 2 INFO.

| ID | Finding |
|---|---|
| CRITICAL-1 | SSRF: `notify_neighbours` posts to `server_uri` values the remote grid supplies |
| HIGH-1 | XML injection in `asset_exists` (unescaped `id`) |
| HIGH-2 | Path traversal in asset cache filenames |
| HIGH-3 | No HTTP timeouts on any remote connector |
| HIGH-4 | No response body size limit |

MEDIUM items cover no TLS enforcement for `regionSecret` and tokens, secrets
in error messages, no UUID validation on outgoing requests, cache poisoning,
unchecked HTTP status in `register_region`, and redirect credential leaks.

## Deferred to Phase 7 cutover

- 3.2: record a C# OSgrid region's service traffic and replay it against the
  Rust connectors. Needs live C# region infrastructure.
- 3.4: attach one empty region, then one with content, with the operator
  present. Needs a live grid and an answer from OSgrid.

Both are marked checked in `tasks.md` with a DEFERRED note (`0b0f587`).

## Known gaps

- The five CRITICAL/HIGH findings are open. They must be fixed before any
  region connects to an untrusted grid. Until then the `osgrid` and `custom`
  profiles stay disabled.
- The remote inventory connector is read-only (LOW-3). Write paths must
  apply every review finding when added.
- Hypergrid is traits and stubs only.
- OSgrid third-party acceptance is unresolved.
- Replay fixtures from a real C# region do not exist yet (see 3.2).
