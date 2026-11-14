# Architecture

Rust port of the OpenSimulator 0.9.3.0 server (grid services and region
simulator). Wire and schema compatibility with the C# server is the contract;
internals are free to change. Licence: GPL-3.0-or-later. The C# reference tree
is read-only at `/var/home/erik/Projects/opensim-0.9.3.0-source/`.

Status: Phase 3 in progress. Phases 0, 1 and 2 (port-foundation, open-source-audit,
protocol-core, physics-backend, grid-services, capacity-assessor, grid-portal)
are done, and region-simulator-core is accepted. Most simulator and module crates below are still empty skeletons (a
`lib.rs` with no logic). The table marks which.

## Workspace

Cargo workspace (`Cargo.toml`, edition 2024, `members = ["crates/*"]`). All
crates are prefixed `opensim-`.

| Crate | Role | State |
|---|---|---|
| `opensim-types` | SL protocol enums and bitfields: asset, inventory, folder and wearable types, attachment points, `PermissionMask` | implemented |
| `opensim-llsd` | Wrapper over `aurora-llsd`, plus server-side XML-RPC codec (`src/xmlrpc.rs`) for login and legacy calls | implemented |
| `opensim-msg` | Wrapper over `aurora-msg` (framing, zero-coding, typed messages). `src/objects.rs` adds server-direction object update encoders | implemented |
| `opensim-prim` | Re-exports from `aurora-prim`, plus physics meshing: analytic box/sphere, volume and sculpt trimeshes, LOD selection, mesh asset decoder (`mesh_asset.rs`), V-HACD convex decomposition, shape and decomposition caches, linkset compound colliders (`meshing.rs`). See `docs/prim-meshing-summary.md` | implemented |
| `opensim-net` | Server-side LLUDP: `circuit.rs`, `throttle.rs`, plus `server.rs` (listener), `client.rs` (per-viewer state), `dispatch.rs` (message-id table), `handlers.rs` (viewer handlers) | implemented |
| `opensim-config` | TOML configuration with env overrides; `OpenSim.ini` / `Robust.ini` importer | implemented |
| `opensim-runtime` | Logging (`tracing`), Prometheus metrics, health endpoint, graceful shutdown | implemented |
| `opensim-conformance` | Recorder/replayer (HTTP, XML-RPC, LLUDP), `pCampBot` wrapper, test bot (`tests/test_bot.rs`) | implemented |
| `opensim-physics` | `PhysicsScene`/`PhysicsActor` traits, null backend, and solver-independent behaviour: vehicles (`vehicle.rs`), avatar controller (`avatar.rs`), move-to-target/hover/look-at (`behaviors.rs`), keyframed motion (`keyframe.rs`), terse-update thresholds (`terse_update.rs`) | implemented |
| `opensim-physics-rapier` | Rapier 0.36.1 backend: prims and linksets, terrain heightfield, collisions, probes, materials, snapshot/restore; consumes `aurora-prim` geometry | implemented |
| `opensim-caps` | HTTP capabilities: per-session seed cap, LLSD event queue long-poll, fetch, region info and upload cap handlers | implemented (thin handlers) |
| `opensim-capacity` | Host scan (CPU, memory with cgroup limits, disk, container), network stub, `HostReport`, 12 sourced coefficients, estimate engine with quiet/typical/busy profiles. `benchmark`, `model` and `live` modules are stubs pending the region simulator | partial |
| `opensim-data` | `sqlx` data layer: 13 stores over MariaDB, PostgreSQL and SQLite, plus a migration runner with 53 embedded C# schema files | implemented |
| `opensim-services` | 14 Robust grid service handlers behind axum: asset, inventory, grid, presence, griduser, accounts, auth, authorization, friends, avatar, mute, profiles, map, estate, agent preferences, simulation (501 stub) and XML-RPC login | implemented |
| `opensim-scene` | Scene, object groups and parts, presences, XML2 serializer, terrain (channel, file I/O, brushes, compression), region manager, shutdown | implemented |
| `opensim-modules` | Region modules (land, permissions, chat, ...) | skeleton |
| `opensim-archive` | OAR/IAR archives | skeleton |
| `opensim-script-compiler`, `-vm`, `-api` | SLua embedding, LSL/Luau, `ll*`/`os*` API | skeleton |
| `opensim-portal` | Grid portal: staff auth (argon2), sessions, RBAC roles, audit log, region control, pages, grid map, people/sign-up, first-run wizard, content/catalogue | implemented |
| `opensim-companion` | Tauri 2 desktop app: tray icon, service start/stop, log viewer, notifications, remote grid connect. **Standalone**: excluded from workspace (`Cargo.toml` exclude) | implemented |
| `opensim-robust` | Binary: grid services | skeleton (`main.rs`) |
| `opensim-region` | Binary: region simulator (config, multi-region, LLUDP and caps startup, graceful shutdown) | implemented |

