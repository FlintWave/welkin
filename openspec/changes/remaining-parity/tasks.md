# remaining-parity tasks

## Phase 1: Infrastructure builtins (deferred from lsl-compiler)

- [x] Implement llHTTPRequest, llSetHTTPBody, llHTTPResponse — outbound HTTP with per-script throttle (max 25 pending), URL allowlist/blocklist, timeout, response → http_response event
- [x] Implement llRequestURL, llRequestSecureURL, llReleaseURL — register cap URL on region server, route inbound HTTP → http_request event, return URL to script
- [x] Implement llEmail, llGetNextEmail — send via lettre SMTP, email queue per script, rate limit (20s between sends per OpenSim), email event
- [x] Implement llSetPrimitiveParams, llGetPrimitiveParams — all 40+ PRIM_* / BUILD_* constants from OpenSim (PRIM_TYPE, PRIM_MATERIAL, PRIM_PHYSICS, PRIM_TEMP_ON_REZ, PRIM_PHANTOM, PRIM_POSITION, PRIM_ROTATION, PRIM_SIZE, PRIM_TEXTURE, PRIM_COLOR, PRIM_BUMP_SHINY, PRIM_FULLBRIGHT, PRIM_FLEXIBLE, PRIM_TEXGEN, PRIM_POINT_LIGHT, PRIM_GLOW, PRIM_TEXT, PRIM_NAME, PRIM_DESC, PRIM_ROT_LOCAL, PRIM_PHYSICS_SHAPE_TYPE, PRIM_OMEGA, PRIM_POS_LOCAL, PRIM_LINK_TARGET, PRIM_SPECULAR, PRIM_NORMAL, PRIM_ALPHA_MODE, PRIM_SLICE, PRIM_SIT_TARGET, etc.)
- [x] Implement llSetLinkPrimitiveParams, llSetLinkPrimitiveParamsFast — same as above but targeting linked prims by link number
- [x] Implement llSetVehicleType, llSetVehicleFloatParam, llSetVehicleVectorParam, llSetVehicleRotationParam, llSetVehicleFlags, llRemoveVehicleFlags — all VEHICLE_* constants (TYPE_CAR, TYPE_BOAT, TYPE_AIRPLANE, TYPE_BALLOON, TYPE_SLED, ~40 param constants), integrate with Rapier vehicle dynamics
- [x] Implement llRezObject, llRezAtRoot — fetch item from script's task inventory, deserialize SceneObjectGroup, place at position with velocity and start param, fire on_rez event, handle no-copy removal
- [x] Implement llAttachToAvatar, llDetachFromAvatar, llAttachToAvatarTemp — attachment point constants (ATTACH_CHEST, ATTACH_HEAD, etc.), permission check, attach/detach via AttachmentsModule
- [x] Implement llTarget, llTargetRemove, llRotTarget, llRotTargetRemove — register position/rotation targets with ScriptScheduler, fire at_target/not_at_target/at_rot_target/not_at_rot_target events per tick
- [x] Implement llGetNotecardLine, llGetNumberOfNotecardLines — async notecard reading: fetch notecard asset, parse lines, return via dataserver event with request key
- [x] Implement llRequestAgentData — query ROBUST UserAccounts/GridUser for DATA_ONLINE, DATA_NAME, DATA_BORN, DATA_RATING, DATA_PAYINFO, return via dataserver event

## Phase 2: Missing HTTP capabilities

