<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Remaining parity summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Viewer compatibility](viewer-compatibility.md) · [Script engine](scripting.md)

## Current project state

26 Rust crates, 145k lines, 2,209 tests. 643 of 757 tracked tasks complete
(85%) across 22 completed changes and 6 active ones. The archived changes
cover: port foundation, open-source audit, protocol core, physics backend,
grid services, capacity assessor, grid portal, region simulator core, prim
meshing, script engine, OSSL API, region modules, object interaction, viewer
messaging, module activation, currency module, social modules, external grid
mode, hypergrid, WebRTC voice, archives, and remaining parity.

### Active changes

| Change | Done | Total | Focus |
|---|---|---|---|
| lsl-compiler | 69 | 69 | LSL-to-Luau transpiler, 206 ll* builtins |
| firestorm-session | 29 | 36 | Firestorm viewer compatibility (7 user-validation items remain) |
| integration-wiring | 30 | 35 | End-to-end wiring (5 user-validation items remain) |
| realmwalker | 21 | 80 | Rust bot SDK (Phases 1-2 done, Phases 3-11 pending) |
| modern-avatar-features | 0 | 25 | PBR materials, Bakes on Mesh, Animesh, glTF, EEP |
| script-testing | 0 | 18 | Script engine test coverage and lifecycle verification |

Two proposed changes (plugin-api and llm-world-copilot) have no tasks yet.

### What works now

Verified through viewer sessions (Aurora, Firestorm) and the 10-step bot test:

- XMLRPC and LLSD login on both viewers
- Circuit establishment, region handshake, terrain rendering
- Avatar appearance with baked texture upload
- WASD movement (walk, run, fly), sit, stand
- Chat send and receive (ChatFromViewer routing)
- Map and minimap rendering (MapLayer cap)
- Object creation, editing, duplication, linking
- Script upload via two-step LLSD protocol (UpdateScriptAgent)
- Script execution: state_entry fires, touch_start fires, llSay works
- 206 ll* builtins across 18 modules (math, string, list, object, sensor, timer, physics, communication, HTTP, email, attachments, vehicle, prim params, targets, notecard, dataserver, cap URLs, control)
- 236 os* extension functions across 13 modules, gated by 8 threat levels
- 42 HTTP capability routes
- 53 region modules loaded
- 20 NPC functions with real implementations
- Rapier physics at 11ms (90fps), collision events flowing to scripts
- Object persistence wired into region heartbeat (every 60 ticks) and graceful shutdown, with 85+30 field mappings from SceneObjectPart to database rows
- Permission enforcement on 14 object mutation handlers (ObjectDelete, MultipleObjectUpdate, ObjectName, ObjectDescription, ObjectShape, ObjectPermissions, ObjectSaleInfo, ObjectExtraParams, ObjectGroup, ObjectOwner, ObjectLink, ObjectDelink, ObjectDuplicate, ObjectDuplicateOnRay)
- Avatar name resolution from ROBUST UserAccountService (replaces hardcoded names)
- Script-to-scene change propagation: llSetPos, llSetRot, llSetScale, llSetText changes flow from ScriptContext back to SceneObjectPart each scheduler tick, with ObjectUpdate broadcast to viewers
- OAR/IAR archive load and save
- Estate administration (11 sub-commands, freeze, eject, god mode)
- Cross-region teleport, hypergrid teleport
- Instant messaging, groups, friends, profiles
- Tauri 2 desktop companion app
- WebRTC voice via wolfvoice sidecar
- Economy: MoneyModule, EconomyData, Gloebit REST client

### Not yet working

These features are defined in OpenSimulator 0.9.3.0 but not yet ported or
not yet user-tested:

- PBR materials (glTF material rendering pipeline)
- Bakes on Mesh (server-side texture compositing for mesh bodies)
- Animesh (animated mesh objects driven by BVH)
- glTF mesh import/export
- Extended Environment (EEP) settings (Windlight partial only)
- SearchModule (viewer search for places, people, groups, events)
- Media on a Prim (MOAP / shared media)
- Grid portal (scaffold only: the first-run wizard, staff auth and RBAC have code, but most handlers return fake success and the portal is not wired into any binary)
- Animation overrides (viewer-driven animation replacement)
- Real attachments from inventory (forced attachments work, viewer-initiated pending)
- Groups database persistence (in-memory only)
- Script state checkpoint and restore across region restarts
- Some LSL functions are not wired into the running region: llEmail, llHTTPRequest, llRezObject, llAttachToAvatar and others (see the README)
- Full Firestorm re-test after chat/map/stub fixes
- A 10-minute soak test on either viewer (no completed run is on record)
- Local teleport between regions (code exists, not user-tested)

