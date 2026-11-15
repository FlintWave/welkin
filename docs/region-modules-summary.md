# Region modules summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Decisions](decisions.md#rm1) · Previous: [OSSL API](ossl-api-summary.md) · Next: [Archives](archives-summary.md)

Status: accepted (33 of 33 tasks, 13 sections, 278 tests). Security review
verdict: CONDITIONAL PASS. Change: `openspec/changes/region-modules/`.

## What was built

The region modules from `OpenSim/Region/CoreModules/` that are neither terrain,
archiver nor scripting, in one crate: `crates/welkin-modules` (the design
document calls it `welkin-region-modules`; the crate kept the shorter name).
It sits on `region-simulator-core` and `grid-services` and does not depend on
the script VM.

| Section | Area | Tasks |
|---|---|---|
| 1 | Module framework | 1.1-1.2 |
| 2 | Land management | 2.1-2.3 |
| 3 | Permissions (high risk) | 3.1-3.3 |
| 4 | Estate | 4.1-4.2 |
| 5 | Inventory transfer and access, asset upload | 5.1-5.3 |
| 6 | Attachments | 6.1 |
| 7 | Chat, IM, mute, presence | 7.1-7.3 |
| 8 | World map | 8.1-8.2 |
| 9 | Environment and misc world modules | 9.1-9.2 |
| 10 | Avatar modules | 10.1-10.3 |
| 11 | Entity transfer | 11.1-11.2 |
| 12 | Framework utilities and service connectors | 12.1-12.4 |
| 13 | Security review, integration and protocol tests | 13.1-13.3 |

## Architecture

- `RegionModule` trait (`module.rs`) mirrors C# `IRegionModuleBase` with
  `ISharedRegionModule`/`INonSharedRegionModule`. `ModuleType` is `Shared`
  (one instance for all regions) or `NonShared` (one per region). Lifecycle:
  `initialise(config)`, `add_region(scene)`, `remove_region(scene)`, `close()`.
- `ModuleConfig` is a read-only typed config source (string, int, bool,
  float, each with a default), the analogue of `IConfigSource`.
- `ModuleRegistry` (`registry.rs`) owns the modules and a `TypeId` to module
  index map. `register_interface::<I>()` associates an interface type with a
  module; `get_module::<I>()` looks it up. `initialise_all`, `add_region_all`,
  `remove_region_all` and `close_all` drive the lifecycle.
- `SceneEvents` (`events.rs`) is a typed event bus over
  `tokio::sync::broadcast` (default capacity 64). Modules subscribe to
  payloads such as `ObjectAddedEvent` and `ObjectRemovedEvent`, so modules do
  not call each other directly.

## Key modules

- **Land** (`land/`): `LandParcel`, `LandData` (about 40 fields), `ParcelGrid`
  (4 m cells), `ParcelFlags`, add/remove/subdivide/join, access and ban lists,
  `PrimCountModule`, `DwellModule`.
- **Permissions** (`permissions/`): `DefaultPermissionsModule`, config
  (bypass, propagate, debug), admin/god/estate-manager escalation, 13 check
  functions in `checks.rs` (object, script, terrain, parcel, estate).
- **Estate** (`estate/`): settings, region flags, covenant, access/ban/manager
  lists, `TelehubManager`, restart, kick, teleport home, sun and time dilation.
- **Inventory** (`inventory/`): give item/folder with accept/decline,
  rez and derez (`RezResult`, `DerezAction`), `AssetTransactionModule` and
  `XferModule` for uploads.
- **Attachments**: wear/detach, 41 `AttachmentPoint` variants, persistence on
  detach, logout and login.
- **Chat and IM** (`chat/`, `messaging/`): say/shout/whisper by squared range,
  `GridInstantMessage` with 43 `IMDialog` variants, offline IM, mute list,
  presence.
- **World map** (`world_map/`): map blocks, items, agent locations;
  `MapImageModule` renders 256x256 JPEG tiles with tiny-skia, replacing both
  LegacyMap and Warp3DMap.
- **Environment** (`environment/`): LightShare (Windlight), wind models
  (`WindModel` trait), sound, media-on-a-prim, region ready/restart, access,
  vegetation, scene serialiser.
- **Avatar** (`avatar/`): `AvatarFactoryModule` (17 wearable types, 12 bake
  types), baked texture cache, friends with `FriendPerms`, lure, gods, dialog,
  combat, gestures, commands.
- **Entity transfer** (`transfer/`): local and grid teleport, region crossing,
  `TeleportFlags`, six-state state machine that rejects invalid transitions
  and double teleports, child agent management.
- **Framework** (`framework/`): user management, monitoring and region stats,
  library, search stubs, `FlotsamAssetCache` (in-memory LRU; disk tier is a
  stub), J2K texture priority queue, IP ban, caps, dynamic attributes,
  service throttle, five outbound and three inbound service connector stubs.

Hypergrid parts of entity transfer, inventory access and user management are
deferred to the `hypergrid` change; OAR/IAR to `archives`; groups and
profiles to `social-modules`.

## Security review

> [!CAUTION]
> **CONDITIONAL PASS** — 1 CRITICAL (nil UUID ownership bypass), 3 HIGH.
> See [security review](security-review-permissions.md) for details.

[security-review-permissions.md](security-review-permissions.md) (Opus, adversarial, task 13.1):

| Severity | Finding |
|---|---|
| Critical | C-1: nil agent UUID equals nil owner, so ownership checks pass for unowned objects, parcels and an unset estate owner |
| High | H-1: `is_god()` ignores `allow_grid_gods` |
| High | H-2: no god-mode request gate, admins escalate automatically |
| High | H-3: default configuration has bypass on |
| Medium | M-1 to M-4: estate manager not gated by `force_grid_gods_only`, group checks are no-ops, only the owner mask is examined for non-owners, `can_compile_script` ignores bypass |
| Low / Info | 3 low, 5 informational, including missing denial logging and missing checks against C# |

Required before a multi-user grid: C-1 and H-1. Required before production:
H-2, H-3 and M-4. Group checks (M-2, M-3) block correct group behaviour until
the group system exists. Until then, restrict the simulator by network, as
for S5.

## Tests

278 tests, all in `crates/welkin-modules/src/lib.rs`. They include the
framework tests, 21 land tests, a 40-case permission truth table (owner,
group, estate manager, god, stranger, bypass on and off), 27 chat, 21
environment, 15 map image, 30 avatar factory and 29 entity-transfer tests,
13 integration tests (rez/permission/take, cross-region teleport with
attachments, offline IM, parcel buy/sell) and 13 protocol replay and
acceptance tests. The nil-UUID paths flagged in the review have no tests yet.

## Known gaps

Most modules now have real implementations, delivered by later changes
(remaining-parity, viewer-messaging, module-activation). Viewer protocol
handlers for land and search are still stubs; the flotsam disk tier is a
stub. Viewer acceptance against a live Aurora Viewer has been exercised
(login, chat, map, movement, object CRUD all verified).
