<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Viewer compatibility

[← Back to docs](README.md) · [Integration architecture](integration-architecture.md) · [Architecture](architecture.md)

Current viewer compatibility status, updated as live testing progresses.

## Supported viewers

| Viewer | Login | In-world | Status |
|---|---|---|---|
| Aurora Viewer | Yes | Partial | First connected 2026-11-14 |
| Firestorm (OpenSim build) | Yes | Working | Tested 2026-11-14 |
| LibreMetaverse bot | Yes | 10/10 steps pass | Automated test client |

## Groups

Groups are off by default; turn them on with `[Groups] Enabled = true` (see the
[configuration reference](configuration.md#groups)). The region talks to the
grid's groups service, which `welkin-robust` provides. These behaviours come from
the code and its tests, not from a live viewer run.

Working:

- Group data is sent at login, so the viewer shows the avatar's groups with
  their real names.
- The active group appears in `AgentDataUpdate`.
- A viewer can create a group, join one, leave one, update a group's info and
  activate a group. The region replies with the viewer's usual reply messages and
  refreshes the avatar's group state after a change.
- A viewer can open a group's profile and read its member list, its roles, the
  titles the avatar may pick, and which members hold which role. The member list
  goes out in replies of up to 25 members, and only to members of the group: a
  request from an avatar that is not a member gets no reply.
- A member with the right to invite can invite another avatar to the group. The
  region stores the invitation in the groups database, so it survives a restart,
  and sends the invitee an instant message. The invitee can accept, which adds
  them to the group and refreshes their group state, or decline. A refused
  invitation shows the inviter "Unable to invite that agent (no permission, or
  already a member)."
- The group founder holds the Owner role only, not Owner and Officer both.

How it differs from OpenSimulator:

- The region refuses a join request for a group with closed enrollment and shows
  the viewer "This group is not open for enrollment." OpenSimulator does not
  check this and relies on the viewer to hide the Join button. This is a
  deliberate improvement.
- Every group handler acts for the authenticated avatar of the connection and
  never reads the agent ID that a packet claims, so one avatar cannot create,
  join, leave or change groups in another avatar's name.
- `GroupPowers` in the login group data is an 8-byte big-endian binary value,
  the same encoding OpenSimulator uses.
- The region makes its calls to the groups service without holding the session
  lock, and runs each group request in its own task. A slow or hung groups
  service therefore does not stall the region's packet loop or the avatar's
  other traffic; a test with a hung service covers this. The connector times out
  a request after 15 seconds and a connection attempt after 5 seconds, and
  speaks the XML protocol that ROBUST's groups service uses.

Not covered by this work: changing roles or their powers, assigning members to
roles, ejecting members, group notices and group proposals. The region has no
handlers for these changes. Treat them as missing until a later change adds
them.

Limits to know about:

- Groups are grid-only. Hypergrid avatar IDs are not supported.
- There is no notice handling yet (a planned milestone). A group with closed
  enrollment can be joined only by accepting an invitation.
- Only SQLite has been used, as with the rest of Welkin.
- The tests ran in process against the groups router, not against a launched
  `welkin-robust` binary, so the full path from a viewer to a running grid is
  unverified.

## Firestorm session (2026-11-14)

Tested with the Firestorm OpenSim build against a local Welkin grid
(ROBUST + one region). Commits 053b903 through cfd4cae cover all work
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
- Cross-region teleport via EntityTransferModule (FSM flow, distance validation up to 4095m)
- Region crossing with child agent handoff
- Hypergrid teleport via gatekeeper resolution and UserAgentService verification
- Sit and stand on objects

### Object interaction

- Permission enforcement on 14 object mutation handlers via DefaultPermissionsModule: ObjectDelete, MultipleObjectUpdate, ObjectName, ObjectDescription, ObjectShape, ObjectPermissions, ObjectSaleInfo, ObjectExtraParams, ObjectGroup, ObjectOwner, ObjectLink, ObjectDelink, ObjectDuplicate, ObjectDuplicateOnRay. Denied mutations log a warning and skip the operation.
- Object persistence wired into region heartbeat (every 60 ticks) and graceful shutdown
- Object creation (ObjectAdd with FullObjectUpdate broadcast)
- Object editing: move, rotate and scale via MultipleObjectUpdate
- ObjectSelect, ObjectDeselect
- Touch events: ObjectGrab, ObjectGrabUpdate, ObjectDeGrab
- Property editing: name, description, shape, permissions, sale info, extra params
- RezObject, DeRezObject
- ObjectLink, ObjectDelink, ObjectDuplicate
- llSetPrimitiveParams/llGetPrimitiveParams with 90+ PRIM_* constants (shape, texture, color, glow, flex, light, sculpt)
- llSetLinkPrimitiveParams/llSetLinkPrimitiveParamsFast
- Vehicle dynamics: llSetVehicleType plus 6 vehicle functions with 46 VEHICLE_* constants and type presets (car, boat, airplane, balloon, sled)
- llRezObject/llRezAtRoot with inventory validation and rez channel
- Attachments: llAttachToAvatar, llDetachFromAvatar, llAttachToAvatarTemp with 38 ATTACH_* constants
- Position and rotation targets: llTarget, llRotTarget with at_target/not_at_target events

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
- llHTTPRequest with 25-pending throttle, config parsing, response channel
- llRequestURL/llRequestSecureURL/llReleaseURL for inbound HTTP caps from scripts
- llEmail/llGetNextEmail with 20s rate limiting and email queue
- llGetNotecardLine/llGetNumberOfNotecardLines via dataserver events
- llRequestAgentData with DATA_* constants via dataserver events

### Avatar and presence

- Avatar name resolution from ROBUST UserAccountService via RemoteAccountsConnector (POST METHOD=getaccount); falls back to "Unknown User" if unreachable
- Avatar profile viewing
- Multi-user presence (new users see existing avatars)
- AgentUpdate and ViewerEffect flowing
- 53 region modules loaded
- 20 NPC functions: osNpcCreate/Remove, osNpcMoveToTarget, osNpcSit/Stand/Touch, osNpcSay/Shout/Whisper, osNpcPlayAnimation/StopAnimation, osNpcSaveAppearance/LoadAppearance
- osTeleportAgent/osTeleportOwner
- osGetAvatarList, osKickAvatar, osForceOtherSit
- osSetSpeed/osGetSpeed with per-avatar speed multiplier
- SetDisplayName cap

### Estate administration

- EstateOwnerMessage dispatch with 11 sub-commands (setregioninfo, setregionterrain, restart, estateaccessdelta, simulatormessage and others)
- FreezeUser: freeze/unfreeze target avatar with alert
- EjectUser via TeleportFailed, remove presence, optional ban
- GodUpdateRegionInfo for god-mode region settings
- RequestRegionInfo with full RegionInfo reply

### Physics

- Rapier physics step running at 11ms (90fps)
- Collision events (collision_start, collision, collision_end) flowing to scripts with per-frame diffing
- Terrain heightfield collision via Scene::set_terrain()

### World and environment

- osSetDynamicTextureURL/osSetDynamicTextureData for runtime texture rendering
- osSetRegionWaterHeight, osSetRegionSunSettings, osSetSunParam/osGetSunParam
- osGetCurrentSunHour
- osSetTerrainHeight/osGetTerrainHeight, osSetTerrainTexture/osSetTerrainTextureHeight
- osTerrainFlush, osSetWindParam/osGetWindParam
- osGetSimulatorMemory (reads /proc/self/status VmRSS)
- osGetRegionSize, osGetRegionStats, osRegionNotice

### Economy

- MoneyModule trait with SampleMoneyModule
- Economy data packets (EconomyData, MoneyBalance)
- XML-RPC endpoints: `/currency.php`, `/landtool.php`
- LSL money functions: llGiveMoney, llTransferLindenDollars, llSetPayPrice
- DTL/NSL currency proxy mode
- Gloebit REST client

### Scripting pipeline (verified end-to-end)

Viewer script uploads use a two-step protocol: the viewer POSTs LLSD metadata
to UpdateScriptAgent, which returns a one-shot uploader URL with
`state: "upload"`. The viewer then POSTs raw LSL source to that URL; the
server compiles it, loads it into the VM, and returns `state: "complete"`.
Upload tokens are single-use (replay returns 404). Direct raw-text POST
still works for bot tests and automation.

Script-to-scene change propagation: llSetPos, llSetRot, llSetScale and
llSetText changes in ScriptContext flow back to SceneObjectPart after each
scheduler tick, with terse ObjectUpdate broadcast for transform changes and
full ObjectUpdate for hover text changes. Threshold-based diffing (0.001)
prevents unnecessary broadcasts.

The bot test validates the full scripting chain: rez a prim, upload LSL,
confirm state_entry fires llSay, send ObjectGrab, confirm touch_start fires
llSay. All 10 bot test steps pass (login, region connect, chat send, chat
echo, movement, rez prim, script upload, script alive, touch event, logout).

### Known limitations

- Full DCT terrain compression (field-order fix pending)
- Avatar animations on Aurora Viewer (needs AgentAnimation packet)

### Not yet user-tested

These features are implemented and code-verified but have not been validated
by a user in a live viewer session:

- Local teleport between regions
- 10-minute soak test on Firestorm (Aurora soak test passed)
- Reconnection after viewer restart
- Full Firestorm re-test after the chat/map/stub fixes

### Known viewer messages

These are Firestorm-internal warnings that appear with an empty inventory and
do not indicate server bugs:

- "Could not put on outfit" (no clothing items in inventory)
- "Could not load settings" (no viewer settings in inventory)

## Aurora Viewer session (2026-11-14)

First viewer connection to Welkin. Aurora uses a non-standard LLUDP flag
layout: ZEROCODED=0x80, ACK=0x10 (swapped vs the Linden Lab specification).
The server handles both layouts via the `aurora-msg` crate. Aurora connects
via XMLRPC login on port 8002 using the `--loginuri` flag for grid selection.

### Connection sequence

Login, circuit established, region handshake, terrain data, avatar
appearance, event queue, presence registered. The full sequence completes
without manual intervention.

### Working

- XMLRPC login on port 8002
- Circuit and region handshake
- 256 terrain patches rendered (flat terrain, height 21.0)
- Avatar visible with wearables and name display
- EventQueue (LLSD long-poll, 25s timeout)
- Minimap (CoarseLocationUpdate at 3s interval)
- Ping keepalive
- Camera rotation
- Capabilities HTTP
- 9 region modules loaded

### Soak test

10+ minutes clean at 60fps with zero errors or disconnects.

### Known gaps

- GetDisplayNames returns 404 (not yet implemented, non-critical)
- Avatar animations (T-pose, needs AgentAnimation packet)
- Movement (needs position update echo)
- Textures (asset service schema bug at time of test)

## Capabilities

42 capability route entries implemented:

| Cap | Status |
|---|---|
| AgentPreferences | Working |
| AvatarPickerSearch | Working |
| ChatSessionRequest | Working |
| CopyInventoryFromNotecard | Working |
| CreateInventoryCategory | Working |
| DispatchRegionInfo | Working |
| EnvironmentSettings | Working (GET and POST) |
| EstateChangeInfo | Working |
| EventQueueGet | Working (25s long-poll) |
| FetchInventory2 | Wired to ROBUST xinventory |
| FetchInventoryDescendents2 | Wired to ROBUST xinventory |
| FetchLib2 | Working |
| GetDisplayNames | Working |
| GetMesh | Proxied to ROBUST asset service |
| GetMesh2 | Proxied to ROBUST asset service |
| GetTexture | Proxied to ROBUST asset service |
| GroupMemberData | Working |
| HomeLocation | Working |
| LandResources | Working |
| MapLayer | Working (POST and GET) |
| MeshUploadFlag | Working |
| NewFileAgentInventory | Working |
| RegionInfo | Working |
| RemoteParcelRequest | Working |
| SearchStatRequest | Working |
| ServerReleaseNotes | Working |
| SetDisplayName | Working |
| SimConsoleAsync | Working |
| SimulatorFeatures | Working |
| UpdateAgentInformation | Working |
| UpdateAgentLanguage | Working |
| UpdateGestureAgentInventory | Working |
| UpdateGestureTaskInventory | Working |
| UpdateNotecardAgentInventory | Working |
| UpdateNotecardTaskInventory | Working |
| UpdateScriptAgent | Two-step LLSD upload (like bake textures); raw POST fallback for bots |
| UpdateScriptTask | Working |
| UploadBakedTexture | Two-phase, stores in ROBUST |
| ViewerAsset | Working (all asset type query params: texture_id, bodypart_id, clothing_id, etc.) |
| ViewerMetrics | Working |
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
| `14d769f` | Infrastructure builtins: llHTTPRequest, llSetPrimitiveParams, vehicle, email, attachments, targets, notecard reading |
| `1dd1302` | 19 new HTTP capability handlers (42 total routes) |
| `fa8de4d` | Estate admin, cross-region teleport, region crossing, HG teleport |
| `cfd4cae` | Physics verification, 20 NPC functions, os* teleport, dynamic textures, 100+ os* functions |
| `e94f119` | Register LSL builtins on VM so scripts actually execute (ScriptContext wiring) |
| `c707f3b` | Bot test: script upload, execution and touch interaction (10 steps) |
| `56d05a3` | ViewerAsset: support all asset type query params (bodypart_id, clothing_id, etc.) |
| `3d68f07` | UpdateScriptAgent: two-step LLSD upload protocol (fixes viewer-initiated uploads) |
| `f94dacd` | Object persistence wired into region heartbeat and shutdown (85+30 field mappings) |
| `69f0db4` | Permission enforcement on 14 object mutation handlers |
| `58e5d0a` | Avatar name resolution from ROBUST, script-to-scene change propagation |
