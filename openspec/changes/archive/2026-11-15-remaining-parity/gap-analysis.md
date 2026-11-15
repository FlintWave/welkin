# Welkin vs OpenSim 0.9.3.0: Gap Analysis

Generated 2026-11-15 from disk audit of Welkin codebase and OpenSim 0.9.3.0 C# source.

## Summary

| Category | OpenSim | Welkin | Coverage |
|----------|---------|--------|----------|
| LSL functions (VM builtins) | 456 | 95 | 21% |
| OSSL functions | 258 | 163 real + 155 stubs (318 registered) | 63% real |
| Capability handlers | 63 | ~45 | 71% |
| ROBUST services | 21 | 17+ | ~81% |
| Hypergrid services | 10 | 10 | 100% |
| Database backends | 3 | 3 | 100% |
| Physics engines | 4 | 1 (Rapier) | 25% (by design) |

Welkin's infrastructure (networking, services, database, hypergrid) is solid. The
biggest gaps are in the scripting runtime (361 missing LSL builtins) and the viewer-facing
features that scripts and viewers expect to work (materials, environment, media, search).

---

## P0: Viewer cannot function without these

Without these, a viewer logs in but can't do basic operations. Aurora or Firestorm
will show errors, crash handlers, or missing content.

| Feature | OpenSim | Welkin | Effort | Notes |
|---------|---------|--------|--------|-------|
| LSL math builtins (llSin, llCos, llSqrt, llPow, llAbs, llFabs, llFloor, llCeil, llRound, llLog, llLog10, llAtan2, llAcos, llAsin, llFrand, llModPow) | 16 functions | MISSING | S | Pure math, no server state. Port from LSL_Api.cs. |
| LSL string builtins (llSubStringIndex, llGetSubString, llDeleteSubString, llInsertString, llToUpper, llToLower, llStringLength, llStringTrim, llParseString2List, llParseStringKeepNulls, llDumpList2String, llEscapeURL, llUnescapeURL, llMD5String, llSHA1String, llSHA256String, llBase64ToString, llStringToBase64, llBase64ToInteger, llIntegerToBase64, llXorBase64StringsCorrect, llHash, llOrd, llChar, llReplaceSubString) | 25 functions | MISSING | S | Pure string ops, no server state. |
| LSL list builtins (llGetListLength, llList2Integer, llList2Float, llList2String, llList2Key, llList2Vector, llList2Rot, llList2List, llList2ListStrided, llList2CSV, llCSV2List, llListFindList, llListFindStrided, llListInsertList, llListReplaceList, llListSort, llListSortStrided, llListRandomize, llListStatistics, llDeleteSubList, llGetListEntryType, llList2Json, llJson2List, llJsonGetValue, llJsonSetValue, llJsonValueType, llList2ListSlice, llListSortStrided) | ~28 functions | MISSING | S | Pure list/JSON ops. |
| LSL rotation/vector math (llEuler2Rot, llRot2Euler, llRot2Fwd, llRot2Left, llRot2Up, llRot2Angle, llRot2Axis, llAxes2Rot, llAxisAngle2Rot, llRotBetween, llAngleBetween, llVecDist, llVecMag, llVecNorm, llLinear2sRGB) | 15 functions | MISSING | S | Pure math on Quaternion/Vector3. |
| LSL detected-event functions (llDetectedKey, llDetectedName, llDetectedOwner, llDetectedPos, llDetectedRot, llDetectedVel, llDetectedGrab, llDetectedGroup, llDetectedLinkNumber, llDetectedType, llDetectedTouchFace, llDetectedTouchPos, llDetectedTouchST, llDetectedTouchUV, llDetectedTouchNormal, llDetectedTouchBinormal) | 16 functions | MISSING | M | Need event context stack in the VM scheduler. Every touch/collision/sensor event depends on these. |
| LSL prim property getters/setters (llGetPrimitiveParams, llSetPrimitiveParams, llGetLinkPrimitiveParams, llSetLinkPrimitiveParams, llSetLinkPrimitiveParamsFast, llGetNumberOfSides, llGetLinkNumberOfSides, llGetLinkName) | 8 functions | MISSING | L | Complex: must encode/decode PRIM_* constant lists. Core to every builder script. |
| LSL text/hovertext (llSetText) | 1 function | MISSING | S | High visibility. Every vendor, greeter, and info board uses this. |
| LSL particle system (llParticleSystem, llLinkParticleSystem) | 2 functions | MISSING | M | Encodes PSYS_* constants into viewer-side particle packet. Fire, smoke, sparkles all need this. |
| LSL texture functions (llGetTextureOffset, llGetTextureRot, llGetTextureScale, llOffsetTexture, llRotateTexture, llScaleTexture, llSetTextureAnim, llSetLinkTextureAnim) | 8 functions | MISSING | M | Texture animations (spinning signs, scrolling water). |
| LSL sound (llPlaySound, llLoopSound, llStopSound, llPreloadSound, llTriggerSound, llTriggerSoundLimited, llAdjustSoundVolume, llLinkPlaySound, llLinkStopSound, llSound, llSoundPreload, llCollisionSound, llSetSoundRadius, llSetSoundQueueing, llLinkSetSoundRadius, llLinkSetSoundQueueing, llLoopSoundMaster, llLoopSoundSlave, llPlaySoundSlave) | 19 functions | MISSING | M | Ambient sounds, door sounds, music triggers. |
| LSL dialog (llDialog, llTextBox) | 2 functions | MISSING | S | ScriptDialog packet to viewer. Every menu-driven script uses llDialog. |
| LSL HTTP (llHTTPRequest, llHTTPResponse, llGetHTTPHeader, llRequestURL, llRequestSecureURL, llReleaseURL, llGetFreeURLs, llSetContentType) | 8 functions | MISSING | M | llHTTPRequest exists in Welkin's http.rs builtin but llHTTPResponse, URL hosting, headers need real work. |
| LSL sleep/time (llSleep, llGetTimeOfDay, llGetUnixTime, llGetTimestamp, llGetDate, llGetGMTclock, llGetWallclock, llGetAndResetTime, llMinEventDelay) | 9 functions | MISSING | S | Trivial, but scripts call llSleep and llGetUnixTime constantly. |
| LSL link messages (already partial) | llMessageLinked exists | PARTIAL | S | Verify full link_message event dispatch works end-to-end. |

