# grid-services summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Decisions](decisions.md#s1) · Previous: [Physics backend](physics-backend-summary.md) · Next: [Capacity assessor](capacity-assessor-summary.md)

Change `grid-services` (accepted; all 14 tasks ticked). It replaces the C#
Robust grid services with axum handlers over a `sqlx` data layer. The change
has no `design.md`; the rationale comes from `proposal.md` and the commit
messages.

## 1. Data layer (`welkin-data`)

- `sqlx` stores (`e3f9c79`): 13 stores covering accounts, agent preferences,
  assets, auth, avatar, estate, friends, grid, grid user, inventory, mute,
  presence and profiles. One module per store, with a shared pool in
  `pool.rs`. Three backends (MariaDB/MySQL, PostgreSQL, SQLite) sit behind
  the same store API.
- Migration runner (`33c5026`, `migrations.rs`): 53 embedded schema files
  taken from the C# tree. A database created by C# OpenSim opens unchanged,
  which is the compatibility contract in decision R2.

## 2. Service handlers (`welkin-services`)

Fourteen handlers, each in its own module, served by axum. Wire format
follows C# Robust, as recorded by the conformance recorder.

| Handler | Commit | Notes |
|---|---|---|
| Asset | `2d4be39` | 7 REST endpoints |
| Inventory | `9a461fe` | XInventory, 21 methods |
| Grid, presence, griduser | `cdad250` | |
| Accounts, auth, authorization | `de7dc23` | |
| Friends, avatar, mute, profiles | `a3f4fdc` | Mute list uses CRC32 change detection; profiles speak JSON-RPC 2.0 with 18 methods |
| Map images, estate, agent preferences, simulation | `2b0a212` | Simulation is a 501 stub |
| Login | `9b7c31b` | XML-RPC, 636 lines in `login.rs`; orchestrates the account, auth, avatar, inventory, presence, grid and friends stores |

## 3. Proof

- Fixtures (`c7229de`): synthetic request/reply fixtures for all 14
  handlers, since no live C# traffic was recorded in this phase.
- Replay (`9593604`, `welkin-conformance/tests/replay_grid_services.rs`):
  14 replay tests, one per handler, all passing.
- Wire-traffic tests (`b4e1791`,
  `welkin-conformance/tests/interop_wire_traffic.rs`): 4 multi-step
  integration tests. They cover the `pCampBot` login, rez and teleport
  sequence (task 3.2) and the Aurora Viewer login sequence (task 3.3) as
  wire traffic against the Rust handlers. They do not drive a running C#
  simulator or the viewer binary.

## 4. Security review (task 3.4)

A second Claude review (Opus) covered authentication, inventory writes and
asset writes.

- No critical findings.
- M1: MD5 password hashes were compared with `==`, which leaks timing.
  Fixed in `840f769` with an OR-accumulate constant-time comparison in
  `welkin-services/src/lib.rs`, used by `auth.rs` and `login.rs`.
- Some admin-style endpoints are not gated in the handlers. This is left to
  deployment configuration (network placement or a reverse proxy) and is
  noted here so operators do not expose them. `set_login_level` in the
  login handler already rejects with `AdminRequired`.

## 5. Open items

- Simulation service is a stub that returns 501.
- Tasks 3.2 and 3.3 were met with replayed wire traffic. A run against a
  live C# simulator and the Aurora Viewer binary is still worth doing when
  `welkin-robust` has a real `main.rs`.
