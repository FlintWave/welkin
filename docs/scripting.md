<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Script engine

[← Back to docs](index.md) · [Architecture](architecture.md) · [Integration architecture](integration-architecture.md)

Welkin runs LSL (Linden Scripting Language) scripts by compiling them to Luau
bytecode and executing them in a sandboxed VM. OpenSimulator uses XEngine
(interpreted) or YEngine (Mono/CIL compiled); Welkin replaces both with a
Luau-based runtime and has no Mono dependency.

## Compilation pipeline

LSL source goes through four stages before it runs:

```
LSL source → lexer → parser → AST
                                 ↓
                            analyzer (scope, types, implicit casts, constant folding)
                                 ↓
                            codegen → Luau source
                                 ↓
                            mlua Compiler → bytecode
```

The `welkin-script-compiler` crate implements the first four stages. The
`compile_lsl()` function runs the full pipeline in one call: lex, parse,
analyze, generate Luau source. The `welkin-script-vm` crate then compiles that
Luau to bytecode via `mlua::Compiler` and loads it into a sandboxed Luau VM.

The lexer handles all LSL tokens including type keywords (integer, float,
string, key, vector, rotation, list), control flow (if/else, for, while, do,
jump, return), states and events. The parser builds a typed AST with global
variables, user-defined functions, states and event handlers. The analyzer
resolves scopes, checks types, inserts implicit casts (integer to float, string
to key) and folds constants. The code generator emits Luau source with a
`__dispatch` function and a `__states` table that routes events to the correct
state's handler.

## Supported LSL features

**Types:** integer (i32), float (f32), string, key (UUID string), vector
(x/y/z), rotation (x/y/z/s quaternion), list (heterogeneous).

**Control flow:** if/else, for, while, do-while, jump (labels), return.

**Operators:** arithmetic (+, -, \*, /, %), comparison (==, !=, <, >, <=, >=),
logical (&&, ||, !), bitwise (&, |, ^, ~, <<, >>), assignment (=, +=, -=, \*=,
/=, %=), increment/decrement (++, --), type casts.

**States:** the `default` state plus named states via `state name { ... }`.
State transitions emit `state_exit` then `state_entry`.

**Events:** the parser recognizes 36 event types:

attach, at_rot_target, at_target, changed, collision, collision_start,
collision_end, control, dataserver, email, experience_permissions,
experience_permissions_denied, http_request, http_response,
land_collision, land_collision_start, land_collision_end, link_message,
linkset_data, listen, money, moving_end, moving_start, no_sensor,
not_at_rot_target, not_at_target, object_rez, on_rez, path_update,
remote_data, run_time_permissions, sensor, state_entry, state_exit,
timer, touch, touch_start, touch_end, transaction_result

## How scripts run

### Compile-on-rez

When a scripted object is rezzed (or a script is uploaded via the
UpdateScriptAgent cap), the region compiles its LSL source to Luau bytecode
and creates a `ScriptInstance`. The region's heartbeat loop constructs a
`ScriptContext` for the instance, wiring it to channel senders for chat,
timer, sensor, control and link-message events. `register_builtins()` calls
`builtins::register_all()`, which registers all 206 ll\* functions, 236 os\*
functions, and extension APIs (LightShare, MOD, JsonStore) as Luau globals
on the VM. The bytecode is then executed once to register the script's own
handler functions, and a `state_entry` event is queued.

The full chain from upload to execution:

```
LSL source → compile_lsl() → Luau source → mlua::Compiler → bytecode
    → ScriptInstance::new(bytecode)
    → register_builtins(ScriptContext { chat_sender, timer_sender, ... })
    → scheduler.add_script(instance)
    → scheduler.post_event(StateEntry)
    → __dispatch routes to default.state_entry()
    → llSay(0, "hello") → chat_sender → heartbeat loop → relay to clients
```

### Event dispatch

Each `ScriptInstance` has an inbound event queue (FIFO). The `ScriptScheduler`
drives execution by calling `run_next_event()` on each instance in round-robin
order, one event per tick per script.

For compiled LSL scripts, event dispatch goes through the generated `__dispatch`
function, which routes to the correct handler in the current state's table. For
raw Luau scripts (used in tests), dispatch falls back to a direct global
function lookup.

