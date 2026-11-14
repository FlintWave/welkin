# region-simulator-core summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Decisions](decisions.md#n1) · Previous: [Grid portal](grid-portal-summary.md) · Next: [Prim meshing](prim-meshing-summary.md)

Status: accepted (Phase 3). All 7 task groups in
`openspec/changes/region-simulator-core/tasks.md` are checked. Commits
`759e3c6` through `129d401`. Second review verdict: CONDITIONAL PASS.

## What was built

The region simulator: scene graph, LLUDP client stack, HTTP capabilities,
terrain, persistence and the `opensim-region` binary. It hosts one or more
regions per process, accepts viewer connections, serves terrain and objects,
and saves state to a C#-compatible region database.

Not included (later phases): land/parcels, permissions, chat, IM, estate
management, archiver, scripting hooks, other CoreModules.

## Architecture

```text
viewer --UDP--> opensim-net (server, client, dispatch, handlers)
viewer --HTTP-> opensim-caps (seed cap, event queue, cap handlers)
                    |
                    v
             opensim-scene (Scene -> groups -> parts, presences, terrain)
                    |                    |
                    v                    v
         opensim-physics (trait)   opensim-data (SimulationDataStore, sqlx)
                    ^
         opensim-region (binary: config, regions, listeners, shutdown)
```

- One `Scene` per region. `RegionManager` runs several in one process.
- LLUDP dispatch is a table keyed on message id, calling typed handler
  functions. Wire codecs come from `opensim-msg`.
- Persistence stores objects as XML2 and uses the existing `regionstore`
  tables, so a C#-created database loads without migration.

## Key modules

| Crate | Module | Role |
|---|---|---|
| `opensim-scene` | `scene.rs` | Region heartbeat, entity maps, update dispatch, spatial queries (position, owner, name), dirty tracking and periodic backup |
| | `group.rs` | `SceneObjectGroup`: linked prim set, root part, link/unlink, bounding box |
| | `part.rs` | `SceneObjectPart`: shape, texture entry, extra params, properties, flags, inventory stub |
| | `presence.rs` | `ScenePresence`: position, rotation, velocity, controls, animation, sit, appearance |
| | `xml2.rs` | XML2 object serializer and parser, interchange format shared with C# |
| | `terrain.rs` | Heightfield channel, variable region size, patch dirty tracking |
| | `terrain_io.rs` | LLRAW, RAW32 and PNG import/export |
| | `terrain_brush.rs` | Raise, lower, flatten, smooth, noise, revert |
| | `terrain_compress.rs` | LayerData patch compression for the viewer |
| | `region_manager.rs`, `shutdown.rs` | Multi-region host, graceful persist/deregister/close |
| `opensim-net` | `server.rs` | UDP listener, accept, per-client routing |
| | `client.rs` | Per-viewer state: acks, resend queue, sequence numbers, lifecycle |
| | `dispatch.rs` | Handler table keyed on message id |
| | `handlers.rs` | Viewer handlers (UseCircuitCode, CompleteAgentMovement, AgentUpdate, object and avatar messages) |
| | `circuit.rs`, `throttle.rs` | Phase 1 circuit and throttle, reused |
| `opensim-caps` | `server.rs` | Per-session seed cap and cap URL dispatch via axum |
| | `event_queue.rs` | LLSD long-poll event queue, enqueue and poll/ack cycle |
| | `handlers.rs` | Texture/mesh fetch, region info, agent preferences, upload caps |
| `opensim-data` | `simulation.rs` | `SimulationDataStore` trait and sqlx implementation (objects, terrain, region settings, spawn points) |
| `opensim-region` | `main.rs` | Config, database init, scenes, LLUDP and caps startup, grid registration, shutdown |

## Tests

Counted from `#[test]` and `#[tokio::test]` attributes:

| Crate | Unit and inline | Integration files |
|---|---|---|
| `opensim-scene` | 137 | `scene_proof.rs`, `xml2_proof.rs` |
| `opensim-net` | 30 | `lludp_roundtrip.rs` (conformance) |
| `opensim-caps` | 16 | none |
| `opensim-data` | 18 | `simulation_roundtrip.rs`, `csharp_compat.rs` |
| `opensim-region` | 11 | `acceptance.rs` |

Totals shown include integration test files under each crate's `tests/`
directory where present. Proof coverage: scene graph operations, XML2
round-trip against C#-serialized objects, loading a C# region database and
checking object positions, and an end-to-end acceptance run
(connect, terrain, rezzed object, avatar movement).

## Security review

Independent second review of persistence and client stack protocol handling.
Verdict: CONDITIONAL PASS. Three HIGH findings in the network stack must be
fixed before production use:

1. Bounded client map: the per-server client table has no size cap, so a
   flood of circuit codes can exhaust memory.
2. Bounded mpsc: channels between the receive loop and clients are unbounded,
   so a slow client can grow queues without limit.
3. Stale session reaping: sessions that stop responding are not removed on a
   timer.

Until these land the region binary is not for public exposure; restrict it by
network. Persistence code had no HIGH findings.

## Known gaps

- The three HIGH findings above are open.
- Cap handlers beyond the seed cap and event queue are thin; texture and mesh
  fetch and upload caps need real asset-service wiring.
- Avatar and object handlers cover the core viewer flow only; permissions,
  land and chat are absent by scope.
- Capacity assessor benchmark/model/live modules are still stubs; they can
  now be built on this simulator (see decision C1).
- Test counts for `opensim-net` and `opensim-caps` are modest relative to the
  protocol surface; fuzzing of the new handlers is not yet set up.
