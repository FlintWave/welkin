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

## Open questions

- OSgrid: whether it accepts regions on third-party software; administrators
  must be asked.
- Provenance of `bin/openmetaverse_data`, `bin/assets` and the message
  template header.
- JPEG 2000: `aurora-assets` (`openjpeg-sys`, C) versus a pure Rust decoder.
