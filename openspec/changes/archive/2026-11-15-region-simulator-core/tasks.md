## 1. Scene graph

- [x] 1.1 Scene struct: heartbeat loop, entity maps, update dispatch, frame timing
- [x] 1.2 SceneObjectGroup: linked prim set, root part, link/unlink, bounding box
- [x] 1.3 SceneObjectPart: shape, texture entry, extra params, properties, flags, inventory stub
- [x] 1.4 ScenePresence: position, rotation, velocity, animation state, sit, controls, appearance
- [x] 1.5 Scene entity add/remove/update and spatial queries (by position, by owner, by name)
- [x] 1.6 XML2 scene serialization: round-trip tests against C# output

## 2. Client stack

- [x] 2.1 LLUDPServer: listener, accept, per-client dispatch using aurora-msg decode
- [x] 2.2 LLUDPClient: ack tracking, reliable resend, sequence numbers, connection lifecycle
- [x] 2.3 LLClientView packet dispatch: typed handler table keyed on message id
- [x] 2.4 Core viewer handlers: UseCircuitCode, CompleteAgentMovement, AgentUpdate, ObjectUpdate, ImprovedTerseObjectUpdate
- [x] 2.5 Object handlers: ObjectAdd, ObjectDelete, ObjectSelect, ObjectDeselect, ObjectGrab, ObjectProperties, MultipleObjectUpdate
- [x] 2.6 Avatar handlers: AvatarAnimation, AvatarAppearance, AgentSetAppearance, ViewerEffect, ChatFromViewer

## 3. Capabilities

- [x] 3.1 Capability module: per-session seed cap, cap registration, cap URL dispatch via axum
- [x] 3.2 EventQueue: LLSD long-poll handler, event enqueue, viewer poll/ack cycle
- [x] 3.3 Texture and mesh fetch capabilities (GetTexture, GetMesh, GetMesh2)
- [x] 3.4 Region info caps: SimulatorFeatures, RegionInfo, AgentPreferences, MapLayer
- [x] 3.5 Upload caps: NewFileAgentInventory, UpdateNotecardAgentInventory, UpdateScriptAgent

## 4. Terrain

- [x] 4.1 Terrain channel: heightfield storage, variable-size region support, patch dirty tracking
- [x] 4.2 Terrain file loaders: LLRAW, RAW32, PNG import/export
- [x] 4.3 Terrain modification: raise, lower, flatten, smooth, noise, revert brushes
- [x] 4.4 Viewer terrain protocol: LayerData packets, patch compression, terrain upload

## 5. Persistence

- [x] 5.1 SimulationDataStore trait: store/load objects, terrain, region settings, spawn points
- [x] 5.2 sqlx implementation over existing regionstore tables (MySQL, PostgreSQL, SQLite)
- [x] 5.3 Round-trip test: load a C#-created region database, verify objects and terrain match
- [x] 5.4 Periodic backup: dirty-object tracking, timed persistence in the scene loop

## 6. Region binary

- [x] 6.1 Config loading, database init, Scene creation, grid service registration
- [x] 6.2 LLUDP listener startup, capability HTTP server alongside robust
- [x] 6.3 Graceful shutdown: persist, deregister, close connections
- [x] 6.4 Multi-region support: multiple Scenes in one process

## 7. Proof

- [x] 7.1 Unit tests for scene graph operations (add/remove/link/unlink, spatial queries)
- [x] 7.2 XML2 round-trip tests against C# serialized objects
- [x] 7.3 Aurora Viewer connects, sees terrain, sees a rezzed object, moves the avatar
- [x] 7.4 Load a C# region database and verify objects appear in the correct positions
- [x] 7.5 Second review of persistence code and client stack protocol handling (CONDITIONAL PASS: 3 HIGH findings in net stack — bounded client map, bounded mpsc, stale session reaping needed before production)