## Dependency graph (current)

```
aurora-llsd -> opensim-llsd -------------------+
aurora-msg  -> opensim-msg --------------------+--> opensim-net
aurora-prim -> opensim-prim, opensim-physics-rapier
opensim-physics -> opensim-physics-rapier (rapier3d 0.36.1)
opensim-types <- llsd, msg, net, caps, data, scene, physics, archive, scripts

opensim-config, opensim-runtime -> opensim-robust
opensim-data, opensim-caps -> opensim-services -> opensim-robust
opensim-scene, opensim-services -> opensim-modules
opensim-capacity -> opensim-portal -> opensim-robust (portal router)
opensim-companion (standalone, Tauri 2) --HTTP--> opensim-portal control interface
opensim-script-compiler -> opensim-script-vm -> opensim-script-api
aurora-net (test bot only) -> opensim-conformance, dev-dep of opensim-msg
```

Layering rule: nothing outside a wrapper crate imports an `aurora-*` crate
directly (the one exception is `opensim-types`, which re-exports shared
definitions, and `opensim-conformance`, which needs `aurora-net` as a client).

## Aurora crates reused

Aurora Viewer (GPL-3.0-or-later) is the acceptance viewer. Its crates are
unpublished (`publish = false`, 0.1.0) and change daily, so they are git
dependencies pinned to commit `3d523b14a17bbc6484b208378c0eef291ad5ac32`
(workspace `Cargo.toml`) and wrapped. Updating the pin is a reviewed change
gated by the replay tests.

| Crate | Used for | Why reused |
|---|---|---|
| `aurora-llsd` | LLSD in XML, binary, notation | Standalone, complete; replaces part of libopenmetaverse |
| `aurora-msg` | Packet framing, zero-coding, message types generated from the Linden message template, reader and writer | Avoids regenerating the codec; same template the viewer uses |
| `aurora-prim` | Prim params, texture entries, extra params, sculpts, geometry generator | Needed for physics shapes and object updates; large to rewrite |
| `aurora-assets` | JPEG 2000, mesh, mesh physics hulls (pulls `openjpeg-sys`, C code) | Declared in the workspace; evaluation pending (audit task 2.5) |
| `aurora-net` | Viewer-side client protocol | Test bot only; never linked into the server |

Aurora is a viewer, so some paths exist in one direction only. Gaps found in
the inventory (protocol-core 1.2) are filled in `opensim-msg` (object update
encoders) and offered upstream.

## Other key dependencies

`tokio` and `axum` (async runtime and HTTP), `sqlx` (MariaDB, PostgreSQL,
SQLite over the existing schema), `rapier3d` 0.36.1 (physics backend, behind
the `opensim-physics` trait), `glam` (vectors, as in Aurora), `tracing`,
`metrics` with `metrics-exporter-prometheus`, `serde`/`toml`.

## Build and deployment

- `Containerfile` (multi-stage, Podman and Docker, static binaries, non-root,
  one image per binary). `compose.yaml` starts database, `robust` and one
  region; `compose.reference.yaml` runs unmodified C# OpenSim as the
  conformance reference.