Events carry typed arguments. Touch events provide an agent UUID. Listen events
carry channel, speaker name, speaker UUID and message. Link messages carry
sender link number, integer, string and key. HTTP events carry request ID,
status, metadata and body. The `Custom` variant accepts arbitrary JSON arguments
for extensibility.

### Detected parameters

Touch, collision and sensor events populate a `DetectedInfo` list accessible
through the `llDetected*` family of functions: llDetectedKey, llDetectedName,
llDetectedPos, llDetectedRot, llDetectedVel, llDetectedOwner, llDetectedGroup,
llDetectedType, llDetectedLinkNumber, llDetectedTouchFace, llDetectedTouchPos,
llDetectedTouchNormal, llDetectedTouchBinormal, llDetectedTouchST,
llDetectedTouchUV. Out-of-range indices return sensible defaults (nil UUID,
zero vector, -1 for touch face) matching C# OpenSimulator behavior.

### Instruction limits

The scheduler enforces an instruction budget per event dispatch via the Luau
VM's interrupt callback. Each script gets `max_instructions_per_slice` interrupt
ticks before the VM raises an error. This prevents infinite loops from blocking
the region. An interrupted script enters the `Error` state but other scripts
on the same VM continue running.

### Memory limits

The VM memory limit is set to 0 (unlimited) at creation to accommodate the
100+ registered builtins. Scripts that allocate excessively are caught by the
instruction limit instead. If a per-script memory cap is restored later, it
applies to both direct execution and coroutine threads.

## Script lifecycle

1. **Rez/Upload:** object enters the scene or script is uploaded via the
   UpdateScriptAgent cap. Viewers use a two-step protocol: POST LLSD
   metadata to get a one-shot uploader URL, then POST raw LSL source to
   that URL. The optional `prim_id` query param associates the script
   with a specific prim. Direct raw-text POST works for bot tests
2. **Compile:** LSL source is compiled to Luau bytecode, bytecode is loaded
   into a new `ScriptInstance`
3. **Register builtins:** the heartbeat loop builds a `ScriptContext` with
   channel senders and calls `register_builtins()`. The script's position
   is read from the scene so spatial functions (llSay range, sensor sweeps)
   use the prim's actual coordinates
4. **state_entry:** the scheduler queues and dispatches a `StateEntry` event
5. **Running:** the scheduler delivers events (touch, timer, listen, collision,
   etc.) as they arrive, one per tick
6. **Reset:** `reset()` creates a fresh VM, reloads bytecode, re-registers
   builtins from the stored `ScriptContext`, and queues a new `state_entry`
7. **Derez:** script state is serialized (bytecode + tracked globals as JSON)
   into a `ScriptState` for persistence
8. **Region transfer:** `RegionTransferBundle` captures script states, pending
   events and timer intervals for migration to another region. The source
   region removes the scripts; the destination restores them and resumes
   event processing

### State serialization

The Luau VM does not support coroutine state serialization (no way to snapshot
stack frames, locals or instruction pointer). Welkin saves what it can: compiled
bytecode and a JSON map of tracked global variables. On restore, a fresh VM is
created, globals are injected, and the bytecode is re-executed from the top.
Scripts that need mid-function resume use a checkpoint pattern (a `_checkpoint`
global that the script checks on entry).

Restored scripts get the full set of ll* and os* functions and no absolute
memory cap. An earlier version restored them without the functions, and the
restore could fail with "not enough memory".

## Built-in functions

### ll* functions (LSL API)

206 `ll*` functions are registered across 18 modules. All are wired to region
services and produce real results; there are no log-only stubs.

