# Integration Wiring

## Motivation

The Fable parity audit identified that much of Welkin's code exists but isn't reachable from the running binary. While 53 modules are registered and 40+ capability routes are wired, several critical subsystems have gaps between the code and the running server. Physics is disabled in config. The script lifecycle has missing touch points. The chat system has a masking bug. The map system returns empty data.

This proposal covers the integration wiring needed to close these gaps and make Welkin's existing code actually function as a connected system.

## Scope

### In scope

1. **Physics activation**: set PhysicsEnabled=true in welkin.toml, verify Rapier3D physics loop runs, fix terrain to load from heightmap data instead of flat 21m
2. **Script lifecycle completion**: wire the 7 remaining touch points so scripts can be uploaded, compiled, started, run, persist across restarts, and respond to control packets
   - UpdateScriptAgent cap already exists; verify it triggers compilation and script start
   - RezScript handler: when a prim is rezzed with a script in inventory, start the script
   - ScriptContext channels: llSay/llListen, timer, sensor, touch events routed to/from the VM
   - Task inventory: scripts can read/write prim inventory (llGetInventoryName, etc.)
   - Prim rez triggers script start (state_entry fires)
   - Script state persistence across region restart
   - Control packets (AGENT_CONTROL flags) forwarded to scripts that requested them
3. **Chat masking fix**: the ChatFromViewer stub in handlers.rs shadows the real implementation in main.rs. Remove the stub so the real handler processes chat.
4. **Map data fix**: MapBlockRequest handler returns empty data (blue void in minimap). Wire it to query ROBUST GridService for neighboring region info and return populated MapBlock replies.
5. **LSL compiler remaining 11 tasks**: complete llEmail, llHTTPRequest, llRequestURL, llRezObject, llSetLinkPrimitiveParams, llAttachToAvatar, llSetVehicleType, llTarget, llRotTarget, llRequestAgentData, llGetNotecardLine
6. **Firestorm session remaining fixes**: teleport test, soak test, reconnection test, graceful disconnect cleanup

### Out of scope

- Modern avatar features (PBR, BOM, Animesh, glTF) - separate proposal
- New viewer protocol features beyond what OpenSim 0.9.3.0 supports
- Performance optimization
- Hypergrid federation testing

## Library search results

Already in workspace:
- `rapier3d` (Apache-2.0) - physics engine, already integrated in welkin-physics-rapier
- `mlua` (MIT) - Lua VM for LSL script execution, already in welkin-script-vm
- `lettre` (MIT) - SMTP client for llEmail, already in Cargo.toml
- `reqwest` (MIT/Apache-2.0) - HTTP client for llHTTPRequest, already in workspace

No new crate dependencies needed for this change.

## Phases

### Phase 1: Fix the broken paths (3 days, critical path)
Fix chat masking bug, map data, physics activation. These are the most visible issues when a viewer connects.

### Phase 2: Script lifecycle (5 days, critical path, depends on Phase 1)
Wire the 7 script touch points. Scripts are the most complex subsystem; each touch point must be tested independently then as a chain.

### Phase 3: LSL completion (4 days, can parallel with Phase 2 after task inventory is done)
Complete the 11 remaining LSL function implementations. Many depend on script lifecycle being wired (llRezObject needs rez-triggers-script, llSay needs ScriptContext channels).

### Phase 4: Validation (3 days, depends on Phases 1-3)
Firestorm soak test, teleport test, reconnection, graceful disconnect. Full regression suite.

## Acceptance criteria

- PhysicsEnabled=true, objects fall, terrain has real elevation data
- A script uploaded via viewer compiles, starts, responds to touch, persists across restart
- Chat messages between avatars work in Firestorm and Aurora
- Minimap shows neighboring regions
- All 11 remaining LSL functions work
- 10-minute soak test with no crashes or disconnects
- All 1,093+ existing tests still pass

## Estimates

~15 working days total, ~13 day critical path (Phases 1-2 are sequential, Phase 3 overlaps Phase 2).
