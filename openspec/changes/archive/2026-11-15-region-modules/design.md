# region-modules: Land, permissions, estate, inventory, attachments, chat, IM, map

## Summary

This change ports the non-terrain, non-archiver, non-scripting region modules
from `OpenSim/Region/CoreModules/` to Rust. These are the modules that make a
region feel inhabited: land parcels, permission checks, estate management,
inventory transfers, attachments, chat, instant messages, world map,
environment settings, entity transfer and the framework glue that wires
modules into the scene. Together they cover roughly 50,000 non-test C# lines
across 150+ files.

The work sits on top of `region-simulator-core` (scene graph, client stack,
capabilities, terrain, persistence) and `grid-services` (robust services).
It consumes the script hooks from `script-engine` but does not depend on the
script VM itself.

## Scope

### In scope (ported in this change)

| Area | C# lines (non-test) | Notes |
|---|---|---|
| World/Land | 5,825 | Parcels, land overlay, prim counts, dwell |
| World/Permissions | 2,644 | Object, script, terrain, parcel permission checks |
| World/Estate | 3,263 | Estate settings, telehub, region restart, sun |
| World/WorldMap | 2,221 | Map tile generation, map blocks, agent locations |
| World/LegacyMap | 1,384 | 2D map tile renderer |
| World/Warp3DMap | 1,646 | 3D map tile renderer (replaced by tiny-skia) |
| World/LightShare | 1,123 | Windlight environment settings |
| World/Wind | 867 | Wind model (SimpleRandomWind, ConfigurableWind) |
| World/Region | 773 | Region ready, restart module |
| World/Media | 636 | Media-on-a-prim (MOAP) |
| World/Sound | 469 | Triggered and looped sounds |
| World/Objects | 1,254 | Object commands, buy, access control |
| World/Serialiser | 478 | Scene serialization helper |
| World/Access | 159 | Region access control |
| World/Vegetation | 135 | Tree planting |
| Avatar/Inventory | 2,966 | Inventory transfers (give, accept, decline) |
| Avatar/Attachments | 1,516 | Wear, detach, save, cross-region |
| Avatar/Chat | 481 | Local chat, shout, whisper |
| Avatar/InstantMessage | 1,947 | IM routing, offline IM, mute, presence |
| Avatar/Friends | 2,683 | Friend list, online notifications |
| Avatar/AvatarFactory | 1,335 | Appearance baking, wearable validation |
| Avatar/Lure | 515 | Teleport lure (offer, accept, decline) |
| Avatar/Gods | 351 | God mode, kick, freeze |
| Avatar/Dialog | 251 | Dialog and notification delivery |
| Avatar/BakedTextures | 254 | Baked texture cache |
| Avatar/Commands | 188 | Console commands for avatar management |
| Avatar/Combat | 159 | Damage, health, death |
| Avatar/Gestures | 115 | Gesture activation/deactivation |
| Framework/EntityTransfer | 4,169 | Teleport, region crossing, child agents |
| Framework/InventoryAccess | 2,332 | Rez from inventory, derez, take, coalesce |
| Framework/Caps | 587 | Caps infrastructure module |
| Framework/DynamicAttributes | 263 | DAMap store |
| Framework/InterfaceCommander | 404 | Console command dispatch |
| Framework/Library | 534 | Library inventory |
| Framework/Monitoring | 1,444 | Region stats, frame time monitoring |
| Framework/Search | 271 | Directory search (places, land, people) |
| Framework/ServiceThrottle | 153 | Async service call throttle |
| Framework/UserManagement | 1,539 | User name resolution, HG user mapping |
| ServiceConnectorsOut (new) | ~2,200 | Land, Neighbour, MuteList, AgentPreferences, MapImage |
| ServiceConnectorsIn (new) | ~500 | Land, MapImage, Neighbour |
| Agent/AssetTransaction | 1,163 | Upload transactions (textures, sounds, etc.) |
| Agent/TextureSender | 431 | J2K texture priority queue |
| Agent/Xfer | 492 | Xfer file transfer protocol |
| Agent/IPBan | 220 | IP-based connection blocking |
| Asset (FlotsamCache) | 1,723 | Two-level asset cache (memory + disk) |

**Estimated total: ~49,000 non-test C# lines.**

### Out of scope (deferred to other changes)

