<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Viewer compatibility

[← Back to docs](README.md) · [Integration architecture](integration-architecture.md) · [Architecture](architecture.md)

Current viewer compatibility status, updated as live testing progresses.

## Supported viewers

| Viewer | Login | In-world | Status |
|---|---|---|---|
| Aurora Viewer | Yes | Partial | First connected 2026-11-14 |
| Firestorm (OpenSim build) | Yes | Working | Tested 2026-11-14 |
| LibreMetaverse bot | Yes | 13/13 tests pass | Automated test client |

## Firestorm session (2026-11-14)

Tested with the Firestorm OpenSim build against a local Welkin grid
(ROBUST + one region). Commits 053b903 through cc9520a cover all work
described below.

### Login and connection

- XMLRPC and LLSD login (Firestorm sends XMLRPC, LibreMetaverse 3.x sends LLSD)
- Circuit establishment (UseCircuitCode, CompleteAgentMovement, RegionHandshake)
- PacketAck processing (server clears acked packets)
- Region handle formula: `((loc_x * 256) << 32) | (loc_y * 256)`
- ROBUST config separation (robust.toml for grid DB, welkin.toml for region DB)

### Movement and navigation

- WASD movement with walk, run and fly transitions
- Map and minimap via MapLayer cap (POST and GET)
- Local teleport via map
- Sit and stand on objects

### Object interaction

- Object creation (ObjectAdd with FullObjectUpdate broadcast)
- Object editing: move, rotate and scale via MultipleObjectUpdate
- ObjectSelect, ObjectDeselect
- Touch events: ObjectGrab, ObjectGrabUpdate, ObjectDeGrab
- Property editing: name, description, shape, permissions, sale info, extra params
- RezObject, DeRezObject
- ObjectLink, ObjectDelink, ObjectDuplicate

### Inventory

- Inventory panel with full folder tree (16 categories)
- Inventory caps: FetchInventoryDescendents2, FetchInventory2, FetchLib2, CreateInventoryCategory
- Baked texture upload (UploadBakedTexture cap, two-phase, stored in ROBUST)
- Inventory creation idempotency guard
- Inventory offers between users

### Communication

- Chat send and receive (ChatFromViewer routed through ChatModule)
- Instant messaging with full dialog dispatch (20+ message types)
- Group messaging
- Friendship management (offer, accept, decline, terminate)
- ViewerEffect relay to other clients
- UUIDNameRequest
- ScriptDialogReply

### Avatar and presence

- Avatar profile viewing
- Multi-user presence (new users see existing avatars)
- AgentUpdate and ViewerEffect flowing
- 53 region modules loaded

### Economy

- MoneyModule trait with SampleMoneyModule
- Economy data packets (EconomyData, MoneyBalance)
- XML-RPC endpoints: `/currency.php`, `/landtool.php`
- LSL money functions: llGiveMoney, llTransferLindenDollars, llSetPayPrice
- DTL/NSL currency proxy mode
- Gloebit REST client

### Known limitations

- LSL compilation pipeline runs (lexer, parser, analyzer, Luau codegen) but ~387 ll* functions still need region wiring
- Physics step integration not yet wired
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
| `8da4f35` .. `eb634b0` | Object interaction: editing, properties, rez, link, duplicate |
| `3b7c2d8` .. `368e61d` | Module activation: 9 to 53 region modules |
| `c6834aa` .. `afdecf9` | Viewer messaging: IM dispatch, groups, friends, effects |
| `dcc3e8b` .. `cc9520a` | Currency: MoneyModule, economy packets, Gloebit client |
