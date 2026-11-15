<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Viewer compatibility

[← Back to docs](README.md) · [Integration architecture](integration-architecture.md) · [Architecture](architecture.md)

Current viewer compatibility status, updated as live testing progresses.

## Supported viewers

| Viewer | Login | In-world | Status |
|---|---|---|---|
| Aurora Viewer | Yes | Partial | First connected 2026-11-14 |
| Firestorm (OpenSim build) | Yes | Partial | Testing in progress |
| LibreMetaverse bot | Yes | 13/13 tests pass | Automated test client |

## Firestorm session (2026-11-14)

Tested with the Firestorm OpenSim build against a local Welkin grid
(ROBUST + one region).

### Working

- XMLRPC and LLSD login (LibreMetaverse 3.x sends LLSD, Firestorm sends XMLRPC)
- Circuit establishment (UseCircuitCode, CompleteAgentMovement, RegionHandshake)
- Inventory panel with correct folder tree (16 categories)
- Inventory caps: FetchInventoryDescendents2, FetchInventory2, FetchLib2, CreateInventoryCategory
- Chat send and receive (ChatFromViewer routed through ChatModule)
- AgentUpdate and ViewerEffect flowing
- Baked texture upload (UploadBakedTexture cap, two-phase, stored in ROBUST)
- PacketAck processing (server clears acked packets)
- ViewerAsset cap
- GetDisplayNames cap
- HomeLocation cap
- map-server-url in login response
- Region handle formula: `((loc_x * 256) << 32) | (loc_y * 256)`
- ROBUST config separation (robust.toml for grid DB, welkin.toml for region DB)
- Inventory creation idempotency guard

### In progress

- Chat echo fixed (stub handler was masking real implementation, removed in commit 9d82a90)
- Map display fixed (MapBlockRequest now returns region data, commit 9d82a90)
- Avatar appearance: AgentSetAppearance handler added (commit f4ec89f), orange cloud expected to clear with populated inventory
- 21 log-only stub handlers removed (commit 6f04835, -297 lines)
- Awaiting re-test of chat, map, and avatar appearance in Firestorm

### Not yet working

- Local teleport (same-region)
- Cross-region teleport
- Object editing (rez, modify, delete via viewer)
- ViewerEffect relay to other clients
- Movement direction and speed tuning

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

19 capability handlers implemented:

| Cap | Status |
|---|---|
| Seed | Working |
| EventQueueGet | Working (25s long-poll) |
| GetTexture | Proxied to ROBUST asset service |
| GetMesh | Proxied to ROBUST asset service |
| GetMesh2 | Proxied to ROBUST asset service |
| ViewerAsset | Working |
| FetchInventoryDescendents2 | Wired to ROBUST xinventory |
| FetchInventory2 | Wired to ROBUST xinventory |
| FetchLib2 | Working |
| CreateInventoryCategory | Working |
| UploadBakedTexture | Two-phase, stores in ROBUST |
| GetDisplayNames | Working |
| HomeLocation | Working |
| Upload caps (misc) | Return valid LLSD responses |

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
