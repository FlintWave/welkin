# Decision log

Sources: `openspec/ROADMAP.md` (Decisions, Open questions) and the change
proposals. Neither `port-foundation`, `open-source-audit` nor `protocol-core`
has a `design.md`; their rationale comes from `proposal.md`. Dates are the
dates recorded in the source, or the ROADMAP's last-updated date (2026-11-13)
where none is given.

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

## Open questions

- OSgrid: whether it accepts regions on third-party software; administrators
  must be asked.
- Provenance of `bin/openmetaverse_data`, `bin/assets` and the message
  template header.
- JPEG 2000: `aurora-assets` (`openjpeg-sys`, C) versus a pure Rust decoder.
