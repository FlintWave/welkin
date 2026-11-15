# Module Activation — Design

## Architecture

```
Region Server Startup
    |
    v
Module Registry (currently 9 modules)
    |
    +-- Phase 1: initialize() -- config, service refs
    +-- Phase 2: post_initialize() -- inter-module refs
    +-- Phase 3: add_region(scene) -- subscribe to events
    |
    v
Scene Loop (modules receive events, handle packets/caps)
```

## RegionModule trait

Every module implements this trait:

```rust
pub trait RegionModule: Send + Sync {
    fn name(&self) -> &str;
    fn initialize(&mut self, config: &Config, services: &ServiceContainer);
    fn post_initialize(&mut self);
    fn add_region(&mut self, scene: &Arc<Scene>);
    fn remove_region(&mut self, scene: &Arc<Scene>);
    fn close(&mut self);
}
```

Currently loaded (9): ChatModule, DefaultPermissionsModule,
AvatarFactoryModule, AttachmentsModule, EstateManagementModule,
LandManagementModule, CapsModule, DynAttrsModule, ServiceThrottleModule.

## Load order and dependencies

Modules must be registered in dependency order. The registry calls
`initialize()` on all modules first, then `post_initialize()` (which resolves
inter-module references), then `add_region()`.

```
Tier 0 (no deps):     UserManagementModule, BakedTexturesModule,
                       WindModule, MonitorModule, IPBanModule,
                       RegionReadyModule, AccessModule, PrimCountModule,
                       DwellModule, CombatModule

Tier 1 (scene only):  WorldMapModule, MapImageModule, MapSearchModule,
                       FriendsModule, DialogModule, SoundModule,
                       BasicProfileModule, GesturesModule, GodsModule,
                       RestartModule, LightShareModule, MuteListModule,
                       PresenceModule, SearchModule, LibraryModule

Tier 2 (services):    InventoryAccessModule, InventoryTransferModule,
                       AssetTransactionModule, XferModule,
                       MessageTransferModule, OfflineMessageModule,
                       LureModule, EntityTransferModule, NPCModule,
                       WebRtcVoiceModule

Tier 3 (modules):     GroupsModule (needs MessageTransferModule)
                       GroupsMessagingModule (needs GroupsModule)

Tier 4 (HG):          All HG modules (need EntityTransferModule,
                       UserManagementModule, MessageTransferModule)
```

## Activation strategy

For each module:

1. Add to the module registry vector in region startup
2. Ensure its `initialize()` can resolve all config keys (add defaults if missing)
3. Ensure its `post_initialize()` can find required peer modules
4. Ensure its `add_region()` successfully subscribes to scene events
5. If the module registers UDP packet handlers, verify the packet IDs are in the handler dispatch table
6. If the module registers HTTP caps, verify the cap URLs are seeded
7. Run the region server — module must load without panic
8. Smoke test: send a relevant viewer action and verify a response

## Scene API gaps

Some modules may call scene methods that don't exist yet. Strategy:

- If the method is trivial (getter, event subscription): add it to Scene
- If the method requires significant new logic: mark the module as
  "activated but incomplete" and log a warning on the unimplemented path
- Never stub at the module level — the module either works or is deferred

Reference: OpenSim's `SceneManager.cs` loads modules via
`RegionModulesControllerPlugin.cs` which discovers `INonSharedRegionModule`
and `ISharedRegionModule` implementations, sorts by dependency annotations,
and calls them in order.

## Verification

Each activated module is verified by:
1. Region server starts without panic
2. Module appears in startup log with its name
3. For modules with viewer-facing behavior: manual test or bot test confirms
   the feature works (e.g., map shows tiles after WorldMapModule activation)

## Configuration

All modules are enabled by default (matching OpenSim behavior). Each can be
disabled via config:

```toml
[Modules]
DisabledModules = ["CombatModule", "IRCBridgeModule"]
```
