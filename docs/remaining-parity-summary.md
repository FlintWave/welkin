<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Remaining parity summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Viewer compatibility](viewer-compatibility.md) · [Script engine](scripting.md)

Status: complete (46 of 46 tasks across 4 phases). Change:
`openspec/changes/remaining-parity/`.

## What was built

After the LSL compiler, object interaction, viewer messaging, module
activation and currency work packages, several categories of gaps remained
before Welkin reached full OSgrid parity. This change closed them all in
four phased commits: infrastructure LSL builtins, missing HTTP capabilities,
estate admin with cross-region teleport, and physics verification with os*
function completion.

| Phase | Area | Tasks |
|---|---|---|
| 1 | Infrastructure LSL builtins (deferred from lsl-compiler) | 11 |
| 2 | Missing HTTP capabilities | 16 |
| 3 | Estate admin and cross-region teleport | 8 |
| 4 | Physics verification and os* completion | 11 |

## Phase 1: Infrastructure builtins

11 LSL builtin families that needed external infrastructure beyond the
compiler's scope. Five new modules were added to welkin-script-vm
(prim_params, vehicle, http, email, cap_url), totaling 2,151 lines and
43 new tests.

- llHTTPRequest with 25-pending throttle, URL allowlist/blocklist, timeout,
  and response routed to http_response events
- llRequestURL/llRequestSecureURL/llReleaseURL for inbound HTTP cap URLs
  from scripts
- llEmail/llGetNextEmail with 20s rate limiting and per-script email queue
- llSetPrimitiveParams/llGetPrimitiveParams with 90+ PRIM_* constants
  covering shape, texture, color, glow, flex, light, sculpt and more
- llSetLinkPrimitiveParams/llSetLinkPrimitiveParamsFast for linked prim
  targeting
- llSetVehicleType plus 6 vehicle functions with 46 VEHICLE_* constants and
  type presets (car, boat, airplane, balloon, sled)
- llRezObject/llRezAtRoot with inventory validation and rez channel
- llAttachToAvatar/llDetachFromAvatar/llAttachToAvatarTemp with 38 ATTACH_*
  constants and PERMISSION_ATTACH checks
- llTarget/llRotTarget with at_target/not_at_target events per tick
- llGetNotecardLine/llGetNumberOfNotecardLines via dataserver events
- llRequestAgentData with DATA_* constants via dataserver events

## Phase 2: HTTP capabilities

19 new cap handler functions bringing the total to 42 cap route entries.
All handlers follow the existing axum pattern with LLSD XML request/response.

AvatarPickerSearch, ChatSessionRequest, CopyInventoryFromNotecard,
DispatchRegionInfo, EstateChangeInfo, EnvironmentSettings (GET and POST),
GroupMemberData, LandResources, MeshUploadFlag, RemoteParcelRequest,
SearchStatRequest, SetDisplayName, SimConsoleAsync,
UpdateGestureAgentInventory, UpdateGestureTaskInventory,
UpdateNotecardTaskInventory, UpdateScriptTask, ViewerMetrics,
ServerReleaseNotes.

## Phase 3: Estate admin and cross-region teleport

Packet handlers for estate administration and the teleport system wiring
that had been deferred from earlier work packages.

Estate admin:
- EstateOwnerMessage dispatch with 11 sub-commands (setregioninfo,
  setregionterrain, restart, estateaccessdelta, simulatormessage and others)
- FreezeUser: freeze/unfreeze target avatar with alert
- EjectUser via TeleportFailed, remove presence, optional ban
- GodUpdateRegionInfo for god-mode region settings
- RequestRegionInfo with full RegionInfo reply

Teleport:
- EntityTransferModule: real teleport()/do_teleport() with FSM flow,
  resolve_region_handle(), compute_neighbour() for region crossing,
  distance validation (max 4095m)
- HGEntityTransferModule: teleport_to_foreign_grid() via gatekeeper
  resolution and UserAgentService verification
- Cross-region teleport handler calls EntityTransferModule instead of
  sending TeleportFailed

## Phase 4: Physics and os* completion

Physics verification confirmed Rapier runs at 11ms/90fps. Collision events
(collision_start, collision, collision_end) now flow to scripts with
per-frame diffing. Terrain heightfield collision works via
Scene::set_terrain().

20 NPC functions received real implementations: osNpcCreate/Remove,
osNpcMoveToTarget/StopMoveToTarget, osNpcSit/Stand/Touch,
osNpcSay/SayTo/Shout/Whisper, osNpcPlayAnimation/StopAnimation,
osNpcLookAt, osNpcSaveAppearance/LoadAppearance,
osNpcSetProfileAbout/SetProfileImage, osNpcGetOwner.

osTeleportAgent/osTeleportOwner send TeleportRequest via channel.
osSetDynamicTextureURL/Data send DynamicTextureCommand via channel.

Region, avatar, world, object and communication os* functions were filled
in: osGetSimulatorMemory (reads /proc/self/status VmRSS),
osGetRegionSize, osGetRegionStats, osRegionNotice,
osSetRegionWaterHeight, osSetRegionSunSettings, osSetSunParam/osGetSunParam,
osGetCurrentSunHour, osSetSpeed/osGetSpeed, osGetAvatarList, osKickAvatar,
osForceOtherSit, osSetTerrainHeight/osGetTerrainHeight,
osSetTerrainTexture/osSetTerrainTextureHeight, osTerrainFlush,
osSetWindParam/osGetWindParam, osForceCreateLink/BreakLink,
osSetPrimitiveParams, osSetParcelMediaURL/SIPAddress,
osSetPrimFloatOnWater, osInviteToGroup, osEjectFromGroup.

## Key decisions

- R1: Bundle all remaining gaps into one change instead of separate work
  packages. The items were individually small and shared infrastructure
  (ScriptContext channels, EntityTransferModule wiring).
- R2: Use Rust channel senders in ScriptContext for NPC, teleport and
  dynamic texture commands rather than direct module calls. This keeps the
  VM sandbox boundary clean.
- R3: Physics verification rather than reimplementation. The Rapier
  integration was already in place; this phase confirmed it stepped
  correctly and wired collision events to scripts.
- R4: Rate limits match OpenSim defaults (20s email, 25 pending HTTP
  requests) for behavioral compatibility with existing LSL scripts.

## Tests

All 397 tests pass with zero warnings after Phase 4. Phase 1 added 43 new
tests. Phase 2 includes 17 tests with an updated cap_routes_count assertion
(42). Phase 3 added 12 transfer tests (43 welkin-modules tests total).

## Commits

| Hash | Description |
|---|---|
| `14d769f` | Phase 1: infrastructure builtins (11/11 tasks, 5 new modules, 2151 lines) |
| `1dd1302` | Phase 2: 19 new HTTP capability handlers (42 total routes) |
| `fa8de4d` | Phase 3: estate admin, cross-region teleport, HG teleport |
| `cfd4cae` | Phase 4: physics verification, 20 NPC functions, os* completion |