- [ ] Implement AvatarPickerSearch cap — search user accounts by name prefix, return matches
- [ ] Implement ChatSessionRequest cap — create/join/leave group chat sessions via groups service
- [ ] Implement CopyInventoryFromNotecard cap — extract embedded inventory items from notecard asset
- [ ] Implement DispatchRegionInfo, EstateChangeInfo caps — read/write region and estate settings
- [ ] Implement EnvironmentSettings cap — Windlight/EEP sky/water/day cycle settings, store per-region
- [ ] Implement GroupMemberData cap — paginated group member list with roles
- [ ] Implement LandResources cap — parcel prim limits (max prims, prim counts by category)
- [ ] Implement MeshUploadFlag cap — return whether account has mesh upload permission
- [ ] Implement RemoteParcelRequest cap — look up parcel info by global position
- [ ] Implement SearchStatRequest cap — search places, people, classifieds, events
- [ ] Implement SetDisplayName cap — set avatar display name with cooldown
- [ ] Implement SimConsoleAsync cap — admin console command execution (god-mode only)
- [ ] Implement UpdateGestureAgentInventory, UpdateGestureTaskInventory caps — gesture upload to agent/task inventory
- [ ] Implement UpdateNotecardTaskInventory, UpdateScriptTask caps — notecard/script upload to task inventory
- [ ] Implement ViewerMetrics cap — accept viewer performance stats (log or discard)
- [ ] Implement ServerReleaseNotes cap — return server version/release notes URL

## Phase 3: Estate admin and cross-region teleport

- [ ] Handle EstateOwnerMessage packet — dispatch ~20 sub-commands: setregioninfo, setregionterrain, restart, texturebase, texturedetail, setcovenantid, estateaccessdelta (ban/allow/manager/group lists), simulatormessage, instantmessage, setregiondebug, terraincollisionheight, etc.
- [ ] Handle FreezeUser packet — freeze avatar movement for duration, send alert
- [ ] Handle EjectUser packet — teleport avatar to home location, optional ban
- [ ] Handle GodUpdateRegionInfo packet — god-mode region settings changes
- [ ] Handle RequestRegionInfo packet — send RegionInfo reply with all region properties
- [ ] Wire EntityTransferModule for cross-region teleport — full flow: DoTeleport, create agent at destination region, send child agent seed caps, viewer UpdateSimulator, close source child agent
- [ ] Wire EntityTransferModule for region crossing — avatar walks across border, create child agent at neighbor, seamless handoff
- [ ] Verify HG teleport flow — gatekeeper link, user agent verification, HGEntityTransferModule already loaded from module-activation, test agent transfer via hypergrid URI

## Phase 4: Physics verification and os* completion

- [ ] Verify Rapier physics.step() is called in region main loop — if missing, add physics tick with configurable dt (default 11ms = 90fps matching OpenSim)
- [ ] Verify collision callbacks → script collision_start/collision/collision_end events fire correctly
- [ ] Verify terrain heightfield collision — avatars/objects collide with terrain, not fall through
- [ ] Implement real os* NPC functions — osNpcCreate, osNpcRemove, osNpcMoveTo, osNpcSay, osNpcSit, osNpcStand, osNpcGetPos, osNpcGetRot, osNpcSetRot, osNpcTouch (20 functions, currently stubs)
- [ ] Implement real os* teleport functions — osTeleportAgent, osTeleportOwner (via EntityTransferModule)
- [ ] Implement real os* dynamic texture — osSetDynamicTextureURL, osSetDynamicTextureURLBlend, osSetDynamicTextureData (HTTP fetch image → apply as prim texture)
- [ ] Implement real os* region functions — osGetSimulatorMemory, osGetSimulatorVersion, osGetRegionSize, osGetRegionStats, osRegionRestart, osRegionNotice, osSetRegionWaterHeight, osSetRegionSunSettings
- [ ] Implement real os* avatar functions — osGetAvatarList, osGetAgentIP (admin only), osKickAvatar, osForceOtherSit, osSetSpeed, osSetOwnerSpeed
- [ ] Implement real os* world functions — osSetTerrainHeight, osGetTerrainHeight, osSetTerrainTexture, osSetTerrainTextureHeight, osTerrainFlush, osSetSunParam, osGetSunParam, osSetWindParam, osGetWindParam
- [ ] Implement real os* object functions — osGetNotecard, osGetNotecardLine, osGetNumberOfNotecardLines (sync versions unlike ll* async), osMakeNotecard, osForceCreateLink, osForceBreakLink, osSetPrimitiveParams
- [ ] Implement real os* communication — osSetParcelMediaURL, osSetParcelSIPAddress, osSetPrimFloatOnWater, osInviteToGroup, osEjectFromGroup
