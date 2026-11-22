# Tasks: Welkin recovery

Each box is one work package: one branch, one PR. Tick a box only with an `Evidence:` line under it (PR number plus CI run or test name). See `design.md` section 6 for the detail of each package and section 7 for the evidence rules.

## Phase 0: Stabilise

- [ ] 0.1 Replace the login throttle: peer address, trusted-proxy list, failed attempts only, bounded per-IP and per-account limiters (HIGH RISK)
- [ ] 0.2 Freeze autonomous runners; remove gpt-oss and other banned models from repo configs, hooks and project memory
- [ ] 0.3 Repo cleanup: delete root `src/`, root `tests/*.rs`, `crates/integration_tests`; untrack scratchpad, `__pycache__`, caches; fix `.gitignore` and `.dockerignore`
- [ ] 0.4 CI orphan-file check: every tracked `.rs` belongs to a workspace package; no `#[path]` shims into `src/`
- [ ] 0.5 Quarantine factory leftovers in crates: GetMesh2 log-only field, EEP/PBR `#[path]` shims and dead-code allows, hollow `stub_tests_*` modules and `assert!(true)` tests (the other 26 `stub_tests_*` modules have real assertions and are triaged in 0.15), conformance `#[path]` includes, duplicate AbuseGuard
- [x] 0.6 glTF parser: not landed. `2492ef1` has no callers, reads only the first primitive, and its `gltf` `import` feature resolves external URIs (local file reads on uploads). Kept at tag `salvage/gltf-parser`; a proper import is Phase 11 work.
  Evidence: decision recorded 2026-11-22; tag `salvage/gltf-parser` on origin.
- [ ] 0.7 Toolchain and format: `rust-toolchain.toml`, `rustfmt.toml`, `.editorconfig`, `.cargo/config.toml`; one `cargo fmt --all` commit
- [ ] 0.8 CI rebuild: fmt, clippy, test, deny, doc, MSRV, `--locked`, widened stub gate, `#[allow]` justification check
- [ ] 0.9 Single Containerfile on the pinned toolchain; delete `Dockerfile` and `Dockerfile.region`; CI image build
- [ ] 0.10 Mount `/health` on robust and region; default tracing init in both binaries
- [ ] 0.11 E2E smoke job: robust + region on SQLite in CI, realmwalker integration suite
- [ ] 0.12 Fake-success caps, conformance stubs and unwired script channels fail honestly; robust refuses Hypergrid-enabled config with an error, not a panic
- [ ] 0.13 llRequestPermissions grants only what OpenSim auto-grants; deny otherwise (HIGH RISK)
- [ ] 0.14 Truth reset: untick contradicted boxes across all changes, add Evidence lines to the rest, fix ROADMAP, README and CHANGELOG, one issue per critical and high finding
- [ ] 0.15 Audit the 26 "real implementations" commits, the `*_proof.rs` suites, and oversized PRs #29, #31 and #33 by area; one issue per fake
- [ ] 0.16 GitHub settings: CODEOWNERS, required reviews, code-owner review, enforce_admins, secret scanning, dependabot (needs the owner)
- [ ] 0.17 Prod compose fix: correct env keys and DSN, mount config, one compose file with profiles; move `AllowCreateUser` to a dev example
- [ ] 0.18 Branch cleanup after 0.6 and 0.15: delete `df-*`, `realmwalker-phase3` and `dev-clean` with their worktrees (see `audit-branches.md`)

Circuit authentication (finding C2) and per-agent capabilities (finding C3) are Phase 2 work packages 2.5 and 2.6: they need the Simulation in-connector (2.3) and login calling CreateAgent (2.4) to exist first.

## Phase 1: Backbone

Tasks to be written from `design.md` section 6 when Phase 0 is done.

## Phases 2-11

Tasks to be written per phase, in the order given in `design.md` section 6.
