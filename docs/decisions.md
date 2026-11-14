# Decision log

[← Back to docs](README.md) · [Architecture](architecture.md)

Sources: `openspec/ROADMAP.md` (Decisions, Open questions) and the change
proposals. Neither `port-foundation`, `open-source-audit` nor `protocol-core`
has a `design.md`; their rationale comes from `proposal.md`. Dates are the
dates recorded in the source, or the ROADMAP's last-updated date (2026-11-13)
where none is given.

<details>
<summary>Full decision table (97 entries — click to expand)</summary>

| ID | Decision | Rationale | Date |
|---|---|---|---|
| R1 | Incremental replacement, not a single rewrite | Grid services are stateless HTTP/XML-RPC over a database, so each can be swapped while C# simulators run; every phase ends in something runnable | 2026-11-13 |
| R2 | Wire and schema compatibility is the contract (LLUDP, caps, service protocols, OAR/IAR, DB schema) | Existing grids must migrate without losing content; internals may change | 2026-11-13 |
| R3 | Physics: server-authoritative, engine behind a trait, Rapier backend | Viewers extrapolate from server updates, so the engine is invisible on the wire and can be chosen on merit | 2026-11-13 |
| R3a | Rapier 0.36.1 confirmed as the physics engine after the parity spike (GO); Jolt remains the fallback behind the same trait. Vehicle and avatar behaviour are ported once above the trait from `ODEDynamics.cs` and `ODECharacter.cs` | Pure Rust, no FFI or C toolchain; native ubODE/BulletSim would still need the C# behaviour layers rewritten. Spike: collider types, CCD, snapshot and Aurora geometry all work (see `docs/physics-backend-summary.md`) | 2026-11-13 |
| R4 | Scripts: embed SLua (Luau fork) for LSL and Luau | Serialisable execution state, memory limits and pre-emption hooks are needed for scripts that cross regions; gives Luau for free and replaces writing a VM | 2026-11-13 |
| R5 | Database: `sqlx` over the existing schema (MySQL/MariaDB, PostgreSQL, SQLite) | Keeps schema compatibility; drops Oracle `MySql.Data` | 2026-11-13 |
| R6 | Voice: WebRTC only, via `wolfvoice` sidecar | Vivox is proprietary; Aurora supports WebRTC only; FreeSWITCH not ported | 2026-11-13 |
| R7 / D1 | Licence GPL-3.0-or-later, reuse Aurora crates | Lets Aurora's GPL crates be linked; "or later" matches Aurora's manifest; BSD notices of ported code are preserved; AGPL not adopted because GPLv3 does not oblige network-only operators to publish and that was judged sufficient | 2026-11-13 |
| R8 | Workforce: `beam:cloud` workers code and first-review; Claude writes proposals and signs off | Cost and review discipline | 2026-11-13 |
| D2 | Repository `opensim-rust`, local git only, no remote | Operator decision | 2026-11-13 |
| R9 | Pathfinding not planned | Operator decision | 2026-11-13 |
| A1 | Aurora crates are pinned git dependencies at `3d523b1` and always wrapped | Unpublished, version 0.1.0, change daily; wrapping confines API churn to one crate | 2026-11-13 |
| A2 | `aurora-net` used for the test bot only | It is the viewer side of the protocol | 2026-11-13 |
| A3 | Gaps in Aurora (viewer-only decode paths) are filled here and offered upstream | Aurora is a viewer; server needs the opposite direction | 2026-11-13 |
| P1 | Config in TOML with env overrides, plus INI importer | Replaces Nini; existing `OpenSim.ini`/`Robust.ini` must be importable | 2026-11-13 |
| P2 | Observability via `tracing` and Prometheus | Replaces log4net and the stats console | 2026-11-13 |
| P3 | One multi-stage `Containerfile`, static binaries, non-root, Podman and Docker | Portable images, small attack surface | 2026-11-13 |
| P4 | Conformance harness: record C# request/reply pairs, replay against Rust, `pCampBot` wrapper | Without it phases are judged by eye | 2026-11-13 |
| P5 | Throttle and resend behaviour match `LLUDPClient`/`LLUDPServer` closely | Viewer would stall or flood otherwise (risk MEDIUM) | 2026-11-13 |
| P6 | Updating the Aurora pin is a reviewed change gated by replay tests | Protects against upstream churn | 2026-11-13 |
| L1 | Drop Vivox, FreeSWITCH, Oracle `MySql.Data`, CSJ2K, Warp3D (reimplement) | Proprietary, legacy, or non-compatible/unknown licences (see `docs/licence-audit.md`) | 2026-11-13 |
| S1 | Data layer is one `sqlx` store per service, with the C# schema files embedded and applied by a migration runner (`e3f9c79`, `33c5026`) | Implements R5; a C#-created database opens unchanged and all three backends share one API | 2026-11-13 |
| S2 | Handlers reproduce the C# Robust wire format exactly, checked by replay fixtures (`c7229de`, `9593604`) | R2; mixed grids with C# simulators must keep working | 2026-11-13 |
| S3 | Simulation service is a 501 stub | Not needed for login or the replayed flows; deferred to the region phase | 2026-11-13 |
| S4 | Password hashes are compared in constant time (`840f769`) | Security review finding M1: `==` on MD5 hashes leaks timing | 2026-11-13 |
| S5 | Admin-style endpoints are not gated in code; deployments must restrict them by network or proxy | Security review noted it; matches C# Robust, which relies on the same arrangement | 2026-11-13 |
| C1 | Capacity estimate comes from sourced coefficients now; the on-host benchmark, calibration and live refinement are stubs until `region-simulator-core` (`97f5ebd`, `880338a`) | The benchmark needs a running simulator; the proposal already allows a labelled estimate before that. Result carries `is_measured: false` | 2026-11-13 |
| C2 | Estimate is a range of 80% to 120% of the point value, reserves one core, and names the limiting resource (`97f5ebd`) | Proposal risk is MEDIUM: an overestimate overloads a machine, so no unqualified number is shown | 2026-11-13 |
| C3 | Network speed and public IP are manual configuration for now (`97f5ebd`) | Speed test and address detection deferred; bandwidth limit applies only when upload speed is set | 2026-11-13 |
| G1 | Desktop companion is a Tauri 2 app, optional, excluded from the Cargo workspace (`e8ddfd8`) | Operator decision; one set of portal pages serves browser and control window, and `tray-icon` covers Windows, macOS and Linux. Needs WebKitGTK on Linux; GNOME needs a tray extension | 2026-11-13 |
| G2 | Portal staff and sign-up passwords use argon2; sign-up passwords are hashed at registration (`ddba6d0`, `59eea64`) | Security review H3: plaintext in memory is not acceptable. TOTP rejects accounts that have a secret until implemented (H1), which fails closed | 2026-11-13 |
| G3 | Portal follows the GridGate model: Admin/Operator/Viewer roles, sign-up with approval, map placement, one-command install (`ddba6d0`) | BSD-licensed model in the roadmap, so pages and flow can be adapted; roles enforced in code (H2) | 2026-11-13 |
| G4 | Portal is not for public exposure until security review items M1 to M5 are fixed; restrict by network or proxy meanwhile | Review assessment; same arrangement as S5 | 2026-11-13 |
| M1 | Box and sphere prims use analytic colliders; other prims use aurora-prim trimeshes with LOD chosen by size | Cheaper and exact for the common case; LOD thresholds match C# Meshmerizer | 2026-11-13 |
| M2 | Convex decomposition uses parry3d V-HACD, with C# voxel budget mapped to a grid size clamped to 16 to 256, results cached | No new dependency; C# parameters kept for parity (`max_vertices_per_hull` advisory only) | 2026-11-13 |
| M3 | Sculpt textures come through a `SculptTextureSource` trait; JPEG 2000 decoding deferred | Keeps meshing testable and leaves the `aurora-assets` versus pure Rust question open | 2026-11-13 |
| M4 | Mesh asset decoding is treated as untrusted: indices validated, zlib output capped at 32 MiB, cache mutexes recover from poisoning (`1812e45`) | Security review CRITICAL-1, HIGH-1, MEDIUM-3 | 2026-11-13 |
| N1 | Scene objects persist and interchange as XML2, round-tripped against C# output | R2; lets a C# region database and OAR content load unchanged | 2026-11-13 |
| N2 | `SimulationDataStore` trait in `opensim-data` with one `sqlx` implementation over the existing `regionstore` tables | R5; no migration for C#-created region databases | 2026-11-13 |
| N3 | LLUDP viewer messages dispatch through a table keyed on message id to typed handler functions, not a large class | Replaces the 13k-line `LLClientView`; handlers are testable alone | 2026-11-13 |
| N4 | Several `Scene`s per process via `RegionManager`; caps HTTP server runs beside the LLUDP listener | Matches C# multi-region hosting | 2026-11-13 |
| N5 | Terrain modification brushes ship with the scene crate, not a later module | Heightfield belongs to the scene graph and OAR load needs it | 2026-11-13 |
| N6 | Region simulator accepted as CONDITIONAL PASS: client map and mpsc channels must be bounded and stale sessions reaped before production; restrict by network meanwhile | Second review, 3 HIGH findings in the net stack; same arrangement as S5 and G4 | 2026-11-13 |
| E1 | Grid choice is a TOML profile (`standalone`, `own-grid`, `custom`); Hypergrid is a switch inside a profile (`8f35477`) | The grid a region joins must be configuration, not code |
| E2 | The OSgrid profile is generated by importing OSgrid's own `GridCommon.ini`, not hand-written | Addresses are never copied from memory and can be refreshed |
| E3 | Hypergrid ships as six traits with stubs, no live implementation (`91a1147`) | Fixes the protocol surface now; behaviour needs a live grid to verify |
| E4 | OSgrid setup rules are wizard checks, not documentation only (`91a1147`) | Name, coordinate, estate owner and port mistakes are caught before registration |
| E5 | Replay recording (3.2) and live attach (3.4) deferred to Phase 7 cutover (`0b0f587`) | Needs live C# region and grid infrastructure and an answer from OSgrid |
| E6 | External-grid mode accepted as CONDITIONAL PASS: SSRF fix and the 4 HIGH fixes are required before any untrusted grid; `osgrid` and `custom` stay disabled meanwhile (`e46a2dd`) | Security review of remote write paths, hostile-grid threat model |
| V1 | Go on Luau via mlua 0.12, not a custom VM; LSL runs through a transpiler to Luau (`d413580`) | Spike passed: yield, memory limit, interrupt pre-emption and state serialisation all work | 2026-11-13 |
| V2 | Script state persists as bytecode plus globals JSON (checkpoint pattern); coroutine stacks are not saved (`3fb1cbd`) | Luau cannot snapshot a coroutine stack; scripts restart their event after transfer | 2026-11-13 |
| V3 | One Luau VM per script | Strong isolation; memory limit applies per script | 2026-11-13 |
| V4 | `os*` functions are gated by an 8-level `OsslThreatLevel` with a per-function allow override (`a5539f5`) | Mirrors C# OSSL threat levels so existing grid configuration carries over | 2026-11-13 |
| V5 | `ll*` functions not yet needed are registered as stubs (33 of 82) | Keeps the function table complete so scripts compile and run | 2026-11-13 |
| V6 | Script engine accepted as CONDITIONAL PASS: freeze globals with `sandbox(true)` in `ScriptInstance` and bound the event queue before production (`dd1177d`) | Second review, 2 HIGH findings; same arrangement as S5 and G4 | 2026-11-13 |
| O1 | `os*` functions are gated at registration: a function that fails the threat check is never placed in the Lua globals | Stronger than the C# call-time check; no bypass through a missed check. Cost: role checks need caller context (see O6) | 2026-11-13 |
| O2 | Access is a global threat ceiling plus per-function `FunctionPerm` (`Disabled`, `Everyone`, `AllowList` of roles and UUIDs) | Reproduces C# `Allow_<Function>` semantics so existing INI files carry over | 2026-11-13 |
| O3 | Macros (`osslParcelO`, `osslParcelOG`, `osslNPC`) are expanded when the INI is loaded | Keeps `is_allowed()` free of string handling; expansion needs a depth bound (review S7) | 2026-11-13 |
| O4 | Functions whose subsystem is absent are registered as stubs that log and return a default; 233 registered, catalog test fixed | Scripts compile and run; each stub is replaced by the change that adds its module (same approach as V5) | 2026-11-13 |
| O5 | AES key and IV derive by PBKDF2-HMAC-SHA1, 1000 iterations, salt `OpenSimulator OSSL`, not SHA256 (review S1) | .NET `Rfc2898DeriveBytes` uses SHA1; SHA256 broke ciphertext interchange with C# regions | 2026-11-13 |
| O6 | OSSL accepted as CONDITIONAL PASS: commit the S1 fix, enforce role checks at call time (S2) and remove `expect()` from crypto (S3) before production | Second review; same arrangement as S5 and G4 | 2026-11-13 |
| RM1 | Region modules live in one crate, `opensim-modules`, split by module directory | Single dependency for the region binary; shorter than the `opensim-region-modules` name in the design | 2026-11-13 |
| RM2 | Modules implement a `RegionModule` trait with `Shared`/`NonShared` types and a four-step lifecycle | Maps C# `IRegionModuleBase`, `ISharedRegionModule` and `INonSharedRegionModule` one to one | 2026-11-13 |
| RM3 | Interfaces are found through a `TypeId`-keyed registry, not reflection or globals | Rust has no runtime type discovery; explicit registration is checkable | 2026-11-13 |
| RM4 | Module-to-module notifications use a typed `tokio::sync::broadcast` event bus (`SceneEvents`) | Subscribers are independent; replaces C# scene event delegates | 2026-11-13 |
| RM5 | One tiny-skia renderer replaces LegacyMap and Warp3DMap | One pure Rust path, no GDI or Warp3D dependency | 2026-11-13 |
| RM6 | FlotsamAssetCache uses a HashMap and VecDeque LRU, no `moka`; disk tier is a stub | Simple and sufficient for now | 2026-11-13 |
| RM7 | Hypergrid parts, OAR/IAR and groups/profiles are out of this change (`hypergrid`, `archives`, `social-modules`) | Keeps the change bounded | 2026-11-13 |
| RM8 | Region modules accepted as CONDITIONAL PASS: fix C-1 (nil UUID) and H-1 before a multi-user grid, H-2, H-3 and M-4 before production; restrict by network meanwhile (`d33ef2c`) | Opus adversarial review of permissions; same arrangement as S5 and G4 | 2026-11-13 |