**P0 LSL subtotal: ~157 functions, mostly S/M effort. These are the bread and butter of every LSL script.**

| Feature | OpenSim | Welkin | Effort | Notes |
|---------|---------|--------|--------|-------|
| Avatar appearance baking | Server-side bake pipeline (AvatarAppearanceModule), UploadBakedTexture cap with actual asset storage | Cap accepts upload but no bake pipeline, placeholder storage | L | Without baking, avatars appear as clouds or grey. Need to process wearable layers into baked textures and store as assets. |
| Attachment system | Full attach/detach with scene graph wiring, persistence in avatar_attachments table, link to inventory | Wear creates placeholder object IDs, not wired to scene graph | L | Attachments are visible on every avatar. Broken attachments = broken avatars. |
| Environment settings / EEP | ExtEnvironment cap, Windlight settings, day cycle state machine, per-parcel overrides | Cap returns hardcoded defaults, day_cycle field exists but unused | L | Controls sky, sun position, water. Default hardcoded sky renders but no time-of-day changes, no viewer EEP support. |

---

## P1: Core features users expect to work

A viewer session is functional without these, but users will notice their absence immediately.
Scripts relying on these features will silently fail.

| Feature | OpenSim | Welkin | Effort | Notes |
|---------|---------|--------|--------|-------|
| LSL region/parcel queries (llGetRegionName, llGetRegionFlags, llGetRegionFPS, llGetRegionTimeDilation, llGetRegionCorner, llGetRegionAgentCount, llGetParcelDetails, llGetParcelFlags, llGetParcelMaxPrims, llGetParcelPrimCount, llGetParcelPrimOwners, llGetParcelMusicURL, llSetParcelMusicURL, llGetLandOwnerAt, llOverMyLand, llEjectFromLand, llAddToLandBanList, llRemoveFromLandBanList, llAddToLandPassList, llRemoveFromLandPassList, llResetLandBanList, llResetLandPassList, llManageEstateAccess, llModifyLand) | 24 functions | MISSING | M | Need to wire to region/parcel state. |
| LSL object queries (llGetObjectDetails, llGetObjectMass, llGetObjectPrimCount, llGetObjectPermMask, llSetObjectPermMask, llGetObjectLinkKey, llGetOwnerKey, llGetCreator, llGetBoundingBox, llGetGeometricCenter, llGetCenterOfMass, llGetMass, llGetMassMKS, llGetEnergy) | 14 functions | MISSING | M | Need scene graph queries. |
| LSL script management (llResetScript, llResetOtherScript, llGetScriptName, llGetScriptState, llSetScriptState, llGetFreeMemory, llGetUsedMemory, llGetSPMaxMemory, llGetMemoryLimit, llSetMemoryLimit, llScriptProfiler, llGetStartParameter, llRemoteLoadScriptPin, llSetRemoteScriptAccessPin) | 14 functions | MISSING | M | Script lifecycle management. |
| LSL camera (llSetCameraParams, llClearCameraParams, llSetCameraAtOffset, llSetCameraEyeOffset, llSetLinkCamera, llForceMouselook, llReleaseCamera, llTakeCamera, llGetCameraPos, llGetCameraRot, llGetCameraAspect, llGetCameraFOV) | 12 functions | MISSING | M | Camera control packets to viewer. |
| LSL animation (llGetAnimation, llGetAnimationList, llGetAnimationOverride, llSetAnimationOverride, llResetAnimationOverride, llStartObjectAnimation, llStopObjectAnimation, llGetObjectAnimationNames) | 8 functions | MISSING | M | Animation overrides for walks, sits, etc. Welkin has llStartAnimation/llStopAnimation but not the override system. |
| LSL vehicle (llSetVehicleType, llSetVehicleFloatParam, llSetVehicleVectorParam, llSetVehicleRotationParam, llSetVehicleFlags, llRemoveVehicleFlags) | 6 functions | MISSING | M | Vehicle module exists in physics builtin but these setters are not registered. |
| LSL link operations (llCreateLink, llBreakLink, llBreakAllLinks, llGetLinkKey, llGetLinkNumber, llGetLinkName, llLinkSitTarget, llSetLinkAlpha, llSetLinkColor, llSetLinkTexture) | 10 functions | MISSING | M | Link set management. |
| LSL agent queries (llGetAgentLanguage, llGetAgentList, llGetAgentInfo, llGetAgentSize, llRequestAgentData, llGetUsername, llRequestUsername, llKey2Name, llName2Key, llRequestUserKey, llGetDisplayName, llRequestDisplayName, llIsFriend, llSameGroup) | 14 functions | MISSING | M | Agent info lookups. Scripts use llKey2Name and llGetAgentList heavily. |
| LSL sit/permissions (llSitTarget already exists, but llAvatarOnLinkSitTarget, llSetSitText, llSetTouchText, llSetPayPrice, llSetClickAction, llPassCollisions, llPassTouches, llVolumeDetect, llSetDamage, llCollisionFilter, llScriptDanger, llSetStatus) | 13 functions | MISSING | S-M | Interaction settings. |
| LSL keyframed motion (llSetKeyframedMotion) | 1 function | MISSING | M | Animated doors, elevators, moving platforms. |
| LSL money (llGiveMoney, llTransferLindenDollars) | 2 functions | MISSING | S | Wire to MoneyModule. |
| LSL URL loading (llLoadURL, llMapDestination) | 2 functions | MISSING | S | Send URL dialog to viewer. |
| LSL look-at (llLookAt, llRotLookAt, llStopLookAt, llPointAt, llStopPointAt) | 5 functions | MISSING | S | Viewer effect packets. |
| LSL physics (llGroundContour, llGroundRepel, llGroundSlope, llSetAngularVelocity, llSetVelocity, llSetForceAndTorque, llGetForce, llGetTorque, llPushObject, llScaleByFactor, llGetMaxScaleFactor, llGetMinScaleFactor) | 12 functions | MISSING | M | Extended physics interactions. |
| LSL linkset data (llLinksetDataWrite, llLinksetDataRead, llLinksetDataDelete, llLinksetDataReset, llLinksetDataCountKeys, llLinksetDataListKeys, llLinksetDataFindKeys, llLinksetDataAvailable, llLinksetDataCountFound, llLinksetDataDeleteFound, llLinksetDataWriteProtected, llLinksetDataReadProtected, llLinksetDataDeleteProtected) | 13 functions | MISSING | M | Key-value store per linkset. Newer feature, increasingly used. |
| OSSL terrain stubs | 20 functions (osSetTerrainHeight, osGetTerrainHeight, etc.) | STUBBED | M | Welkin has terrain brushes; these need to wire to them. Reference terrain_stubs.rs and OpenSim's OSSL_Api.cs. |
| OSSL agent stubs | 21 functions (osGetAgentIP, osKickAvatar, osForceOtherSit, etc.) | STUBBED | M | Need agent/presence data access. |
| OSSL NPC stubs | 33 functions (osNpcCreate, osNpcRemove, osNpcSay, osNpcMoveTo, etc.) | STUBBED | M | NPC module exists; wire the OSSL functions to it. |
| OSSL inventory stubs | 52 functions (osForceCreateLink, osGiveInventoryList, etc.) | STUBBED | L | Inventory service calls from script context. Largest stub file. |
| OSSL misc stubs | 29 functions (osSetParcelDetails, osSetRegionSunSettings, etc.) | STUBBED | M | Mixed bag. Parcel, sun, region state, teleport helpers. |
| Materials/PBR/GLTF | RenderMaterials cap, ModifyMaterialParams cap, GLTF material override system, per-face material assignments | Only basic material byte (wood/stone/etc.). DB migration has terrain PBR columns but no runtime support. | XL | Modern viewers send PBR material data. Without support, materials render as blank or fallback. |
| Directory/search service | DirFind* packets, search for people/places/land/events/classifieds, SearchModule wired to backend | SearchModule exists, search_places and search_people return empty Vec | L | Users expect in-world search to work. |
| Map search | Map tiles with search, people/place/event search on world map | MapImageModule generates terrain tiles, upload_map_tile is stub | M | World map works but search returns nothing. |
| Dynamic textures | DynamicTextureModule, osSetDynamicTextureURL/Data, vector rendering on prims | Command enum defined, OSSL functions are stubs | L | Signs, displays, rendered text on prims. |
| Groups database connector | XmlRpcGroupsServicesConnector or database-backed groups, proposals, votes | In-memory StubGroupsConnector, proposals/votes return empty | M | Groups work in-memory but don't persist across restarts. Need real DB connector. |
| Media on a Prim (MOAP) | ObjectMedia cap, ObjectMediaNavigate cap, per-face media entries, shared media | MediaEntry struct defined, no cap handlers, no packet handling | L | Web pages on prim faces. |
| Parcel media/music | llGetParcelMusicURL, llSetParcelMusicURL, ParcelMediaCommandList, ParcelMediaQuery | MISSING | M | Streaming audio URLs on parcels. |