### Feature comparison vs OpenSimulator 0.9.3.0

A rough breakdown by subsystem, counting parity by implemented protocol
handlers and viewer-visible behavior, not by line count:

| Subsystem | Parity | Notes |
|---|---|---|
| Login and circuit | 100% | XMLRPC, LLSD, UseCircuitCode, RegionHandshake |
| Grid services (ROBUST) | 100% | 14 service handlers, replay-tested |
| Region simulator core | 95% | Scene, terrain, presences; pathfinding deferred |
| Object interaction | 95% | Full CRUD, link, rez, properties; MOAP missing |
| Inventory | 90% | Folder tree, caps, transfers; viewer-initiated attach pending |
| Scripting (LSL) | 85% | 206/~310 SL-spec ll* functions; compiler handles 68% of real scripts |
| Scripting (OSSL) | 90% | 236/258 os* functions; 22 are deprecated aliases |
| Physics | 90% | Rapier backend, collisions, vehicles; pathfinding deferred |
| Communication | 95% | Chat, IM, groups messaging, ViewerEffect relay |
| Social | 90% | Groups, friends, profiles, NPCs; groups DB is in-memory |
| Economy | 90% | MoneyModule, Gloebit; DTL/NSL proxy mode |
| Teleport | 90% | Local, cross-region, hypergrid; user validation pending |
| Voice | 100% | WebRTC via wolfvoice (Vivox/FreeSWITCH not ported, Aurora-only) |
| Archives | 100% | OAR/IAR single and multi-region |
| Estate admin | 95% | 11 sub-commands, freeze/eject/god; ban persistence partial |
| Avatar rendering | 70% | Baked textures work; PBR, BoM, Animesh, glTF missing |
| Environment | 60% | Windlight partial; EEP, dynamic sky not ported |
| HTTP capabilities | 80% | 42/~55 standard caps implemented |
| Portal/admin | 100% | Grid portal, wizard, companion app |

Estimated overall parity: ~85% of user-visible features.

The main gaps are in avatar rendering (PBR, Bakes on Mesh, Animesh, glTF),
environment (EEP), and a few viewer-facing features (search, MOAP, animation
overrides). These are tracked in the modern-avatar-features change (25 tasks).

---

## Build history: remaining-parity change

Change: `openspec/changes/remaining-parity/` (archived, 46/46 tasks). This
change closed the last gaps after the LSL compiler, object interaction, viewer
messaging, module activation, and currency work packages.

| Phase | Area | Tasks |
|---|---|---|
| 1 | Infrastructure LSL builtins (deferred from lsl-compiler) | 11 |
| 2 | Missing HTTP capabilities | 16 |
| 3 | Estate admin and cross-region teleport | 8 |
| 4 | Physics verification and os* completion | 11 |

### Phase 1: Infrastructure builtins

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

### Phase 2: HTTP capabilities

19 new cap handler functions bringing the total to 42 cap route entries.
All handlers follow the existing axum pattern with LLSD XML request/response.

AvatarPickerSearch, ChatSessionRequest, CopyInventoryFromNotecard,
DispatchRegionInfo, EstateChangeInfo, EnvironmentSettings (GET and POST),
GroupMemberData, LandResources, MeshUploadFlag, RemoteParcelRequest,
SearchStatRequest, SetDisplayName, SimConsoleAsync,
UpdateGestureAgentInventory, UpdateGestureTaskInventory,
UpdateNotecardTaskInventory, UpdateScriptTask, ViewerMetrics,
ServerReleaseNotes.

### Phase 3: Estate admin and cross-region teleport

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

### Phase 4: Physics and os* completion

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

### Commits

| Hash | Description |
|---|---|
| `14d769f` | Phase 1: infrastructure builtins (11/11 tasks, 5 new modules, 2151 lines) |
| `1dd1302` | Phase 2: 19 new HTTP capability handlers (42 total routes) |
| `fa8de4d` | Phase 3: estate admin, cross-region teleport, HG teleport |
| `cfd4cae` | Phase 4: physics verification, 20 NPC functions, os* completion |
