# Phase 0 summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Decisions](decisions.md) · Next: [Protocol core](protocol-core-summary.md)

Changes: `port-foundation` and `open-source-audit` (both accepted). No
runtime behaviour of the C# server was ported; the aim was a home, a build,
containers, a way to prove equivalence, and a clean provenance record.

## Repository (port-foundation 1)

- Initial commit `9d600e0`: Cargo workspace with 19 crates (now 21), GPLv3
  `LICENSE`, `NOTICE.md` (OpenSimulator BSD notice and Aurora attribution,
  added in `abd7b93`), and the `openspec/` tree moved in.
- Local git only, no remote (decision D2).
- `scripts/check.sh` runs fmt, clippy with `-D warnings`, tests and
  `cargo deny check`. There is no hosted CI workflow in the repo yet; this
  script is the gate. `deny.toml` was fixed for cargo-deny 0.20 and carries
  advisory ignores (`eb47aa9`).

## Runtime base (port-foundation 2)

`welkin-config`: TOML with environment overrides, plus an importer for
`OpenSim.ini`, `Robust.ini` and `Regions/*.ini`. `welkin-runtime`:
`tracing` logs, Prometheus metrics endpoint, health endpoint, graceful
shutdown (commit `efdde1e`).

## Containers (port-foundation 3)

Root `Containerfile` (multi-stage, static, non-root), `compose.yaml`
(database, `robust`, one region) and `compose.reference.yaml` (unmodified C#
server as reference), commit `7d9b2fc`. The compose files document that the
published UDP port must equal the advertised one.

## Conformance harness (port-foundation 4)

`welkin-conformance`: HTTP and XML-RPC recorder (`7d9b2fc`), LLUDP
recorder/replayer in `src/lludp.rs` and a `pCampBot` wrapper with pass/fail
output in `src/pcampbot.rs` (`c779319`). A recorded C# session is stored at
`crates/welkin-conformance/test-data/lludp_session.jsonl`.

## Licence audit (open-source-audit)

Output: `docs/licence-audit.md`, `deny.toml`, `NOTICE.md` (`fee4e61`,
`abd7b93`).

- D1: LGPL-3.0-or-later (updated 2026-11-15, originally GPL-3.0) so Aurora crates can be reused. Ported
  BSD-3-Clause code keeps its notice.
- Aurora crates audited at `3d523b1`; all GPL-3.0-or-later. The message
  template is LGPL 2.1 from Linden Lab, which settles its provenance.
- Dropped: Vivox and FreeSWITCH voice, Oracle `MySql.Data` (to `sqlx`),
  ODE/Bullet (to Rapier), CSJ2K (non-BSD JJ2000 field-of-use notice), and
  various Ms-PL/MPL helper libraries replaced by Rust crates.
- Open: Warp3D has no licence in the tree (reimplement); provenance of
  `bin/openmetaverse_data` and `bin/assets` still unconfirmed.

## Open tasks

In `openspec/changes/open-source-audit/tasks.md`, 1.2 and most of 2 and 3 are
not ticked although `docs/licence-audit.md` and `deny.toml` exist; the task
list needs reconciling. Not done: JPEG 2000 evaluation (2.5), Aurora
`[patch.crates-io]` review (2.6), naming and trademark rules (3.3).
