# Changelog

## v0.1.0-alpha (2026-11-16)

First tagged release. Welkin can accept a viewer login, rez a region, and run
scripts, but most of the OpenSim surface area is still stubbed or missing.
This release exists so that people can build, connect, and see what works
before the project is anywhere near production-ready.

### What works

- **Grid login**: ROBUST grid services (asset, inventory, authentication,
  user-account, grid-info, avatar, presence, friends, map) accept viewer
  logins. Aurora Viewer and Firestorm both connect successfully.
- **Region simulator**: a single region loads, sends terrain and object data to
  viewers, and handles avatar movement (walk, run, fly, jump, fall, crouch).
- **Terrain**: real DCT-based LayerData encoding, terraform brushes (raise,
  lower, flatten, smooth, noise, revert), terrain persistence across restarts.
- **Physics**: Rapier 3D backend with terrain colliders, object colliders,
  script-driven forces and impulses, and gravity.
- **Objects**: rez from inventory, take/copy/delete/return to inventory,
  object persistence across restarts, link sets, and basic permissions.
- **Attachments**: wear from inventory, attach in-world objects, detach to
  inventory or ground, login restore of worn attachments.
- **Scripts**: Lua-based LSL compiler and VM. Scripts compile and run, persist
  across restarts, and expose a subset of LSL builtins (see below for gaps).
- **Groups**: create, join, leave, roles, titles, invitations, group chat,
  stored notices, membership-gated queries, and per-avatar task queues.
- **OAR loading**: `load oar <file>` console command imports assets, objects,
  and .r32 terrain from OpenSim OAR archives.
- **Default assets**: 5,683 default asset sets seeded at ROBUST startup.
- **Parcels**: ParcelProperties sent via both UDP and event queue (modern
  viewers need the event queue path for fly permissions).
- **Animations**: walk, run, stand, sit, jump, fall, crouch, crouchwalk, and
  fly animations with the correct UUIDs from avataranimations.xml.
- **Instant messaging**: P2P IM relay between agents; session IDs computed
  correctly (XOR of both agent IDs, not leaked sender session).
- **Bot testing**: multi-bot LibreMetaverse harness (login, chat, IM, friend
  requests, group invites, teleport, wandering).
- **CI**: GitHub Actions running `cargo clippy -D warnings` and `cargo test`.
- **Tests**: 2,688 passing (0 failures, 2 ignored).

### What does not work yet

This is an alpha. Large sections of the OpenSim protocol surface are either
stubbed (the packet handler exists but does nothing useful) or missing entirely.

Measured stub counts as of this tag:

- 233 lines across the codebase mention "stub" (grep count; includes test
  helpers and comments, so the real count is lower, but nobody has sorted them)
- 138 of those are in welkin-modules (the largest gap)
- 53 in the script VM (LSL builtins that log and return a default)
- 28 in welkin-services
- 24 TODO comments (10 in welkin-capacity, 8 in welkin-portal)
- 0 `todo!()` or `unimplemented!()` macros

Notable gaps:

- **LSL builtins**: many `ll*` functions log a stub warning and return a
  default value. The compiler and VM work, but coverage of the ~400 LSL
  functions is partial.
- **Region crossing**: avatars cannot walk or fly between regions.
- **Teleport**: inter-region teleport is not wired.
- **Map tiles**: the region serves a flat tile; object overlay and grid upload
  are not implemented.
- **Search**: people/places/events/land search returns empty results.
- **Media on a prim (MOAP)**: not implemented.
- **Animation overrides**: not implemented.
- **Extended Environment Protocol (EEP)**: not implemented.
- **Estate management**: estate settings persist, but most estate management
  commands from the viewer are stubs.
- **Economy**: ObjectBuy delivers the object and sends a refusal alert, but
  there is no currency backend.
- **ClientEgress**: the trait exists and modules use it, but the region binary
  installs NullEgress, so module-initiated packets (alerts, kicks, dialogs,
  appearance updates) are silently dropped at runtime.
- **Postgres/MySQL**: SQLite is the only tested backend. Placeholder syntax is
  ported but the column types have known issues (#79).
- **Web UI**: none. Configuration is INI files and a console REPL.

### Known limits

- Single-region only. No grid of multiple regions.
- SQLite only (Postgres and MySQL schemas exist but are untested).
- No HTTPS or TLS on any service endpoint.
- The REPL `create-user` help text is wrong (#82).
- Groups do not enforce Unicode normalization, so visually identical names can
  coexist (#81).

### Build

Requires Rust 1.88+ (edition 2024, let-chains).

```
cargo build --release
```

See docs/quick-start.md for setup and first login.

### Stats

- 527 commits over 3 days (project started 2026-11-13)
- 26 crates, ~198,000 lines of Rust
- 2,688 tests passing
- LGPL-3.0-or-later
