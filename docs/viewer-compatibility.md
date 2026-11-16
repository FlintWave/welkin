<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Viewer compatibility

[← Back to docs](index.md) · [Integration architecture](integration-architecture.md) · [Architecture](architecture.md)

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
- The region makes its calls to the groups service without holding the session
  lock, so a slow groups service does not freeze the avatar's connection.

Not covered by this work: group roles and role editing, member lists, group
notices and group proposals. The region code has no handlers for the role and
member requests. Treat those as missing until a later change adds them.

Limits to know about:

- Groups are grid-only. Hypergrid avatar IDs are not supported.
- There is no notice or invite handling yet (planned as later milestones).
  Joining a group with closed enrollment is refused until invites exist.
- Only SQLite has been used, as with the rest of Welkin.
- The tests ran in process against the groups router, not against a launched
  `welkin-robust` binary, so the full path from a viewer to a running grid is
  unverified.

## Inventory objects and attachments

These behaviours come from the region code and its end-to-end tests with real
grid services; they are not yet confirmed in a live viewer session.

Taking and returning objects (`DeRezObject`): the region saves the object to
inventory, as an asset and an inventory item, before it removes the object from
the scene. The permissions module must allow the action.

- Take and Take copy put the item in the folder the viewer chose, or the Objects
  folder if it chose none.
- Delete puts the item in Trash when the owner deletes it, and in Lost and
  Found when someone else does.
- Return puts the item in the owner's Lost and Found.
- The item's permissions follow OpenSimulator's rules: the owner keeps the
  object's permissions, and anyone else gets them cut down to the next-owner
  permissions.

Attachments: an attachment is an object worn by an avatar.

- Viewers see a worn object in `ObjectUpdate` with the avatar as its parent, the
  attachment point swapped into the position the viewer expects, and the
  inventory item ID it came from. Attached objects leave the physics engine.
- The region handles wearing an inventory object, one or several at a time,
  attaching an object that lies in the world (a copy goes into inventory first,
  so it can be taken off again), detaching to inventory, and dropping to the
  ground.
- An avatar can wear up to 38 attachments, as in OpenSimulator. The viewer's
  append flag (0x80) adds to what is worn at a point instead of replacing it.
  When the viewer names no point, the region picks the last point the object
  used, or the left hand.
- When an avatar logs out, times out or is ejected, the region saves each
  attachment into its inventory item and removes it from the scene and from
  every viewer.
- The region checks that an attach can succeed before it copies a world object
  into inventory, so a failed attach does not leave a stray inventory copy.
- The region remembers what each avatar wears in the grid's avatar service, in
  one `_ap_<point>` entry per attachment point, and wears the same items again at
  the avatar's next login.

## Terraforming

These behaviours come from the region code and its tests, including
round-trip tests of the terrain encoder against the viewer's decoding
algorithm; they are not yet confirmed in a live viewer session.

- A viewer's `ModifyLand` request edits the scene's terrain, and the changed
  patches are sent to every connected viewer as `LayerData`. The region no
  longer sends a flat 21 m terrain.
- The six brushes ported from OpenSimulator are flatten, raise, lower, smooth,
  noise and revert. A single click paints a round brush; viewers send the
  brush radius in the extended block, and older viewers send only a 1, 2 or 4 m
  size index. A dragged rectangle over the whole region applies the same action
  to the whole area, as OpenSimulator's flood brushes do. An area edit limited
  to one parcel is not supported yet and does nothing.
- The terrain encoder follows the Second Life LayerData format: a discrete
  cosine transform per 16 by 16 patch, zigzag ordering and quantisation. The
  region is 16 by 16 patches. Updates are split into packet-sized payloads, and
  the transform uses lookup tables.
- A request is applied only if the requesting avatar may edit terrain
  (`can_edit_terrain`). Requests with NaN or infinite values, or absurd ones, are
  dropped whole; see the [terraforming security review](security-review-terraforming.md).
