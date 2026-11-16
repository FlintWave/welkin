<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Crate structure

[← Back to docs](index.md) · [Architecture](architecture.md) · [Module development guide](module-development.md)

This page is a map for someone who is about to change the code. The
[architecture page](architecture.md) has the full crate table and the dependency
graph. Here you find where things live.

Welkin is a Cargo workspace of 25 crates, edition 2024, Rust 1.88 or later.
`welkin-companion` (a Tauri desktop app) lives beside them and builds
separately. Two crates produce programs: `welkin-region` and `welkin-robust`.

## Layers

| Layer | Crates | Role |
|---|---|---|
| Core | `welkin-types`, `welkin-llsd`, `welkin-msg`, `welkin-config`, `welkin-runtime` | Types, LLSD and LLUDP codecs, TOML config, logging |
| Network | `welkin-net`, `welkin-caps` | UDP server and circuits, HTTP capabilities, event queues |
| Scene | `welkin-scene`, `welkin-prim`, `welkin-modules` | Scene graph, terrain, prim meshing, region modules |
| Physics | `welkin-physics`, `welkin-physics-rapier` | Physics traits and the Rapier 3D backend |
| Scripting | `welkin-script-compiler`, `welkin-script-vm`, `welkin-script-api` | LSL to Luau, the VM and builtins, constant tables |
| Services | `welkin-services`, `welkin-data` | Grid service handlers, remote connectors, SQL stores |
| Archives | `welkin-archive`, `welkin-migration` | OAR and IAR, C# migration tools |
| Testing | `welkin-conformance`, `welkin-capacity`, `welkin-realmwalker` | Replay, benchmarks, bot SDK |
| Binaries | `welkin-region`, `welkin-robust` | The two programs |
| Scaffold | `welkin-portal` | Web portal; no other crate depends on it and no binary mounts it |

## Where to look

In `welkin-region/src/`, the region simulator:

| Area | Files |
|---|---|
| Packet loop and startup | `main.rs` |
| Console and admin API | `console.rs`, `console_io.rs` |
| Saving and loading | `persist.rs`, `persist_load.rs`, `script_persist.rs`, `region_identity.rs` |
| Estate and permissions | `estate_config.rs` |
| Objects and inventory | `object_add.rs`, `inventory_rez.rs`, `attachments_io.rs`, `scene_sync.rs` |
| Movement and physics | `avatar_move.rs`, `physics_cmd.rs` |
| Terrain | `terraform.rs` |
| Sound | `sound_io.rs` |
| Groups | `groups_io.rs`, `groups_queries.rs`, `groups_admin.rs`, `groups_invites.rs`, `groups_chat.rs`, `groups_queue.rs`, `groups_tasks.rs` |

Elsewhere:

| Area | Where |
|---|---|
| Scene graph, objects, attachments | `welkin-scene/src/scene.rs`, `group.rs`, `part.rs` |
| Terrain heights, brushes, LayerData encoder | `welkin-scene/src/terrain*.rs` |
| Object XML (XML2) | `welkin-scene/src/xml2.rs` |
| The `RegionModule` trait and registry | `welkin-modules/src/module.rs`, `registry.rs` |
| Grid services (login, accounts, groups and others) | `welkin-services/src/` |
| Clients for those services, used by the region | `welkin-services/src/remote/` |
| SQL stores and migrations | `welkin-data/src/` |
| Builtin LSL functions | `welkin-script-vm/src/builtins/` |

## Rules the code follows

- Handlers do real work. CI fails on `todo!()`, `unimplemented!()`, `STUB` and
  `PLACEHOLDER` under `crates/`.
- A viewer packet acts for the authenticated avatar of the connection, never for
  an agent ID the packet claims.
- Code that waits on another service does not hold the session lock.
- Features are wired into a running binary. A tested module that nothing calls
  does not count as done.