| Module | Count | Examples |
|---|---|---|
| math | 38 | llAbs, llSin, llCos, llSqrt, llPow, llVecDist, llVecNorm, llRot2Euler, llAtan2, llLog10 |
| list | 27 | llGetListLength, llList2Integer, llList2String, llListSort, llListFindList, llListStatistics, llJson2List |
| object | 26 | llGetObjectName, llGetObjectDesc, llGetPos, llGetRot, llGetScale, llGetOwner, llRezObject |
| string | 26 | llToUpper, llToLower, llStringLength, llSubStringIndex, llGetSubString, llHash, llSHA256String |
| physics | 19 | llApplyImpulse, llSetForce, llGetVel, llGetAccel, llTarget, llRotTarget |
| avatar | 16 | llAttachToAvatar, llDetachFromAvatar, llAttachToAvatarTemp, llRequestPermissions, llRequestAgentData |
| communication | 11 | llSay, llWhisper, llShout, llListen, llRegionSay, llInstantMessage |
| inventory | 10 | llGetNotecardLine, llGetNumberOfNotecardLines, llGiveInventory, llRemoveInventory |
| land | 8 | llGetParcelFlags, llOverMyLand, llGetParcelDetails, llSetParcelMusicURL |
| vehicle | 6 | llSetVehicleType, llSetVehicleFloatParam, llSetVehicleVectorParam |
| prim_params | 4 | llSetPrimitiveParams, llGetPrimitiveParams, llSetLinkPrimitiveParams |
| http | 3 | llHTTPRequest, llHTTPResponse, llSetContentType |
| cap_url | 3 | llRequestURL, llRequestSecureURL, llReleaseURL |
| timer | 3 | llSetTimerEvent, llGetTime, llResetTime |
| sensor | 3 | llSensor, llSensorRepeat, llSensorRemove |
| email | 2 | llEmail, llGetNextEmail |
| control | 2 | llTakeControls, llReleaseControls |

Builtins are registered on the VM at script load time via
`builtins::register_all()`. Each closure captures a shared `ScriptContext`
(`Arc<RwLock<ScriptContext>>`) that holds channel senders for chat, timer,
sensor, control and link-message events. When a script calls `llSay(0, msg)`,
the closure sends a `ChatMessage` through the chat channel; the heartbeat
loop picks it up, routes it to listen events on other scripts, and relays
it to connected viewers.

### OSSL functions

236 `os*` functions are registered across 13 modules: agent, crypto, draw,
inventory, math, misc, npc, sim_info, string, stubs, terrain, and time.
These provide the OpenSimulator-specific scripting extensions. 20 NPC
functions have real implementations (osNpcCreate, osNpcMoveTo, osNpcSay,
etc.) and terrain, sun, wind and speed functions read and write live region
state.

### Extension APIs

Three extension modules are registered alongside the LSL and OSSL builtins:

| Module | Functions | Purpose |
|---|---|---|
| LightShare | 3 | lsGetWindlightScene, lsSetWindlightScene, lsSetWindlightSceneTargeted |
| MOD API | 9 | modInvokeI/F/S/K/V/R/L/N, modSendCommand |
| JsonStore | 14 | JsonCreateStore, JsonGetValue, JsonSetValue, JsonTestStore, etc. |

LightShare controls per-parcel Windlight sky and water settings. The MOD API
lets scripts invoke registered C#-style module commands by type. JsonStore
provides an in-memory JSON document store for cross-script data sharing.

### Detected functions

16 `llDetected*` functions (see "Detected parameters" above) are registered
separately via the event system and share an `EventContext` through
`Arc<RwLock<..>>`.

## Crate layout

| Crate | Role |
|---|---|
| `welkin-script-compiler` | Lexer, parser, AST, semantic analyzer, Luau code generator |
| `welkin-script-vm` | Luau VM wrapper (mlua), ScriptInstance, ScriptScheduler, builtins, OSSL, events, region transfer |
| `welkin-script-api` | Shared type definitions for the script API |

## ll\* function reference

Every function listed below is registered on the Luau VM and callable from
LSL scripts. Grouped by module.

### Math (38)

llAbs, llAcos, llAngleBetween, llAsin, llAtan2, llAxes2Rot, llAxisAngle2Rot,
llCeil, llCos, llEuler2Rot, llFabs, llFloor, llFrand, llGenerateKey,
llGetGMTclock, llGetTimestamp, llGetUnixTime, llGetWallclock, llLinear2sRGB,
llListStatistics, llLog, llLog10, llModPow, llPow, llRot2Angle, llRot2Axis,
llRot2Euler, llRot2Fwd, llRot2Left, llRot2Up, llRotBetween, llRound, llSin,
llSqrt, llTan, llVecDist, llVecMag, llVecNorm

Scalar arithmetic, trigonometry, vector/rotation math, random numbers, time
queries and UUID generation. All pure computation with no side effects.

### List (27)