- Edited terrain is saved on the backup timer in OpenSimulator's terrain
  format and loaded again at start; see the
  [configuration reference](configuration.md#backup).
- Each avatar can send one terrain edit every 20 ms, as in OpenSimulator.
  Requests that arrive sooner are dropped.
- Golden test vectors generated by OpenSimulator 0.9.3.0 check the encoder: 17
  match byte for byte and 2 are checked by decoding only.

## Avatar movement and physics

These behaviours come from the region code and its tests; they are not yet
confirmed in a live viewer session.

- Avatars stand on the real terrain. The avatar movement code and the physics
  engine read one terrain source, so an avatar and a dropped prim agree on where
  the ground is. A standing avatar's centre sits 1.0 m above the ground.
- A walking avatar can jump. Pressing jump (the UP_POS control while walking)
  gives an upward speed of 5.0 m/s, about 1.3 m of height. An avatar that is in
  the air, including one that stops flying, falls under gravity of 9.80665 m/s^2
  (OpenSimulator's standard value) to a terminal speed of 54 m/s and lands on
  the terrain. A flying avatar cannot go below the ground.
- Each avatar keeps its vertical velocity between updates, and the region sends
  it to viewers. Jump and fall are integrated in fixed 0.02 s sub-steps, so
  jump height does not depend on how often the viewer sends AgentUpdate.
- Physical prims move under the Rapier physics engine. The region copies the
  simulated position and motion back into the scene and sends terse updates to
  viewers, including a final update with zero velocity when an object comes to
  rest. When a script or an edit moves an object, the move is pushed into the
  physics engine so the collider follows.
- The terrain collider is built at the full region size and orientation. An
  earlier version covered only 1 m by 1 m and was transposed, so objects fell
  through the ground.
- Script physics commands reach the physics bodies; see the
  [script engine](scripting.md#physics-19) page.

## Scene delivery and object creation

These behaviours come from the region code and its tests (including a UDP
round trip that parses the sent datagrams); they are not yet confirmed in a
live viewer session.

- When a viewer arrives, the region sends it every object already in the scene.
  The root prim and every linkset child are sent as full object updates, in
  batched ObjectUpdate packets, with their sound fields and texture entry.
- `ObjectAdd` builds the new prim from the shape the viewer sends: prim code,
  state, scale, rotation, and every path and profile field (curve, begin, end,
  scale, shear, twist, taper, revolutions, skew, radius offset, hollow). A
  tube, torus or hollow prim made in the viewer keeps its shape. The state
  value is also copied into the last attach point, as OpenSimulator does. The
  texture entry is left empty, so the prim uses the default plywood texture.
- A full object update, for example after a script changes a sound, sends the
  root and every linkset child, batched. It no longer sends the root alone.
- Rezzing an object from inventory (`RezObject`) fetches the inventory item and
  its asset, reads the object XML, gives every part a new ID, and sets the
  owner to the rezzing avatar and the permissions from the item. The
  permissions module must allow the rez. Coalesced objects are not supported
  yet.
- All three paths build their updates through one function in `scene_sync.rs`,
  so a prim looks the same whichever path sent it.

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

- Terrain edits made in the viewer: see [Terraforming](#terraforming) for what is and is not supported
- Avatar animations on Aurora Viewer (needs AgentAnimation packet)
- Groups are partial: see [Groups](#groups)

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
- Terrain is sent as real LayerData: a DCT, zigzag and quantised encoding of
  the heightfield, in packet-sized payloads (see [Terraforming](#terraforming)).
  The flat-terrain shortcut (range 0, offset 21.0) is gone.

## Sound protocol

Sound messages follow OpenSimulator's behaviour:

- AttachedSound carries flags 0, or QUEUE (0x10) for llPlaySound with
  queueing on.
- SoundTrigger goes only to avatars inside the prim's sound radius; a radius of
  0 is treated as 100 m.
- PreloadSound goes to avatars inside twice the sound radius.
- A looping or stopped sound is carried in a full ObjectUpdate for the root
  prim, so avatars who enter the region later hear the loop. STOP is flag 0x20.
- A prim that has a saved sound comes back from a restart looping, with the
  LOOP flag set and a radius of 100 m. The saved data does not keep the loop
  flag or the radius, so the region restores both for any prim with a sound.

See the [script engine](scripting.md#sound) page for the LSL side.

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