- A region advertises its external host and UDP port to viewers, so the
  published port must equal the advertised one.
- `scripts/check.sh`: fmt, clippy (`-D warnings`), tests, `cargo deny check`.
  `deny.toml` holds the licence allow-list.

## Verification

1. Unit tests ported with the code.
2. Recorded C# request/reply pairs replayed (`opensim-conformance`,
   `test-data/lludp_session.jsonl`).
3. Test bot on `aurora-net` logs in against a simulator.
4. Fuzz targets in `crates/opensim-msg/fuzz/`.
5. Grid services: 14 replay tests and 4 multi-step wire-traffic tests (`opensim-conformance/tests`).
6. Later: live `pCampBot`, physics trajectory comparison against recorded C# runs, Aurora Viewer sessions.

Physics details: `docs/physics-backend-summary.md`. Grid services details: `docs/grid-services-summary.md`. Region simulator details: `docs/region-simulator-core-summary.md`. Capacity assessor details: `docs/capacity-assessor-summary.md`. Grid portal details: `docs/grid-portal-summary.md`.

## Region simulator

Details in `docs/region-simulator-core-summary.md`.

- `opensim-region` loads config, opens databases, builds one `Scene` per
  region (`RegionManager` for several), starts the LLUDP listener and the caps
  HTTP server, and registers with the grid.
- `opensim-net` receives UDP, routes to a per-viewer `LLUDPClient`, and
  dispatches by message id to typed handlers that mutate the `Scene`.
- `opensim-caps` serves per-session capabilities and the LLSD event queue.
- `opensim-scene` holds groups, parts, presences and the terrain channel;
  `xml2.rs` is the C#-compatible object format.
- `opensim-data::simulation` persists objects (XML2), terrain, settings and
  spawn points over the C# `regionstore` tables. The scene loop saves dirty
  objects on a timer and on shutdown.
- Open before production: bounded client map, bounded channels, stale session
  reaping (second review, 3 HIGH).

## Grid Portal

The portal is served by the `robust` binary via `axum`, with GridGate's page set
as the model. It is the single management interface for the grid.

### opensim-portal crate (2,084 lines, 12 source files)

| Module | Responsibility |
|---|---|
| `auth.rs` | Argon2id password hashing (OWASP defaults via `argon2` 0.5), login/logout handlers, TOTP stub (locked: returns false until implemented) |
| `sessions.rs` | In-memory session store (HashMap), `RequireStaff` axum extractor, Bearer token and cookie extraction, expiry enforcement |
| `roles.rs` | `StaffRole` enum: Admin, Operator, Viewer with `can_manage_accounts()` and `can_manage_regions()` checks |
| `audit.rs` | In-memory audit log (to be persisted to SQLite) |
| `region_control.rs` | CRUD + start/stop/snapshot for regions (process management stubs pending region-simulator-core) |
| `pages.rs` | Portal HTML shell, status, logs, metrics, audit endpoints |
| `map.rs` | Region placement with coordinate conflict detection, spiral free-coordinate finder, variable sizes, external grid overlay (6 tests) |
| `people.rs` | Sign-up with approval, account search/suspend, password reset with token store; all staff handlers enforce `require_admin()` |
| `wizard.rs` | 7-step first-run wizard: mode → owner → network → capacity → placement → content → finish |
| `content.rs` | Catalogue format with licence fields, OAR import, terrain presets, backup scheduling |

### Security model

- **Authentication:** Argon2id with OsRng salt, UUID v4 session tokens, constant-time password verification.
- **Authorization:** RBAC: Admin (full access), Operator (region management), Viewer (read-only). People/account management requires Admin. Region management requires Operator or above.
- **Security review findings:** 3 HIGHs fixed (TOTP stub locked, role enforcement on people endpoints, password hashed at sign-up). 5 MEDIUMs noted for deployment hardening (rate limiting, cookie CSRF flags, bot protection, audit persistence, body size limit). Full report: `scratchpad/security_review_portal.md`.

