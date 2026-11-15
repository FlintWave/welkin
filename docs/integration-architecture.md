<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Integration architecture

[← Back to docs](README.md) · [Architecture](architecture.md) · [Viewer compatibility](viewer-compatibility.md)

How Welkin's crates connect at runtime. The [architecture doc](architecture.md)
lists every crate and its role; this page traces the request paths that tie
them together.

## Two binaries, one schema

Welkin ships two server binaries. `welkin-robust` runs the grid services (login,
asset storage, inventory, grid registry, presence tracking, friends, groups,
profiles, estates, agent preferences, mute lists, offline IM, map tiles and
authorization). `welkin-region` runs one or more region simulators, each
hosting a scene that viewers connect to. Both binaries read from the same
database schema (53 C# migration files, applied on first startup by
`welkin-data`).

In standalone mode the region binary embeds the grid services internally. In
grid mode the two processes communicate over HTTP using the same XML/LLSD
wire formats that C# OpenSimulator uses.

## Viewer to region

A viewer connects to the region through two channels: LLUDP (a custom UDP
protocol for real-time scene data) and HTTP capabilities (REST endpoints for
inventory, assets, event polling and uploads).

### LLUDP path

```
Viewer  ──UDP──>  welkin-net (server.rs)
                      │
                      ▼
              LLUDPClient (per viewer)
                      │
                      ▼
              dispatch.rs (message ID table)
                      │
                      ▼
              handlers.rs (typed handler functions)
                      │
                      ▼
              welkin-scene (Scene, SceneObjectGroup, ScenePresence)
```

`welkin-net` opens a UDP socket, accepts `UseCircuitCode` to establish a
circuit, then routes every subsequent packet through `LLUDPClient` (one per
viewer session). The dispatch table maps message IDs to typed handler
functions. Handlers read and mutate the `Scene` in `welkin-scene`, which
holds object groups, presences and terrain.

Outbound packets (object updates, terrain patches, avatar appearance) follow
the reverse path: scene state is encoded by `welkin-msg` (wrapping
`aurora-msg` for framing and zero-coding) and sent through the circuit's
reliable or unreliable channel.

### HTTP capabilities path

```
Viewer  ──HTTP──>  welkin-caps (axum handlers)
                       │
                       ├── EventQueueGet (25s LLSD long-poll)
                       ├── FetchInventory2, FetchLib2 ──> welkin-services (ROBUST)
                       ├── GetTexture, GetMesh ──> welkin-services (asset)
                       ├── UploadBakedTexture ──> welkin-services (asset)
                       └── MapLayer, RegionInfo, SimulatorFeatures (local)
```

After login the viewer receives a seed capability URL. All subsequent HTTP
requests go through `welkin-caps`, which serves 42 capability route entries.
Caps that need grid data (inventory, assets, display names) proxy to ROBUST's
service endpoints. Caps that need only local region state (map layer, simulator
features) answer directly.

The LLSD event queue (`EventQueueGet`) is a 25-second long-poll. The region
posts events to it (teleport, group notices, script dialogs) and the viewer
reads them in order.

## Region to ROBUST

The region talks to ROBUST over HTTP, using the same request/response formats
as C# OpenSimulator's service connectors. Each grid service has a matching
client in `welkin-services`.

```
welkin-region
    │
    ├── login ──> welkin-services/login (XML-RPC on public port 8002)
    │
    ├── grid registration ──> welkin-services/grid
    ├── presence ──> welkin-services/presence, griduser
    ├── inventory ──> welkin-services/inventory
    ├── assets ──> welkin-services/assets
    ├── friends, groups, profiles ──> respective service handlers
    └── estate, auth, authorization ──> respective service handlers
```

ROBUST exposes two ports. The public port (default 8002) serves viewer login
(XML-RPC and LLSD) and grid info. The private port (default 8003) serves all
service-to-service traffic. The split keeps inter-service endpoints off the
public network.

