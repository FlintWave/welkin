<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Module activation summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Region modules](region-modules-summary.md)

Status: accepted (45 of 45 tasks, 3 phases). Change:
`openspec/changes/module-activation/`.

## What was built

Registered and wired 44 region modules that had real implementations in the
codebase but were never loaded at startup. Before this change only 9 modules
loaded. Afterward, 53 modules load on region startup, covering world map,
friends, groups, instant messaging, inventory transfers, NPC, voice,
search, hypergrid and more.

The modules were activated in three priority tiers:

| Phase | Tier | Modules | Tasks |
|---|---|---|---|
| 1 | P0 (basic usability) | WorldMapModule, MapImageModule, MapSearchModule, FriendsModule, DialogModule, InventoryAccessModule, InventoryTransferModule, AssetTransactionModule, XferModule, MessageTransferModule, BakedTexturesModule, UserManagementModule | 12 |
| 2 | P1 (normal operation) | GroupsModule, GroupsMessagingModule, OfflineMessageModule, MuteListModule, PresenceModule, SoundModule, BasicProfileModule, LibraryModule, SearchModule, GesturesModule, LureModule, GodsModule | 12 |
| 3 | P2 (full parity) | NPCModule, WebRtcVoiceModule, WindModule, LightShareModule, CombatModule, PrimCountModule, DwellModule, MonitorModule, IPBanModule, RegionReadyModule, RestartModule, AccessModule, EntityTransferModule, 8 HG modules, Gatekeeper wiring | 21 |

## Architecture

Every module implements the `RegionModule` trait with a three-phase startup:
`initialize()` (config and service refs), `post_initialize()` (inter-module
refs), `add_region()` (subscribe to scene events, register packet handlers
and HTTP caps).

Modules load in dependency order across five tiers:

| Tier | Dependency | Examples |
|---|---|---|
| 0 | None | UserManagementModule, BakedTexturesModule, WindModule, MonitorModule |
| 1 | Scene only | WorldMapModule, FriendsModule, DialogModule, SoundModule |
| 2 | ROBUST services | InventoryAccessModule, EntityTransferModule, NPCModule |
| 3 | Other modules | GroupsModule (needs MessageTransferModule), GroupsMessagingModule (needs GroupsModule) |
| 4 | HG stack | All 8 HG modules (need EntityTransferModule, UserManagementModule, MessageTransferModule) |

All modules are enabled by default, matching C# OpenSimulator behavior. Any
module can be disabled via the `[Modules] DisabledModules` config key.

## Key decisions

- Activation only, not rewriting. Each module already had a working Rust
  implementation from earlier phases. This change wired them into the startup
  sequence without modifying their internals.
- Scene API gaps filled at the boundary. Where a module called a missing scene
  method, trivial getters and event subscriptions were added to `Scene`. No
  stubs at the module level.
- Dependency-aware ordering. The registry loads all modules in tier order to
  guarantee that `post_initialize()` can resolve peer references.
- HG modules loaded last. The eight hypergrid modules depend on
  EntityTransferModule, UserManagementModule and MessageTransferModule, so
  they sit in their own tier.

## Verification

Each module was verified by confirming the region server starts without panic,
the module appears in the startup log by name, and (for viewer-facing modules)
the feature responds to viewer actions. The final module count (53) is visible
in the viewer-compatibility doc under "Avatar and presence."

## Commits

| Hash | Description |
|---|---|
| `3b7c2d8` | Phase 1: register 12 P0 modules (map, friends, dialog, inventory, baked textures, user management) |
| `3975545` | Phase 2: register 12 P1 modules (groups, offline IM, mute, presence, profiles, search, gestures) |
| `368e61d` | Phase 3: register 20 P2 modules + HG wiring (NPC, voice, wind, combat, entity transfer, 8 HG modules) |