**P1 subtotal: ~204 LSL functions + 155 OSSL stubs to fill + materials + search + media + groups DB.**

---

## P2: Important for parity, not blocking basic use

| Feature | OpenSim | Welkin | Effort | Notes |
|---------|---------|--------|--------|-------|
| Experience system | llAgentInExperience, llCreateKeyValue, llReadKeyValue, etc. Key-value store, experience permissions, experience profiles | MISSING | L | OpenSim added this relatively recently. Used by some content creators. |
| Gesture playback | Gesture activation, playback sequencing, chat triggers | Cap handlers log body only, no activation/playback system | M | Gestures are inventory items that trigger animation+sound+chat sequences. |
| Animation override (AO) system | Full AO with llSetAnimationOverride, per-state overrides, persistence | llStartAnimation/llStopAnimation only | M | Custom walk, sit, fly animations. |
| God mode tools | GodUpdateRegionInfo, god kick, god teleport, region management packets | Cap exists, no comprehensive packet handling | M | Admin operations. OpenSim has extensive god-mode packet handlers. |
| Region restart scheduling | ScheduleRestart, restart countdown, viewer notification | MISSING | S | Graceful restarts with user warning. |
| Telehub system | Telehub placement, spawn point routing | MISSING | M | Landing point management for regions. |
| Collision sounds | Per-prim collision sound assignment, llCollisionSound | MISSING | S | Wire to physics collision events. |
| Wearable/outfit management | Outfit folder management, server-side outfit service, appearance persistence | Cap accepts baked texture uploads, no outfit folder management | M | COF (Current Outfit Folder) management. |
| Estate access lists | Full ban list, access list, estate manager list, group access | Estate module exists but coverage unclear | M | Verify and fill gaps in estate access management. |
| Varregion terrain stitching | Variable-size regions (256x256 to 8192x8192), terrain stitching across region boundaries | Region manager exists, no var-region terrain stitching | L | Large region support. |
| Additional physics engines | ubODE (default in OpenSim), BulletSim, BasicPhysics, POS | Rapier only | XL | Single engine is fine for release if Rapier covers the behavior. ubODE parity is optional. |
| LSL remaining (~43 functions) | Deprecated/specialized: llCloud, llWind, llEdgeOfWorld, llMakeExplosion/Fire/Fountain/Smoke (deprecated particle shortcuts), llRemoteData* (XMLRPC, deprecated), llRefreshPrimURL/llSetPrimURL (deprecated), llCollisionSprite (deprecated), llGodLikeRezObject, llTargetedEmail, llXorBase64Strings (deprecated), llSendRemoteData, llRemoteDataReply, llOpenRemoteDataChannel, llCloseRemoteDataChannel, llRemoteDataSetRegion, llRemoteLoadScript | MISSING | S-M | Many are deprecated. Implement the non-deprecated ones; the deprecated ones can return void or log a warning. |
| Throttle module | Per-service token bucket throttling | Stub | M | Rate limiting for services. Not critical for small deployments. |
| Capability lifecycle management | Dynamic create/destroy of per-agent HTTP handlers | Stub description | M | Current system works but could leak handlers on disconnect. |