## Script engine integration

Scripts connect to the region through channels, not direct function calls.

```
welkin-script-vm
    │
    ScriptScheduler (round-robin tick)
    │
    ScriptInstance (one Luau VM per script)
    │
    ├── builtins (ll* functions) ──> ScriptContext (shared state)
    ├── OSSL (os* functions) ──> ScriptContext
    └── event queue <── region dispatches events (touch, timer, collision)
```

`welkin-script-compiler` compiles LSL source to Luau bytecode.
`welkin-script-vm` loads that bytecode into a sandboxed Luau VM (via mlua)
and runs it under instruction and memory limits. The `ScriptScheduler`
drives all scripts in round-robin order, one event per tick per script.

Built-in functions that need region state (llSay, llSetPos, llHTTPRequest)
go through a `ScriptContext` shared via `Arc<RwLock<..>>`. Functions that
need to send packets or trigger region actions (teleport, NPC commands,
dynamic textures) use typed channels back to the region.

## Module system

Region modules live in `welkin-modules` and follow a lifecycle trait
(`RegionModule`). The `ModuleRegistry` owns all modules and provides
interface lookup by `TypeId`. Modules communicate through `SceneEvents`,
a typed `tokio::sync::broadcast` event bus.

53 region modules are loaded, covering land, permissions, estate, chat, IM,
groups, friends, profiles, NPCs, environment, entity transfer, world map,
attachments, concierge and IRC bridge.

## Physics integration

The `welkin-physics` crate defines the `PhysicsScene` and `PhysicsActor`
traits. The `welkin-physics-rapier` crate implements them with Rapier 0.36.1.
The region's scene loop calls `physics.step()` each frame; collision events
flow back to the script engine as `collision_start`, `collision` and
`collision_end` events.

Terrain heightfield data comes from `welkin-scene` and is injected into the
physics world via `Scene::set_terrain()`.

## Data layer

`welkin-data` provides 13 typed stores over `sqlx` (MariaDB, PostgreSQL,
SQLite). Stores map to database tables matching the C# schema. The migration
runner applies 53 embedded SQL files on startup.

Region-side persistence (`welkin-data::simulation`) saves objects as XML2,
terrain as raw heightmap, and region settings as rows. The scene loop
saves dirty objects on a timer and on shutdown.

## Cross-region transfer

Entity transfer uses `welkin-modules::EntityTransferModule`. The flow:

1. Viewer requests teleport (TeleportLocationRequest packet or map click)
2. EntityTransferModule resolves the destination region handle
3. FSM validates the transition (distance check, max 4095m per hop)
4. The source region serializes avatar state and scripts
   (`RegionTransferBundle` from `welkin-script-vm`)
5. The destination region restores state and resumes event processing
6. The source region removes the presence

Hypergrid teleport adds a gatekeeper resolution step through
`HGEntityTransferModule` and verifies the agent via `UserAgentService`.

## Configuration

`welkin-config` loads TOML files with environment variable overrides
(`OPENSIM__Section__Key=value`). Grid profiles (`standalone`, `own-grid`,
custom) set service URLs. The `osim-migrate convert-config` command imports
C# `.ini` files to TOML.

## Crate dependency summary

Everything flows downward:

- Aurora crates (pinned, wrapped) sit at the bottom: `aurora-llsd`,
  `aurora-msg`, `aurora-prim`
- `welkin-types` is used by nearly every crate for shared enums and bitfields
- Core protocol crates (`welkin-llsd`, `welkin-msg`, `welkin-net`) handle
  wire encoding
- `welkin-data` and `welkin-services` form the service layer
- `welkin-scene` and `welkin-modules` form the simulation layer
- `welkin-script-compiler` and `welkin-script-vm` form the script engine
- The two binaries (`welkin-robust`, `welkin-region`) sit at the top,
  assembling the layers

See the [architecture doc](architecture.md) for the full dependency graph
and the crate table.
