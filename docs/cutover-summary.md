<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Cutover summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Migration guide](migration-guide.md) · [Operator guide](operator-guide.md)

Status: accepted (19 of 19 tasks, 7 sections). Change:
`openspec/changes/cutover/`.

## What was built

Migration tooling, load testing, protocol conformance validation, mixed-grid
support, container hardening, and the first production grid deployment. This
phase ports no new C# code. It verifies that everything ported in prior phases
works end-to-end and provides a path for existing C# grids to migrate
incrementally.

| Section | Area | Tasks |
|---|---|---|
| 1 | Migration tooling: DB validator, password upgrade, config converter | 1.1-1.3 |
| 2 | Bot framework: pCampBot port, scripted action sequences | 2.1-2.2 |
| 3 | Protocol conformance: LLUDP replay, service connector replay | 3.1-3.2 |
| 4 | Load testing: harness, C# baselines, Rust comparison | 4.1-4.3 |
| 5 | Mixed-grid validation: C# region on Rust Robust, Rust region on C# Robust, cross-region teleport | 5.1-5.3 |
| 6 | Container hardening: production images, compose stack | 6.1-6.2 |
| 7 | Production grid, operator guide, migration guide, final review | 7.1-7.4 |

## Architecture

| Module | Responsibility |
|---|---|
| `welkin-migration/db_validator.rs` | Connect to existing C# grid database, verify schema compatibility, generate ALTER statements (863 lines) |
| `welkin-migration/password_upgrade.rs` | Batch Argon2id hash generation over existing MD5 digests with rollback flag for mixed operation (459 lines) |
| `welkin-migration/config_converter.rs` | Parse C# OpenSim.ini + Robust.ini, generate Rust configuration, flag unsupported options (517 lines) |
| `welkin-conformance/pcampbot.rs` | Login, walk, fly, teleport, cross regions, chat, group join via aurora-net viewer protocol |
| `welkin-conformance/bot_scripting.rs` | Programmable action sequences for regression testing (534 lines) |
| `welkin-conformance/protocol_replay.rs` | Replay recorded C# LLUDP and cap request/response pairs, diff outputs, categorize divergences |
| `welkin-conformance/service_conformance.rs` | Replay C# Robust HTTP requests against Rust Robust, verify format and content match |
| `welkin-conformance/load_test.rs` | Configurable concurrent avatars, physics objects, scripts, asset transfers with baseline comparison (795 lines) |
| `welkin-conformance/mixed_grid.rs` | C# region against Rust Robust, Rust region against C# Robust, cross-region teleport in mixed grid |

## Migration path

The design assumes incremental migration, not a single cutover:

1. Deploy Rust Robust alongside C# Robust on a different port
2. Point one test region at Rust Robust and validate services
3. Run pCampBot and protocol replay to find divergences
4. Migrate all services to Rust Robust
5. Replace C# regions one at a time with Rust regions
6. Import OAR archives for content migration
7. Cut over DNS or load balancer

## Key decisions

- C1: Schema stays compatible throughout. No breaking migrations. The Argon2id
  password upgrade is opt-in and preserves MD5 fallback for C# interop.
- C2: pCampBot ported in Rust using `aurora-net` for viewer protocol, reusing
  existing test infrastructure rather than a separate binary.
- C3: Load test targets defined against C# measurements on identical hardware.
  The Rust server must meet or exceed C# on avatars per region, script
  executions per second, asset throughput, and region crossing latency.
- C4: Mixed-grid is the primary migration strategy. No big-bang cutover required.

## Tests

131 tests across the conformance and migration crates:

| Module | Tests |
|---|---|
| db_validator | 15 |
| password_upgrade | 12 |
| config_converter | 12 |
| pcampbot | 16 |
| bot_scripting | 10 |
| protocol_replay | 14 |
| service_conformance | 10 |
| load_test | 15 |
| mixed_grid | 17 + cross-region |

Final acceptance review passed all 12 subsystems, 1844 tests across the full
project, zero failures.

## Documentation produced

- [Test grid deployment](test-grid-deployment.md) (215 lines)
- [Operator guide](operator-guide.md) (606 lines)
- [Migration guide](migration-guide.md) (477 lines)
- [Final acceptance review](final-acceptance-review.md)