| AR1 | Replace C# custom tar (455 lines) with `tar` + `flate2` crates | Crate handles edge cases (long paths, symlinks, permissions) correctly; less code to maintain | 2026-11-14 |
| AR2 | Async asset gathering with `tokio::time::timeout` replaces C# `ManualResetEvent` + timer | Idiomatic Rust async; same timeout semantics | 2026-11-14 |
| AR3 | OAR load options (merge, displace, rotate, bbox) are a flags struct, not positional arguments | Type-safe; matches the C# `Dictionary<string, object>` semantics without the stringly-typed API | 2026-11-14 |
| AR4 | OspResolver writes `first.last` profiles on save and resolves on load, with UserManagement fallback | Preserves cross-grid UUID portability; fallback handles grids without profile data | 2026-11-14 |
| AR5 | Path traversal protection: all tar paths validated against expected prefix before processing | Defense in depth over `tar` crate's built-in protection; rejects `../`, absolute, null bytes, drive letters | 2026-11-14 |
| AR6 | Archives accepted as CONDITIONAL PASS: fix M-1 through M-5 before production (decompression bomb limit, Vec allocation cap, reject non-UTF-8 paths, null byte check, drive-letter check) | Opus adversarial review; same arrangement as S5 and G4 | 2026-11-14 |