---

## P3: Nice to have, not required for parity claim

| Feature | OpenSim | Welkin | Effort | Notes |
|---------|---------|--------|--------|-------|
| Marketplace integration | None in OpenSim core (third-party) | MISSING | N/A | Not part of OpenSim itself. Skip. |
| FreeSWITCH voice | Vivox + FreeSWITCH backends | WebRTC/wolfvoice only | L | Welkin chose WebRTC. FreeSWITCH is legacy. |
| Dynamic plugin loading | IRegionModuleBase with runtime discovery via Mono.Addins | All modules statically compiled | L | Proposal exists for post-release. Static compilation is a valid Rust design choice. |
| Additional DB backends | MSSQL (removed in 0.9.x), PostgreSQL via Npgsql | All three backends present | N/A | Already at parity. |

---

## Work Estimation

| Priority | Items | LSL Functions | OSSL Functions | Feature Gaps | Estimated Effort |
|----------|-------|---------------|----------------|--------------|-----------------|
| P0 | ~157 LSL + 3 features | 157 | 0 | 3 (baking, attachments, EEP) | L-XL |
| P1 | ~204 LSL + 155 OSSL stubs + features | 204 | 155 | 8 (materials, search, media, groups DB, dynamic textures, parcel media, MOAP, map search) | XL |
| P2 | ~43 LSL + features | 43 | 0 | 11 features | L |
| P3 | N/A | 0 | 0 | 3 features | Skip or defer |
| **Total** | | **361 LSL** | **155 OSSL** | **25 features** | |

The single largest piece of work is the 361 missing LSL VM builtins. About 70 of those
are pure math/string/list functions that need no server state (S effort each, high
parallelism potential). Another ~90 need scene graph or region state queries (M effort).
The rest need packet construction, service calls, or new subsystems (M-L effort).

The 155 OSSL stubs are a separate but parallel workstream. Most need to wire existing
Welkin subsystems (terrain, NPC, inventory, agent presence) to the OSSL function signatures.

The missing viewer-facing features (materials/PBR, EEP, MOAP, baking, search) are the
hardest items because they require new subsystems, not just function implementations.