### opensim-companion crate (standalone Tauri 2 app)

Not part of the workspace; has its own `Cargo.toml` with Tauri 2 dependencies.
Excluded via `exclude = ["crates/opensim-companion"]` in workspace `Cargo.toml`.

- WebView pointing at the local portal (`http://localhost:8002/`)
- System tray: status indicator, start/stop service, open portal, show window, quit
- Tauri commands: `start_service`, `stop_service`, `service_status` (systemctl), `view_logs` (journalctl), `open_portal`, `connect_remote`, `list_remote_grids`, `set_portal_url`
- Desktop notifications for service start/stop
- `.desktop` launcher file for Linux, bundle config for Windows/macOS in `tauri.conf.json`

### install/ directory

| File | Purpose |
|---|---|
| `install.sh` | Full install: distro detection, prereqs, system user, directories, config generation, admin token, DB setup, systemd service, start |
| `upgrade.sh` | Backup config, stop service, replace binary, run migrations, restart |
| `uninstall.sh` | Stop, disable, remove binary/service; preserves data |
| `install-archive.sh` | Release tarball install to `/opt/opensim/` (non-container path) |
| `opensim-robust.service` | Systemd unit file |
| `container/Containerfile` | Multi-stage Rust build → Debian slim runtime |
| `container/compose.yaml` | Compose with optional MySQL/PostgreSQL |

### E2E test

`crates/opensim-portal/tests/e2e_portal_flow.rs`: 14-stage integration test
via `tower::ServiceExt::oneshot()` covering: portal page, status, wizard (7
steps), login, region lifecycle, map placement, people sign-up/approval,
authenticated pages, content, access control, logout.

## Phase 2 summary

Phase 2 complete: 53 tasks across 3 changes. All accepted.

| Change | Tasks | Lines | Key deliverables |
|---|---|---|---|
| `grid-services` | 14/14 | ~2,800 | 14 HTTP service handlers (asset, inventory, grid, presence, auth, login, etc.), replay conformance tests (14 fixtures), interop wire-traffic tests (4 multi-step flows), Opus security review with 1 fix (constant-time hash comparison) |
| `capacity-assessor` | 13/13 | ~600 | Host scanning (CPU, memory with cgroup limits, disk, container), 12 sourced coefficients, estimate engine with 3 workload profiles, stubs for benchmark/model/live pending region-simulator-core (Phase 3) |
| `grid-portal` | 26/26 | ~3,800 | 2,084-line portal crate, installer scripts, Tauri 2 companion app, 14-stage e2e test, Opus security review with 3 fixes |

Phase 2 total: 111/142 overall tasks complete.

Phase 3: region-simulator-core (all 7 task groups) and prim-meshing accepted.

## External grid mode

Grid profiles let a region join a grid it does not run. See
`docs/external-grid-mode-summary.md`.

| Component | Location | Role |
|---|---|---|
| Grid profiles | `opensim-config/src/profile.rs` | `Standalone`, `OwnGrid`, `Custom(name)`; TOML loader, override chain, importer for C# `GridCommon.ini` (generates the `osgrid` profile) |
| Remote connectors | `opensim-services/src/remote/` | Asset, grid and inventory clients (inventory read-only) |
| Asset cache | `opensim-services/src/cache.rs` | Local cache with expiry |
| Registration | `opensim-services/src/registration.rs` | Register, deregister, neighbour notification |
| Hypergrid traits | `opensim-services/src/hypergrid.rs` | Gatekeeper, UserAgent, asset, inventory, friends, IM traits with stubs |
| Wizard checks | `opensim-services/src/wizard.rs` | Region name, coordinates, estate owner, ports |

The remote grid is treated as hostile. Review `docs/security-review-remote-writes.md`
found 1 CRITICAL (SSRF) and 4 HIGH issues that block use against an untrusted grid.

## Roadmap

See `openspec/ROADMAP.md` for phases 1 to 7, and `docs/decisions.md` for the
decision log.