| SM1 | Port Groups V2 only; drop legacy XmlRpc connector (3,991 lines) | V2 is cleaner, supports HG, and is the default in 0.9.3.0 | 2026-11-14 |
| SM2 | NPCAvatar implements only methods actually called by NPCModule; rest are no-ops | C# has ~1,400 stub lines; 241 lines is sufficient for the real call paths | 2026-11-14 |
| SM3 | IRC bridge uses `irc` crate (MPL-2.0) instead of porting C# IRC client | MPL-2.0 is GPLv3-compatible; proven crate, less code to maintain | 2026-11-14 |
| SM4 | OfflineIM uses same `sqlx` backend as other services | No separate storage engine needed; consistent with grid-services pattern | 2026-11-14 |
| SM5 | Social modules accepted as CONDITIONAL PASS: fix H-1 (IRC injection), H-2 (group creation power), H-3 (notice attachment size) before production; IRC bridge must not go live until H-1 is resolved | Opus adversarial review; same arrangement as S5 and G4 | 2026-11-14 |

| H1 | HG modules extend base modules via trait composition, not separate implementations; `HGEntityTransferModule` wraps `EntityTransferModule` | Base modules are already ported; composition avoids duplication and keeps overrides explicit | 2026-11-14 |
| H2 | UUI format (`uuid;griduri;name`) stored alongside local UUIDs; `HGUserManagementModule` resolves `first.last@griduri` to local cache | Matches C# HG identity model; cache avoids repeated cross-grid lookups | 2026-11-14 |
| H3 | Suitcase inventory uses path validation with root-anchored check to prevent directory traversal | Defense in depth over folder-ID checks; foreign visitors must not escape their suitcase | 2026-11-14 |
| H4 | Traveling agent sessions use cryptographically random tokens, single-use, expire on logout | Prevents replay and prediction; stronger than C# which uses predictable session IDs | 2026-11-14 |
| H5 | HELO probe discovers HG capability before attempting cross-grid operations, with timeout and retry | Avoids wasting resources on grids that don't support HG; graceful degradation | 2026-11-14 |
| H6 | Hypergrid accepted as CONDITIONAL PASS (CRITICAL RISK): fix F-01 (suitcase escape), F-02 (VerifyAgent), F-03 (UUID impersonation) and all HIGH findings before any public-facing deployment | Opus adversarial review; same arrangement as S5 and G4 | 2026-11-14 |

