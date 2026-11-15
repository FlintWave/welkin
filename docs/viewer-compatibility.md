<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Viewer compatibility

[← Back to docs](README.md) · [Integration architecture](integration-architecture.md) · [Architecture](architecture.md)

Current viewer compatibility status, updated as live testing progresses.

## Supported viewers

| Viewer | Login | In-world | Status |
|---|---|---|---|
| Aurora Viewer | Yes | Partial | First connected 2026-11-14 |
| Firestorm (OpenSim build) | Yes | Core features working | Tested 2026-11-14 |
| LibreMetaverse bot | Yes | 13/13 tests pass | Automated test client |

## Firestorm session (2026-11-14)

Tested with the Firestorm OpenSim build against a local Welkin grid
(ROBUST + one region). Commits 053b903 through 079f043.

### Working

- XMLRPC and LLSD login (Firestorm sends XMLRPC, LibreMetaverse 3.x sends LLSD)
- Circuit establishment (UseCircuitCode, CompleteAgentMovement, RegionHandshake)
- WASD movement with walk, run and fly transitions
- Chat send and receive (ChatFromViewer routed through ChatModule)
- Inventory panel with full folder tree (16 categories)
- Inventory caps: FetchInventoryDescendents2, FetchInventory2, FetchLib2, CreateInventoryCategory
- Baked texture upload (UploadBakedTexture cap, two-phase, stored in ROBUST)
- Map and minimap via MapLayer cap (POST and GET)
- Local teleport via map
- Object creation (ObjectAdd with FullObjectUpdate broadcast)
- Sit and stand on objects
- Avatar profile viewing
- Multi-user presence (new users see existing avatars)
- AgentUpdate and ViewerEffect flowing
- PacketAck processing (server clears acked packets)
- ViewerAsset, GetDisplayNames, HomeLocation caps
- Region handle formula: `((loc_x * 256) << 32) | (loc_y * 256)`
- ROBUST config separation (robust.toml for grid DB, welkin.toml for region DB)
- Inventory creation idempotency guard
- 22 HTTP capability handlers
- 18 UDP packet handlers

### Known limitations

- Object editing (MultipleObjectUpdate and related packets)
- Instant messaging
- Inventory wear and rez
- ~30 region modules written but not yet loaded
- Script engine: Luau VM runs but no LSL compiler yet
- Currency and economy: interface defined, not implemented
- Cross-region teleport

### Known viewer messages

These are Firestorm-internal warnings that appear with an empty inventory and
do not indicate server bugs:

- "Could not put on outfit" (no clothing items in inventory)
- "Could not load settings" (no viewer settings in inventory)

## Aurora Viewer session (2026-11-14)

First viewer connection to Welkin. Aurora uses a non-standard LLUDP flag
layout: ZEROCODED=0x80, ACK=0x10 (swapped vs the Linden Lab specification).
The server handles both layouts via the `aurora-msg` crate.

### Working

- Login with full session data
- Circuit and region handshake
- 256 terrain patches rendered (flat terrain, height 21.0)
- Avatar visible with wearables and name display
- EventQueue (LLSD long-poll, 25s timeout)
- Minimap (CoarseLocationUpdate at 3s interval)
- Ping keepalive
- Camera rotation
- Capabilities HTTP
- 9 region modules loaded
- Zero crashes or disconnects

### Not yet working

- Avatar animations (T-pose, needs AgentAnimation packet)
- Movement (needs position update echo)
- Textures (asset service schema bug at time of test)

## Capabilities

22 capability handlers implemented:

| Cap | Status |
|---|---|
| AgentPreferences | Working |
| CreateInventoryCategory | Working |
| EventQueueGet | Working (25s long-poll) |
| FetchInventory2 | Wired to ROBUST xinventory |
| FetchInventoryDescendents2 | Wired to ROBUST xinventory |
| FetchLib2 | Working |
| GetDisplayNames | Working |
| GetMesh | Proxied to ROBUST asset service |
| GetMesh2 | Proxied to ROBUST asset service |
| GetTexture | Proxied to ROBUST asset service |
| HomeLocation | Working |
| MapLayer | Working (POST and GET) |
| NewFileAgentInventory | Working |
| RegionInfo | Working |
| SimulatorFeatures | Working |
| UpdateAgentInformation | Working |
| UpdateAgentLanguage | Working |
| UpdateNotecardAgentInventory | Working |
| UpdateScriptAgent | Working |
| UploadBakedTexture | Two-phase, stores in ROBUST |
| ViewerAsset | Working |
| ViewerStats | Working |

## Protocol notes

- Firestorm and Aurora use different login codecs: Firestorm sends XMLRPC,
  LibreMetaverse 3.x sends LLSD. Both are supported.
- Aurora swaps the ZEROCODED and ACK flag bits in the LLUDP header. Welkin
  detects and handles both layouts.
- Terrain uses a flat-terrain shortcut (range=0, dc_offset=21.0). Full DCT
  compression has a field-order fix pending.

## Commits

| Hash | Description |
|---|---|
| `053b903` | Firestorm login fixes (real_id, inventory root, buddy list) |
| `9d82a90` | Remove chat stub, fix MapBlockRequest, fix ChatFromViewer routing |
| `6f04835` | Remove 21 log-only stub handlers (-297 lines) |
| `f4ec89f` | AgentSetAppearance handler (orange cloud fix) |
| `3d927ae` | Movement, teleport and object creation support |
| `079f043` | Firestorm session completion (sit/stand, profiles, multi-user) |
