# Module Activation

## Summary

Register and wire all existing-but-unloaded region modules into the Welkin
region server. The parity audit found ~40 modules with real implementations
(375-1431 lines each) that are defined in the codebase but never registered
in the region startup sequence. Activating them is high-leverage: each module
brings multiple features online with minimal new code.

## Motivation

The ROBUST service layer is 100% complete (17/17 services), but only 9 of ~60
region modules are loaded. Features like world map, friends, groups, instant
messaging, inventory transfers, NPC, voice, and hypergrid all have substantial
Rust implementations that sit unused because they aren't registered with the
scene. This is the single highest-ROI change for closing the OSgrid parity gap.

## Scope

### In scope

- Register each module in the region server startup sequence
- Wire event subscriptions (scene events, packet handlers, cap registrations)
- Verify each module's dependencies are met before loading
- Fix compile errors from unresolved scene APIs at the API boundary
- Respect load order (dependency-aware activation)
- Verify each module loads without panic and registers its handlers

### Modules by priority

**P0 — Basic usability (12 modules):**
WorldMapModule, MapImageModule, MapSearchModule, FriendsModule, DialogModule,
InventoryAccessModule, InventoryTransferModule, AssetTransactionModule,
XferModule, MessageTransferModule, BakedTexturesModule, UserManagementModule

**P1 — Normal grid operation (12 modules):**
GroupsModule, GroupsMessagingModule, OfflineMessageModule, MuteListModule,
PresenceModule, SoundModule, BasicProfileModule, LibraryModule, SearchModule,
GesturesModule, LureModule, GodsModule

**P2 — Full parity (22 modules):**
NPCModule, WebRtcVoiceModule, WindModule, LightShareModule, CombatModule,
PrimCountModule, DwellModule, MonitorModule, IPBanModule, RegionReadyModule,
RestartModule, AccessModule, EntityTransferModule, and all 8 HG modules
(HGEntityTransfer, HGFriends, HGInventoryAccess, HGLure,
HGMessageTransfer, HGUserManagement, HGWorldMap, Gatekeeper wiring)

### Out of scope

- Writing new modules from scratch (tracked in their respective phase changes)
- Implementing missing methods within modules (separate per-module work)
- New UDP packet handlers (tracked in `viewer-packets` change)
- New HTTP capabilities (tracked in `missing-caps` change)

## Phase

Cross-cutting. Can start immediately since code already exists. P0 modules
should land before the next Firestorm test round.

## Risk

Low for P0/P1 modules — they're self-contained with few external dependencies.
Medium for P2 — EntityTransferModule and HG modules have complex inter-module
dependencies and need careful ordering. WebRtcVoiceModule needs the wolfvoice
sidecar to be useful.

## Dependencies

- `firestorm-session` (in progress) — some P0 modules fix known session bugs
- `region-modules` (accepted) — module implementations themselves
- `hypergrid` (accepted) — HG module wiring
- `webrtc-voice` (accepted) — voice module activation
