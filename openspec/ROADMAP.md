# Roadmap: OpenSimulator 0.9.3.0 ported to Rust

Status: planning. Nothing is implemented. Last updated 2026-11-13.

## Goal

A Rust implementation of the OpenSimulator 0.9.3.0 server (grid services and
region simulator), shipped as containers, built only from open-source parts,
with Aurora Viewer as the acceptance viewer. Existing grids must be able to
move over without losing content.

## What is being ported (measured in this tree)

1,539 C# files, 561,396 lines, targeting .NET 8.

| Area | Lines | Notes |
|---|---|---|
| Framework | 65,342 | types, config, HTTP server, serialization |
| Data | 32,436 | MySQL, PGSQL, SQLite backends and migrations |
| Services + Server | 42,313 | Robust grid services (login, grid, assets, inventory, ...) |
| Capabilities + ClientStack | 36,491 | LLUDP and HTTP capabilities (the viewer protocol) |
| Region/Framework | 65,204 | scene graph, objects, avatars |
| Region/CoreModules | 86,967 | terrain, land, permissions, archiver, chat, ... |
| Region/OptionalModules | 31,427 | voice, NPCs, IRC, scripting extras |
| ScriptEngine | 89,668 | YEngine plus 454 `ll*` and 290 `os*` functions |
| PhysicsModules | 57,203 | ubODE, BulletSim, meshing, convex decomposition |
| Addons, plugins, tools, tests, application, ThirdParty | 38,338 | Groups, OfflineIM, pCampBot, tests |

The rows sum to 545,389; the remaining 16,007 lines are outside these
directories, almost all in the Prebuild build tool, which is not ported.

## Decisions

1. **Incremental replacement, not a single rewrite.** The grid services speak
   HTTP and XML-RPC and are stateless over a database, so each one can be
   replaced while unmodified C# simulators keep running against it. The
   simulator is ported after the services. Every phase ends with something
   that runs in a real grid.
2. **Wire and schema compatibility are the contract.** LLUDP messages,
   capabilities, service connector protocols, OAR/IAR archives and the
   database schema stay compatible. Internals are free to change.
3. **Physics: server-authoritative, engine behind a trait, Rapier as the
   backend.** See `changes/physics-backend/design.md`. Viewers do not compute
   physics; they extrapolate from the server's position and velocity updates,
   so the engine cannot be seen on the wire and can be chosen on merit.
4. **Scripts: embed SLua, Linden Lab's open-source script VM, for both LSL
   and Luau.** See `changes/script-engine`. LSL scripts must be suspended
   mid-event, saved and moved to another region when their object crosses a
   border. SLua (`secondlife/slua`, MIT) is a fork of Luau built for exactly
   that: serialisable execution state, per-script memory limits, hooks for
   pre-emptive scheduling, and LSL compiled onto the same VM through
   Tailslide. Embedding it gives Luau scripting at no extra cost and replaces
   the earlier plan to write a VM. Aurora Viewer's script code already
   refers to Luau.
5. **Database: `sqlx` over the existing schema.** MariaDB/MySQL, PostgreSQL
   and SQLite stay supported. This drops Oracle's `MySql.Data` connector.
