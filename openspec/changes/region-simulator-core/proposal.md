## Why

The region simulator is the process that hosts a piece of the virtual world.
It runs the scene loop, holds the scene graph (objects, avatars, terrain),
accepts viewer connections over LLUDP and HTTP capabilities, ticks physics,
and persists state to the database. Without it, the grid services are an
API with no world behind it.

The C# implementation spans 96,127 lines across Region/Framework (65,204)
and Region/ClientStack (30,923). The Rust workspace already has crate stubs
(`welkin-scene`, `welkin-region`, `welkin-caps`) and working foundations
from Phase 1 (`welkin-net` circuit/throttle, `welkin-msg` message codec,
`welkin-physics` trait and Rapier backend).

## What Changes

### Scene graph (`welkin-scene`)

- `Scene`: the region heartbeat, entity container, update dispatch, backup
  timer, module host. One `Scene` per region; multiple regions per process.
- `SceneObjectGroup`: a linked set of prims — the "object" in viewer terms.
  Holds the root part, link numbers, physics actor, script attachment points.
- `SceneObjectPart`: a single prim with VolumeParams, TextureEntry,
  ExtraParams, properties (name, description, owner, permissions, flags),
  inventory and script state pointers.
- `ScenePresence`: an avatar in the scene — position, rotation, velocity,
  controls, animation state, attachments, appearance, sit target.
- Scene serialization: XML2 object format (the internal persistence and
  OAR/IAR interchange format). Must round-trip with C# OpenSim.

### Client stack (`welkin-net`, `welkin-caps`)

- `LLUDPServer`: accept viewer UDP connections, dispatch incoming packets
  to their `LLUDPClient`. Uses `aurora-msg` for encode/decode.
- `LLUDPClient`: per-viewer connection state — ack tracking, reliable
  resend queue, throttle categories, sequence numbers.
- `LLClientView`: packet handler dispatch — the viewer API surface. In C#
  this is 13,480 lines; in Rust it becomes typed handler functions called
  from a dispatch table keyed on message id.
- Capabilities module: per-session HTTP endpoints seeded on login. Includes
  EventQueue (long-poll), texture/mesh fetch, upload, region info, agent
  preferences and the ~25 capabilities Aurora Viewer uses.
- Event queue: LLSD long-poll delivering events the viewer expects on the
  HTTP path rather than UDP.

### Terrain

- Heightfield storage: per-region terrain channel, 256×256 base with
  variable-size region support, patch-based dirty tracking.
- Terrain file loaders: LLRAW (the native format), PNG, RAW32.
- Terrain modification: raise, lower, flatten, smooth, noise, revert.
  These are needed for viewer terrain editing and OAR import.

### Persistence

- `ISimulationDataStore` trait in `welkin-data`: store and load objects,
  terrain, region settings, parcels, region environment, spawn points.
- `sqlx` implementation over the existing `regionstore` tables so that a
  database written by C# OpenSim can be loaded by the Rust simulator
  without migration.

### Region binary (`welkin-region`)

- Process entry point: load config, open databases, create Scene(s),
  start the LLUDP listener(s), register with the grid service, enter
  the heartbeat loop.
- Graceful shutdown: persist dirty objects, deregister from grid, close
  viewer connections.

## Impact

- Modifies: `welkin-scene`, `welkin-region`, `welkin-net`, `welkin-caps`,
  `welkin-data` (new simulation-data store trait and sqlx impl).
- New files in existing crates; no new crates.
- New dependencies: none beyond what the workspace already has.
- Risk: HIGH for persistence (data loss on save/load bugs) and for the
  client stack (protocol faults drop viewers). Both areas get an
  independent second review. The scene serializer is tested against
  XML2 round-trip with C# output.
- This change does NOT include: land/parcels module, permissions module,
  chat, IM, estate management, archiver, scripting hooks, or any
  CoreModules. Those come in later phases (region-modules, archives).
  Terrain modification is included because the heightfield is part of
  the scene graph and needed for OAR loading.