llCSV2List, llDeleteSubList, llGetListEntryType,
llGetListLength, llJson2List, llJsonGetValue, llJsonSetValue, llJsonValueType,
llList2CSV, llList2Float, llList2Integer, llList2Json, llList2Key, llList2List,
llList2ListSlice, llList2ListStrided, llList2Rot, llList2String, llList2Vector,
llListFindList, llListFindStrided, llListInsertList, llListRandomize,
llListReplaceList, llListSort, llListSortStrided, llListStatistics

List manipulation, type conversion, searching, sorting, statistics and JSON
interop.

### Object (26)

llDie, llGetAlpha, llGetColor, llGetCreator, llGetKey, llGetLinkKey,
llGetLinkNumber, llGetNumberOfPrims, llGetObjectDesc, llGetObjectDetails,
llGetObjectName, llGetOwner, llGetPos, llGetRot, llGetScale, llGetTexture,
llRezAtRoot, llRezObject, llSetAlpha, llSetColor, llSetObjectDesc,
llSetObjectName, llSetPos, llSetRot, llSetScale, llSetTexture

Object properties, position, rotation, scale, textures, color, alpha, rez and
die. Reads and writes go through the scene graph.

### String (26)

llBase64ToInteger, llBase64ToString, llChar, llDeleteSubString,
llDumpList2String, llEscapeURL, llGetSubString, llHash, llInsertString,
llIntegerToBase64, llMD5String, llOrd, llParseString2List,
llParseStringKeepNulls, llReplaceSubString, llSHA1String, llSHA256String,
llStringLength, llStringToBase64, llStringTrim, llSubStringIndex, llToLower,
llToUpper, llUnescapeURL, llXorBase64, llXorBase64StringsCorrect

String manipulation, encoding, hashing and parsing.

### Physics (19)

llApplyImpulse, llApplyRotationalImpulse, llGetAccel, llGetOmega, llGetVel,
llGround, llGroundNormal, llMoveToTarget, llRotTarget, llRotTargetRemove,
llSetBuoyancy, llSetForce, llSetHoverHeight, llSetStatus, llSetTorque,
llStopMoveToTarget, llTarget, llTargetRemove, llWater

Forces, impulses, velocity, hover, move-to-target, position/rotation targets
and terrain height queries.

llApplyImpulse, llApplyRotationalImpulse, llSetForce, llSetTorque,
llSetBuoyancy, llMoveToTarget and llStopMoveToTarget act on the physics body of
the object that owns the script, including when the script sits in a child
prim. llSetStatus handles STATUS_PHANTOM. Changing STATUS_PHYSICS at runtime,
any other STATUS_* flag, and llSetHoverHeight are not supported by the physics
backend yet; the region logs the reason instead of dropping the command
silently.

### Avatar (16)

llAttachToAvatar, llAttachToAvatarTemp, llAvatarOnSitTarget,
llDetachFromAvatar, llGetAgentInfo, llGetAgentSize, llGetPermissions,
llGetPermissionsKey, llRequestAgentData, llRequestPermissions, llSitTarget,
llStartAnimation, llStopAnimation, llTeleportAgent, llTeleportAgentHome,
llUnSit

Attachments, animations, permissions, sit targets, teleport and agent data
queries via dataserver events.

### Communication (11)

llInstantMessage, llListen, llListenControl, llListenRemove,
llMessageLinked, llOwnerSay, llRegionSay, llRegionSayTo, llSay, llShout,
llWhisper

Chat (say/shout/whisper on numbered channels), instant messages, listen
filters and link messages between scripts in the same linkset.

### Inventory (10)

llAllowInventoryDrop, llGetInventoryKey, llGetInventoryName,
llGetInventoryNumber, llGetInventoryType, llGetNotecardLine,
llGetNumberOfNotecardLines, llGiveInventory, llGiveInventoryList,
llRemoveInventory

Prim inventory queries, notecard reading via dataserver events, giving items
to avatars and removing items.

### Land (8)

llGetLandOwnerAt, llGetParcelDetails, llGetParcelFlags, llGetParcelMaxPrims,
llGetParcelPrimCount, llOverMyLand, llSetParcelMediaURL, llSetParcelMusicURL

Parcel queries, owner checks and parcel media/music URL settings.

### Vehicle (6)