6. **Voice: WebRTC only, through an existing Rust voice service.** The Vivox
   module depends on a proprietary service and Aurora Viewer supports WebRTC
   voice only. FreeSWITCH is not ported. The server implements the two voice
   capabilities and forwards them as JSON-RPC to `wolfvoice` (Rust,
   Apache-2.0), run as a sidecar container. The capability handling is ported
   from `os-webrtc-janus` (C#, MPL-2.0).
7. **Licence: GPL-3.0-or-later, and Aurora Viewer's crates are reused**
   (operator decision, 2026-11-13). The "or later" form matches Aurora's own
   manifest. Ported OpenSimulator code keeps its BSD notice, which GPLv3
   permits. Reused, checked at Aurora commit `3d523b1`:
   - `aurora-llsd`: LLSD in XML, binary and notation form. Standalone.
   - `aurora-msg`: packet framing, zero-coding and typed messages generated
     from the message template, with a reader and a writer.
   - `aurora-prim`: prim parameters, texture entries, extra parameters,
     sculpts and the full prim geometry generator.
   - `aurora-assets` (to evaluate): JPEG 2000, mesh assets and mesh physics
     hulls. It builds C code (`openjpeg-sys`).
   - `aurora-net` is the viewer's side of the protocol. It is used for the
     test bot only, not in the server.

   The crates are unpublished (`publish = false`, version 0.1.0) and change
   daily, so they are taken as a git dependency pinned to a commit and
   wrapped, never used directly across the workspace. Where the server needs
   the opposite direction of a message path the viewer only decodes, the
   addition is offered upstream. The Aurora name and logo are trademarks and
   are not used.
8. **Workforce.** Coding and first-pass review go to `beam:cloud` workers.
   Claude writes proposals, reviews adversarially and signs off merges.

## Phases

Each row becomes one or more OpenSpec changes. A change proposal is written
when its phase starts, so later rows are placeholders until then.

| # | Phase | Change id | State |
|---|---|---|---|
| 0 | Repository, CI, container base, conformance harness | `port-foundation` | accepted |
| 0 | Licence and provenance audit | `open-source-audit` | accepted |
| 1 | Types, LLSD, message codec, LLUDP circuit | `protocol-core` | accepted |
| 1 | Physics trait, Rapier backend, parity spike | `physics-backend` | accepted |
| 2 | Robust grid services on the existing schema | `grid-services` | accepted |
| 3 | Scene graph, client stack, capabilities, terrain, persistence | `region-simulator-core` | accepted |
| 3 | Physics shapes from `aurora-prim` geometry, convex decomposition | `prim-meshing` | accepted |
| 2 | Installer, web portal, first-run wizard, map placement, archive import, desktop companion | `grid-portal` | accepted |
| 2 | Host scan and capacity assessment (islands and users) | `capacity-assessor` | accepted |
| 4 | SLua embedding, LSL and Luau, `ll*` API | `script-engine` | accepted |
| 4 | `os*` API | `ossl-api` | accepted |
| 5 | Land, permissions, estate, inventory, attachments, chat, IM, map | `region-modules` | accepted |
| 5 | OAR and IAR archives | `archives` | proposed |
| 6 | Groups, OfflineIM, profiles, NPCs | `social-modules` | proposed |
| 6 | Hypergrid | `hypergrid` | proposed |
| 6 | Grid profiles: standalone, own grid, OSgrid, custom | `external-grid-mode` | accepted |
| 6 | WebRTC voice | `webrtc-voice` | proposed |
| 7 | Migration tooling, load tests, first production grid | `cutover` | proposed |

## Order and dependencies

- `port-foundation` and `open-source-audit` come first and run in parallel.
- `protocol-core` blocks everything that talks to a viewer.
- `physics-backend` starts with a spike that can run before any simulator
  exists. Its result is needed before `region-simulator-core` fixes the scene
  loop.
- `grid-services` needs only `port-foundation` and the LLSD and XML-RPC parts
  of `protocol-core`.
- `script-engine` is the largest single risk. Its spike (build SLua, run an
  LSL corpus) needs no simulator and can run in phase 0.
- `grid-portal` needs `grid-services` and the region control interface.

## How progress is measured

1. Unit tests ported alongside the code they cover (9,924 lines exist).
2. Protocol tests: recorded request and reply pairs from the C# server are
   replayed against the Rust one.
3. Bot tests: `pCampBot` logs in, moves, teleports and crosses regions.
4. Physics trajectories compared against recordings from C# OpenSim.
5. Aurora Viewer sessions against a mixed grid, then an all-Rust grid.

## Grid portal: what exists

No existing project can be dropped in, because each one manages C# OpenSim
processes or reads its database from PHP. Two are worth building on:

| Project | Licence | What it has | Use |
|---|---|---|---|
| `ystudy/GridGate-OpenSim` | BSD (OpenSimulator fork, .NET 10) | Browser admin: grid map with drag-and-drop region placement, sign-up approval, staff roles, two-factor sign-in, estates, hypergrid links, one-command install. Very new: no stars, one main author, first release candidate | Model for the portal; its HTML and JavaScript front end can be adapted under BSD |
| `GuduleLapointe/w4os` | AGPL-3.0 (WordPress, PHP) | Avatar registration, profiles, in-world search, destination guide. Active | Runs unchanged beside the Rust services, because the schema is kept |
| `GwynethLlewelyn/goswi` | Licence file not found | Go admin console over the database | Reference only until licensed |
| `ManfredAabye/oswebinterface` | MIT (PHP) | Viewer splash and login pages | Optional |
| `soup-bowl/opensimulator-docker` | MIT | Container build for C# OpenSimulator | Reference server for conformance tests |

Plan: the `robust` binary serves its own portal (`axum`), with GridGate's
page set as the model, and w4os stays a supported companion for grids that
want a WordPress front. HTTP/2 was dropped from the list below: Aurora's
manifest builds `reqwest` without its `http2` feature.

## Improvements to fold in (candidates, not yet committed)

None of these changes what a viewer or a C# simulator sees. Licence, version
and last release of every crate named here were read from crates.io on
2026-11-13. All licences are GPLv3-compatible. Four have had no release for
over a year and need a second look before use: `gltf` (2024-05), `noise`
(2024-03), `irc` (2025-03, MPL-2.0) and `prometheus` (2025-03).

Modernisation, done in the phase that touches the code:

| Item | C# today | Rust approach | Phase |
|---|---|---|---|
| Password storage | Salted MD5 (`AuthenticationServiceBase`) | Argon2id over the MD5 digest the viewer sends (`argon2`); opt-in after cutover, because C# cannot read it | 2, 7 |
| TLS | Optional, manual certificates | `rustls` with automatic certificates | 0 |
| Metrics and tracing | None | Prometheus, OpenTelemetry | 0 |
| Asset store | Database blobs or gzip files keyed by SHA-256 | Same keys, zstd, optional S3-compatible storage (`object_store`) | 2 |
| Concurrency | Locks in 226 files | `tokio` tasks and message passing per region | 3 |
| Script limits | Cooperative | CPU and memory metering in the VM | 4 |
| Convex decomposition | `ConvexDecompositionDotNet` (5,701 lines) | V-HACD in `parry3d` | 3 |
| Map tiles | Warp3D software renderer | `tiny-skia` or headless `wgpu` | 5 |
| Region save and restore | Restart | Physics and scene snapshot | 3 |
| Abuse limits | `BasicDOSProtector` | `governor` rate limits, fuzzed decoders | 1 |

New features that are mostly connectors:

| Feature | Existing piece | Cost |
|---|---|---|
| WebRTC voice | `wolfvoice` sidecar plus two capabilities | Medium |
| Luau scripts beside LSL | Comes with SLua | Included in `script-engine` |
| Walkable web view of a region | `Klaatou/Web_3d` (MIT, Python) converts an OAR to glTF | Low, as a sidecar |
| Search for places, people, classifieds | `tantivy` | Low |
| Chat bridges (Matrix, IRC, Discord) | `matrix-sdk`, `irc`, `serenity` | Low each |
| Script email | `lettre` | Low |
| Web admin and REST API with OpenAPI | `axum`, `utoipa` | Low to medium |
| Scheduled OAR backups to object storage | `object_store` | Low |
| glTF export of objects and regions | `gltf`; Aurora has an importer | Low to medium |
| Procedural terrain | `noise` | Low |
| Single sign-on for web tools | `openidconnect` | Low |
| Region modules as sandboxed plug-ins | `wasmtime` | Medium |

## Open questions

- D1: resolved, GPL-3.0-or-later with Aurora crates reused.
- OSgrid: whether it accepts regions run on third-party software. Its FAQ
  refused automated access; the administrators must be asked.
- D2: resolved. Repository is `opensim-rust` at
  `/var/home/erik/Projects/opensim-rust/`, local git only, no remote
  (operator decision, 2026-11-13). The C# reference tree stays at
  `/var/home/erik/Projects/opensim-0.9.3.0-source/`.
- Provenance of `bin/openmetaverse_data`, the `bin/assets` library and the
  protocol message template (all originate with Linden Lab).
- Pathfinding is not planned (operator decision, 2026-11-13).