| V1 | JSON-RPC over HTTP to wolfvoice, not gRPC | Simpler, no protobuf dependency, matches wolfvoice's documented API | 2026-11-14 |
| V2 | Channel URIs use `webrtc:region/parcel` format, not SIP URIs | Aurora Viewer expects WebRTC format | 2026-11-14 |
| V3 | Voice credentials are ephemeral per-session, not stored in the database | wolfvoice manages credential lifecycle; no new DB columns | 2026-11-14 |
| V4 | Estate and parcel voice-enable checks reuse existing flags from region-modules | No new database columns needed | 2026-11-14 |
| WV1 | JSON-RPC over HTTP to wolfvoice, not gRPC | Simpler, no protobuf dependency, matches wolfvoice's documented API | 2026-11-14 |
| WV2 | Channel URIs use `webrtc:region/parcel` format, not SIP URIs | Aurora Viewer expects WebRTC format | 2026-11-14 |
| WV3 | Voice credentials are ephemeral per-session, not stored in the database | wolfvoice manages credential lifecycle; no new DB columns | 2026-11-14 |
| WV4 | Estate and parcel voice-enable checks reuse existing flags from region-modules | No new database columns needed | 2026-11-14 |
| WV5 | WebRTC voice accepted as CONDITIONAL PASS: fix H-1 (wolfvoice auth) and H-2 (rate limiting) before production; wolfvoice sidecar must not accept unauthenticated JSON-RPC | Opus adversarial review; same arrangement as S5 and G4 | 2026-11-14 |