| Area | Deferred to | Reason |
|---|---|---|
| World/Archiver | `archives` | OAR save/restore is its own change |
| World/Terrain | already done | In `region-simulator-core` tasks 4.1-4.4 |
| Avatar/Inventory/Archiver | `archives` | IAR save/restore |
| Avatar/UserProfiles (1,894) | `social-modules` | Groups, profiles, NPCs are phase 6 |
| Avatar/Groups (267) | `social-modules` | Groups module |
| Avatar/Profile (179) | `social-modules` | Legacy profile module |
| Scripting/* (7,026) | `script-engine` / `ossl-api` | Already covered by those changes |
| ServiceConnectorsOut already done | `grid-services` | Asset, Auth, Grid, GridUser, Inventory, Presence, Simulation, UserAccounts, Avatar, Authorization |
| ServiceConnectorsIn/Hypergrid | `hypergrid` | HG-specific connectors |
| ServiceConnectorsIn/Login | `grid-services` | Login already done |
| Framework/EntityTransfer HG parts | `hypergrid` | HGEntityTransferModule deferred |
| Framework/InventoryAccess HG parts | `hypergrid` | HGInventoryAccessModule, HGAssetMapper deferred |
| Framework/UserManagement HG parts | `hypergrid` | HGUserManagementModule deferred |

### Already done in other changes that we build on

- **region-simulator-core**: Scene graph, SceneObjectGroup/Part, ScenePresence,
  LLUDP server/client, capabilities (seed, event queue, texture/mesh fetch,
  uploads), terrain (channel, loaders, brushes, protocol), persistence (sqlx
  over regionstore), region binary.
- **grid-services**: All Robust services (assets, inventory, grid, presence,
  authentication, authorization, friends, avatar, mute, user profiles, map
  images, estate, simulation, login). ServiceConnectorsOut for these.
- **script-engine**: SLua VM, scheduler, LSL/Luau API, state save/restore,
  detected params, os* extensions.

## Architecture

### Crate structure

A single new crate: `welkin-region-modules`.

```
crates/welkin-region-modules/
  src/
    lib.rs              -- ModuleRegistry, RegionModule trait
    land/               -- LandManagementModule, LandObject, PrimCountModule
    permissions/        -- DefaultPermissionsModule
    estate/             -- EstateManagementModule, EstateManagementCommands
    map/                -- WorldMapModule, MapImageModule (tiny-skia renderer)
    environment/        -- LightShareModule, WindModule
    media/              -- MoapModule
    sound/              -- SoundModule
    objects/            -- ObjectCommands, BuySellModule, AccessModule
    serialiser/         -- SceneSerialiser (adapts XML2 from welkin-types)
    vegetation/         -- VegetationModule
    region/             -- RegionReadyModule, RestartModule
    inventory/          -- InventoryTransferModule
    inventory_access/   -- InventoryAccessModule (rez, derez, take, coalesce)
    attachments/        -- AttachmentsModule
    chat/               -- ChatModule
    instant_message/    -- IMModule, MessageTransferModule, OfflineMessageModule
    friends/            -- FriendsModule
    avatar_factory/     -- AvatarFactoryModule (appearance baking)
    lure/               -- LureModule
    gods/               -- GodsModule
    dialog/             -- DialogModule
    baked_textures/     -- BakedTexturesModule
    combat/             -- CombatModule
    gestures/           -- GesturesModule
    entity_transfer/    -- EntityTransferModule, EntityTransferStateMachine
    user_management/    -- UserManagementModule
    monitoring/         -- MonitorModule, region stats
    search/             -- SearchModule
    service_throttle/   -- ServiceThrottleModule
    asset_transaction/  -- AssetTransactionModule
    texture_sender/     -- J2KDecoderModule
    xfer/               -- XferModule
    ip_ban/             -- IPBanModule
    asset_cache/        -- FlotsamAssetCache (memory + disk)
    service_connectors/ -- LandServiceConnector, NeighbourServiceConnector, etc.
    caps/               -- CapabilitiesModule
    dynamic_attributes/ -- DAModule
    interface_commander/-- InterfaceCommanderModule
    library/            -- LibraryModule
```

Dependencies: `welkin-types`, `welkin-region-core` (scene graph, client
view), `welkin-services` (grid-services client), `opensim-protocol` (messages,
LLSD), `sqlx`, `tokio`, `tiny-skia` (map tiles), `tracing`.

### Module trait pattern

Every region module implements `RegionModule`:

```rust
#[async_trait]
pub trait RegionModule: Send + Sync {
    fn name(&self) -> &str;
    fn module_type(&self) -> ModuleType; // Shared or NonShared
    async fn initialise(&mut self, config: &Config) -> Result<()>;
    async fn add_region(&mut self, scene: Arc<Scene>) -> Result<()>;
    async fn remove_region(&mut self, scene: Arc<Scene>) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
}
```

This mirrors the C# `ISharedRegionModule` / `INonSharedRegionModule` split.
Modules register themselves into a `ModuleRegistry` which the scene queries
by interface trait (using `TypeId`-keyed downcasting, matching C#'s
`RequestModuleInterface<T>()`).

### Event system for cross-module communication

The C# code uses .NET events (multicast delegates) for module-to-module
notifications: `OnNewClient`, `OnMakeRootAgent`, `OnChatFromWorld`, etc.
The Rust port uses a typed event bus built on `tokio::sync::broadcast`:

```rust
pub struct SceneEvents {
    pub on_new_client: broadcast::Sender<ClientEvent>,
    pub on_chat: broadcast::Sender<ChatEvent>,
    pub on_object_grab: broadcast::Sender<GrabEvent>,
    // ... one channel per event type
}
```

Each module subscribes to the channels it needs in `add_region()`. This
replaces the C# pattern where modules wire up delegates on scene events.

### Map tile rendering

The C# code has two renderers: `LegacyMapModule` (2D heightmap + prim
outlines) and `Warp3DMapModule` (3D software raytrace using a bundled Warp3D
library). Both are replaced by a single `tiny-skia` renderer that produces
the same 256x256 JPEG tiles the map service expects. The roadmap notes this
as a phase 5 modernisation item. The renderer draws:

1. Terrain heightfield as a colored 2D map (elevation-based palette).
2. Water plane.
3. Prim footprints projected onto the terrain.
4. Agent dots for the agent-location overlay.

### Asset cache

FlotsamAssetCache is a two-level cache (in-memory LRU + on-disk file tree
keyed by asset UUID). The Rust port uses `moka` for the memory tier and
the same directory layout on disk for migration compatibility. Cache
expiration, deep scan and statistics match the C# behaviour.

### Entity transfer

EntityTransferModule is the largest single file (4,169 lines) and handles
teleport, region crossing and child agent management. The HG variant is
deferred to `hypergrid`. The local-grid transfer path covers:

1. **Teleport**: agent leaves origin, seed cap on destination, close on
   origin, complete movement on destination.
2. **Region crossing**: child agent on neighbour, position handoff, reparent.
3. **Child agents**: inform neighbours of avatar presence, close stale
   children.

The state machine (`EntityTransferStateMachine`) tracks in-flight transfers
to prevent double-teleports.

## Module inventory

Every CoreModules subdirectory and its disposition in this change:

| Directory | Lines | Disposition | Notes |
|---|---|---|---|
| World/Land | 5,825 | **port** | Parcels, overlay, prim counts, dwell |
| World/Permissions | 2,644 | **port** | HIGH RISK: all permission checks |
| World/Estate | 3,263 | **port** | Estate settings, telehub, restart |
| World/WorldMap | 2,221 | **port** | Map protocol and tile serving |
| World/LegacyMap | 1,384 | **port** | Map renderer (merged into tiny-skia) |
| World/Warp3DMap | 1,646 | **port** | Map renderer (merged into tiny-skia) |
| World/LightShare | 1,123 | **port** | Windlight settings |
| World/Wind | 867 | **port** | Wind model |
| World/Region | 773 | **port** | Region ready, restart |
| World/Media | 636 | **port** | Media-on-a-prim |
| World/Sound | 469 | **port** | Sound triggers |
| World/Objects | 1,254 | **port** | Object commands, buy/sell |
| World/Serialiser | 478 | **port** | Serialization helper |
| World/Access | 159 | **port** | Region access control |
| World/Vegetation | 135 | **port** | Tree planting |
| World/Terrain | 5,785 | **done** | In region-simulator-core |
| World/Archiver | 3,430 | **defer** | archives change |
| Avatar/Inventory | 2,966 | **port** | Inventory transfers |
| Avatar/Attachments | 1,516 | **port** | Attachment lifecycle |
| Avatar/Chat | 481 | **port** | Local chat |
| Avatar/InstantMessage | 1,947 | **port** | IM routing, offline IM, mute |
| Avatar/Friends | 2,683 | **port** | Friends, online status |
| Avatar/AvatarFactory | 1,335 | **port** | Appearance baking |
| Avatar/Lure | 515 | **port** | Teleport lure |
| Avatar/Gods | 351 | **port** | God mode |
| Avatar/Dialog | 251 | **port** | Dialog delivery |
| Avatar/BakedTextures | 254 | **port** | Baked texture cache |
| Avatar/Commands | 188 | **port** | Console commands |
| Avatar/Combat | 159 | **port** | Damage and health |
| Avatar/Gestures | 115 | **port** | Gesture state |
| Avatar/UserProfiles | 1,894 | **defer** | social-modules |
| Avatar/Groups | 267 | **defer** | social-modules |
| Avatar/Profile | 179 | **defer** | social-modules |
| Framework/EntityTransfer | 4,169 | **port** | Local teleport and crossing only |
| Framework/InventoryAccess | 2,332 | **port** | Rez/derez, local only |
| Framework/Caps | 587 | **port** | Caps infrastructure |
| Framework/DynamicAttributes | 263 | **port** | DAMap |
| Framework/InterfaceCommander | 404 | **port** | Console dispatch |
| Framework/Library | 534 | **port** | Library inventory |
| Framework/Monitoring | 1,444 | **port** | Region stats |
| Framework/Search | 271 | **port** | Directory search |
| Framework/ServiceThrottle | 153 | **port** | Throttle |
| Framework/UserManagement | 1,539 | **port** | User name cache, local only |
| ServiceConnectorsOut (new) | ~2,200 | **port** | Land, Neighbour, MuteList, AgentPrefs, MapImage |
| ServiceConnectorsIn (new) | ~500 | **port** | Land, MapImage, Neighbour |
| Agent/AssetTransaction | 1,163 | **port** | Upload transactions |
| Agent/TextureSender | 431 | **port** | J2K decode queue |
| Agent/Xfer | 492 | **port** | Xfer protocol |
| Agent/IPBan | 220 | **port** | IP ban |
| Asset (FlotsamCache) | 1,723 | **port** | Asset cache |
| Scripting/* | 7,026 | **done/defer** | script-engine / ossl-api |
| SCOut (done in grid-services) | ~5,000 | **done** | Asset, Auth, Grid, etc. |
| SCIn (done or HG) | ~800 | **done/defer** | Login done, HG deferred |

## Per-area design notes

### Land management

The C# `LandManagementModule` implements `ILandChannel` and manages an array
of 4x4-metre land parcels covering the region. Each parcel is a `LandObject`
holding `LandData` (owner, name, flags, sale price, media URL, music URL,
access list, ban list). The module maintains:

- `m_landIDList`: a 2D array mapping region coordinates to local parcel IDs.
- `m_landList`: a dictionary of parcel objects keyed by local ID.
- `PrimCountModule`: counts prims per parcel for the land info panel.
- `DwellModule`: tracks avatar dwell time per parcel.

The Rust port stores parcels in a `Vec<LandParcel>` with the same 4x4 grid.
Parcel data is persisted through the existing `land` and `landaccesslist`
tables in the region database. Viewer protocol: `ParcelInfoReply`,
`ParcelProperties`, `ParcelOverlay`, `ParcelDwellReply`.

### Permissions

`DefaultPermissionsModule` implements 40+ permission check functions hooked
into `ScenePermissions`. Every object create, edit, delete, script run, and
terrain modification passes through it. The module reads config for bypass
mode, admin sets, and propagation rules.

**This is HIGH RISK.** An incorrect permission check can allow unauthorized
object deletion, inventory theft, or god-mode escalation. The Rust port must:

1. Port every permission check function with the same logic paths.
2. Add comprehensive unit tests for each check (admin, owner, group, estate
   manager, god mode, bypass mode).
3. Undergo a dedicated security review (task 14.1).

### Estate management

`EstateManagementModule` manages estate-level settings (covenant, access lists,
ban lists, region flags, sun position, terrain textures) and handles viewer
commands (region restart, kick, teleport home). The telehub manager controls
spawn points. Estate data is stored through the existing estate tables in the
grid database.

### Inventory transfers

`InventoryTransferModule` handles give-item between avatars (the folder drop
in the viewer). `InventoryAccessModule` (Framework/) is the larger piece: it
handles rez from inventory, derez (take, take copy, return, delete), and
coalesced objects. Both depend on the inventory service connector.

### Attachments

`AttachmentsModule` manages the lifecycle of attached objects: wear from
inventory, detach to inventory, save on logout, transfer across region
borders, and the special case of HUD attachments. The module hooks into
entity transfer for cross-region attachment migration.

### Chat and IM

`ChatModule` handles local chat (say, shout, whisper) with range-based
delivery. `MessageTransferModule` routes instant messages between avatars
on the same grid, with `OfflineMessageModule` handling storage for offline
recipients. `MuteListModule` filters by the mute list. `PresenceModule`
tracks online/offline for IM routing.

### World map

`WorldMapModule` handles map block requests (the grid map in the viewer) and
agent location updates. `MapImageModule` generates the 256x256 JPEG tile for
each region. The C# code has two renderers; we replace both with a single
`tiny-skia` 2D renderer producing compatible tiles. The map image is uploaded
to the map image service on region startup and periodically thereafter.

### Environment (LightShare, wind)

`LightShareModule` stores and distributes Windlight environment presets per
region. The viewer receives them through the `EnvironmentSettings` capability.
`WindModule` runs a simple wind simulation (random or configurable) and
distributes wind data through `SimulatorViewerTimeMessage`.

### Entity transfer

`EntityTransferModule` (4,169 lines) is the largest single module. It handles:

1. **Local teleport**: same region, just move the avatar.
2. **Grid teleport**: cross-region, involving seed cap creation on the
   destination, agent data transfer, close on origin.
3. **Region crossing**: continuous movement across a border, child agent
   promotion/demotion.
4. **Child agents**: maintain child connections to neighbouring regions for
   seamless visibility.

The `EntityTransferStateMachine` prevents double-teleports and tracks
in-flight state. HG teleport (`HGEntityTransferModule`) is deferred.

### Service connectors (what is new vs already in grid-services)

Grid-services already built connectors for: Asset, Authentication,
Authorization, Avatar, Grid, GridUser, Inventory, Presence, Simulation,
UserAccounts.

**New in this change:**
- `LandServiceConnector` (out): parcel info lookup across regions.
- `NeighbourServiceConnector` (out): neighbour notification on region startup.
- `MuteListServiceConnector` (out): mute list backend calls.
- `AgentPreferencesServiceConnector` (out): agent preferences backend.
- `MapImageServiceConnector` (out): map tile upload.
- `LandServiceInConnector` (in): handle incoming parcel info requests.
- `MapImageServiceInConnector` (in): handle incoming map tile requests.
- `NeighbourServiceInConnector` (in): handle neighbour hello.

## Dependencies

| Depends on | Why |
|---|---|
| `region-simulator-core` | Scene graph, client stack, capabilities, persistence |
| `grid-services` | Service connectors for inventory, grid, presence, etc. |
| `protocol-core` | Message types, LLSD, LLUDP |
| `script-engine` | Script permission checks, script state on attachment transfer |
| `prim-meshing` | Object bounding boxes for map tile rendering |

| Depended on by | Why |
|---|---|
| `archives` | OAR/IAR need land, permissions, serialization |
| `social-modules` | Groups module needs permission checks |
| `hypergrid` | HG modules extend EntityTransfer, InventoryAccess |
| `cutover` | Production readiness requires all modules |

## Testing strategy

1. **Unit tests per module**: Each module gets focused tests for its core
   logic. Land parcel operations, permission check truth tables, estate
   settings round-trips, inventory transfer flows.

2. **Permission check matrix**: A dedicated test suite covering every
   permission function with combinations of: owner, group member, estate
   manager, god, stranger, bypass on/off. This is the highest-priority
   test target.

3. **Integration tests**: Multi-module scenarios:
   - Rez an object, verify permissions, take it back to inventory.
   - Cross a region border with attachments, verify they survive.
   - Send an IM while recipient is offline, verify offline storage.
   - Buy a parcel, verify ownership change and prim counts.

4. **Protocol replay tests**: Record C# OpenSim responses for land info,
   estate info, map requests and replay against the Rust implementation.

5. **Aurora Viewer acceptance**: A viewer session that exercises chat, IM,
   teleport, attachment wear, land info panel, world map, estate tools.

6. **Security review**: Dedicated adversarial review of the permissions
   module and god-mode paths (task 14.1).