llRemoveVehicleFlags, llSetVehicleFlags, llSetVehicleFloatParam,
llSetVehicleRotationParam, llSetVehicleType, llSetVehicleVectorParam

Vehicle dynamics with 46 VEHICLE_\* constants and type presets (car, boat,
airplane, balloon, sled).

### Prim params (4)

llGetPrimitiveParams, llSetLinkPrimitiveParams, llSetLinkPrimitiveParamsFast,
llSetPrimitiveParams

Batch get/set of prim properties using 90+ PRIM_\* constants (shape, texture,
color, glow, flex, light, sculpt).

### HTTP (3)

llHTTPRequest, llHTTPResponse, llSetContentType

Outbound HTTP requests with a 25-pending throttle, inbound response handling
and content type headers for URL caps.

### Cap URL (3)

llReleaseURL, llRequestSecureURL, llRequestURL

Script-owned HTTP endpoints (caps) for inbound requests from external
services.

### Timer (3)

llGetTime, llResetTime, llSetTimerEvent

Script-local elapsed time and periodic timer events.

### Sensor (3)

llSensor, llSensorRemove, llSensorRepeat

One-shot and repeating spatial scans for agents and objects, firing sensor/
no_sensor events.

### Email (2)

llEmail, llGetNextEmail

Send email (20-second rate limit) and receive queued email.

### Control (2)

llReleaseControls, llTakeControls

Request and release keyboard/mouse control events from the avatar.

### Sound

llPlaySound, llLoopSound, llStopSound, llTriggerSound, llTriggerSoundLimited,
llPreloadSound and llAdjustSoundVolume send the same viewer messages as
OpenSimulator. The region relays them from `sound_io.rs`.

- llPlaySound sends an AttachedSound message to every client, with flags 0, or
  QUEUE (0x10) when the script has called llSetSoundQueueing(TRUE).
- llTriggerSound sends a SoundTrigger message only to avatars within the
  prim's sound radius. A radius of 0 means 100 m, and the prim keeps 100 m
  from then on. llTriggerSoundLimited scales the volume down linearly with
  distance, to silence at 100 m.
- llPreloadSound reaches avatars within twice the sound radius, with the same
  0 to 100 m default.
- llLoopSound and llStopSound change the root prim's sound fields and then
  broadcast a full object update, so avatars who arrive later still hear the
  loop. Loop flags are LOOP (0x01), MASTER (0x02) and SLAVE (0x04), plus QUEUE
  (0x10) when queueing is on. llStopSound sets STOP (0x20).
- Scripts in child prims drive the one-shot and range sounds (play, trigger,
  preload) through the prim's linkset root. llLoopSound and llStopSound on a
  child prim are not supported yet: the region logs a warning and ignores the
  command, so put looping sounds in the root prim.
- A collision sound chosen with llCollisionSound plays when a collision
  starts. The region allows one such sound per object every 200 ms and skips
  collisions at a speed of 0.2 m/s or less. llCollisionSound with an empty
  string disables the sound, as in OpenSimulator.

### Constants

The VM defines the LSL constants as globals at runtime, because the compiler
emits their names as they are. They include TRUE, FALSE, PI, NULL_KEY,
ZERO_VECTOR, and the STATUS_*, CHANGED_*, PSYS_* and CAMERA_* families. The
values come from the script API constants table, plus a generated table taken
from OpenSimulator's `LSL_Constants.cs` for names the first table lacks
(particles, camera, media, NPC and vectors).

## Differences from OpenSimulator

| | OpenSimulator | Welkin |
|---|---|---|
| VM | Mono/.NET (XEngine) or CIL (YEngine) | Luau (via mlua/Rust) |
| Compilation | LSL → C# → CIL bytecode | LSL → Luau source → Luau bytecode |
| Sandboxing | AppDomain isolation | Luau sandbox (frozen globals) + memory limits + instruction interrupts |
| State save | Full CIL thread state | Bytecode + globals JSON (no coroutine state) |
| Coroutine state | Serializable via Mono | Not serializable (Luau limitation); checkpoint pattern instead |
| OSSL | C# methods in the same runtime | Rust closures registered as Luau globals |
| Dependencies | Mono runtime, libgdiplus | mlua (statically linked Luau), no external runtime |