| HG1 | HG modules extend base modules via trait composition, not separate implementations | `HGEntityTransferModule` wraps `EntityTransferModule`; avoids code duplication | 2026-11-14 |
| HG2 | UUI format (`uuid;griduri;name`) stored alongside local UUIDs | `HGUserManagementModule` resolves `first.last@griduri` to local cache; matches C# format | 2026-11-14 |
| HG3 | Suitcase inventory uses root-anchored path validation to prevent escape | Defense in depth over folder-ID checks; prevents directory traversal | 2026-11-14 |
| HG4 | Traveling agent sessions use cryptographically random tokens, single-use, expire on logout | Prevents session replay and token prediction attacks | 2026-11-14 |
| HG5 | HELO probe discovers HG capability before cross-grid operations, with timeout and retry | Avoids sending HG requests to non-HG grids | 2026-11-14 |
| HG6 | Hypergrid accepted as CONDITIONAL PASS (CRITICAL RISK): fix F-01 (suitcase escape), F-02 (VerifyAgent), F-03 (UUID impersonation) before any multi-user grid; fix F-04 through F-06 before production | Opus adversarial review; same arrangement as S5 and G4 | 2026-11-14 |

</details>

## Open questions

- OSgrid: whether it accepts regions on third-party software; administrators
  must be asked.
- Provenance of `bin/openmetaverse_data`, `bin/assets` and the message
  template header.
- JPEG 2000: `aurora-assets` (`openjpeg-sys`, C) versus a pure Rust decoder.
