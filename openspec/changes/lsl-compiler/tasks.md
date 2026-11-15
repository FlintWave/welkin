# lsl-compiler tasks

## Phase 1: LSL Parser (P0 — nothing works without this)

- [x] Implement LSL lexer/tokenizer — all tokens: keywords (integer, float, string, key, vector, rotation, list, default, state, if, else, for, while, do, jump, return, event names), operators (+, -, *, /, %, =, +=, -=, *=, /=, %=, ==, !=, <, >, <=, >=, &&, ||, !, ~, &, |, ^, <<, >>), literals (integer, hex, float, string with escapes, key), punctuation ({, }, (, ), ;, ,, @, .)
- [x] Define LSL AST types — Script (globals + states), GlobalVar, GlobalFunc, State (name + events), Event (name + params + body), Statement (if/else, for, while, do-while, jump, label, return, expression, declaration, state-change), Expression (binary, unary, call, member access, cast, list literal, vector literal, rotation literal, assignment, ternary-style type cast)
- [x] Implement recursive descent parser — top-level: global variables, global functions, states (default required)
- [x] Parse all expression types — precedence climbing for operators, function calls, member access (.x, .y, .z, .s), type casts ((integer)x, (string)v, (vector)"<1,2,3>"), list constructor [a, b, c], vector/rotation constructors <x, y, z>
- [x] Parse all statement types — if/else with dangling-else resolution, for/while/do-while, jump/label (@label, jump label), return with optional value, variable declarations with optional initializer, state change (state name)
- [x] Parse all LSL type declarations — integer, float, string, key, vector, rotation, list as parameter types and variable types; void return for events
- [x] Implement error recovery — report errors with line:column spans, continue parsing after errors to report multiple issues, suggest fixes for common mistakes
- [x] Add parser tests — parse all 36 event signatures, all operator precedence levels, all type combinations, all control flow patterns, nested expressions

## Phase 2: Semantic Analysis + Transpiler (P0)

- [x] Implement scope resolution — global scope (variables + functions), per-function/event local scope, block scoping for for-loop variables, no nested function definitions in LSL
- [x] Implement type checking — verify operand types for all operators, check function call argument counts and types, verify event parameter signatures match LSL spec, check assignment type compatibility
- [x] Implement implicit cast insertion — insert conversion nodes in AST per LSL cast rules (integer→float, string↔key, all types→string in string context)
- [x] Implement constant folding — evaluate constant integer/float expressions at compile time, fold constant string concatenation, resolve constant vector/rotation literals
- [x] Implement Luau code generator — emit state dispatch table, emit event handler functions, emit global variables with default initializers, emit global function definitions
- [x] Generate type coercion runtime helpers — __int2float, __float2int, __vec2str, __rot2str, __str2key, __key2str, __vec_add, __vec_sub, __vec_mul, __vec_cross, __rot_mul, __vec_rot (vector * rotation), register all in Luau environment
- [x] Generate vector/rotation operations — vector +, -, *, /, %, ==, != with component-wise semantics; rotation *, == between rotations; vector * rotation for rotation application; vector . (dot product via __vec_dot); vector % (cross product via __vec_cross)
- [x] Generate list operations — list + list (concatenation), list + value (append), list == list (length comparison per LSL spec), llList2Integer/Float/String/Key/Vector/Rotation as builtins, llListSort, llListFindList, llList2ListStrided
- [x] Generate string operations — string + with type coercion (integer/float/vector/rotation auto-convert), comparison operators on strings (lexicographic)
- [x] Generate control flow — if/else, for/while/do-while as Luau equivalents, jump/label as goto (Luau supports goto), state change as current_state assignment + state_exit + state_entry firing
- [x] Add transpiler tests — compile LSL corpus from OpenSim test scripts, verify generated Luau executes correctly, test all implicit cast paths, test state transitions

## Phase 3: Region Integration (P0)

- [ ] Wire ScriptScheduler.tick() into region main loop — call per frame with instruction budget, respect scripts_enabled config flag
- [ ] Implement script compile on rez — when RezObject places object with script inventory items, fetch script asset, compile LSL→Luau→bytecode, create ScriptInstance, register with scheduler
- [ ] Implement script compile on edit — UpdateScriptAgent cap receives new source, compile, hot-swap ScriptInstance in scheduler
- [ ] Wire touch events — ObjectGrab→touch_start, ObjectGrabUpdate→touch, ObjectDeGrab→touch_end with detected face/position/key
- [ ] Wire chat events — ChatFromViewer→listen event dispatch via channel matching (llListen registrations), implement llListen/llListenRemove/llListenControl
- [ ] Wire collision events — physics engine collision callbacks → collision_start/collision/collision_end with detected key/name/position
- [ ] Wire timer events — llSetTimerEvent registers with ScriptScheduler timer heap (already exists), fire timer() event
- [ ] Wire changed events — inventory change, region change, link change, owner change → changed() event with change flags
- [ ] Wire money event — MoneyModule.OnObjectPaid → money() event on target object's scripts
- [ ] Wire link_message — llMessageLinked → link_message() event dispatch to target prims in link set
- [ ] Wire sensor events — llSensor/llSensorRepeat → sensor()/no_sensor() with detected object list, implement llDetectedName/Key/Pos/Rot/Vel/Type/Owner/Group
- [ ] Wire HTTP events — llHTTPRequest → http_response() event with status/body, implement HTTP request cap for inbound http_request() events
- [ ] Implement script state persistence — serialize ScriptInstance state (globals + current_state + listen registrations + timers) for derez/region-crossing, restore on rez
- [ ] Implement script reset — llResetScript and llResetOtherScript clear state and fire state_entry, viewer "Reset Scripts" button triggers reset for all scripts in object

## Phase 4: Builtin Completion (P1)

### Communication (highest priority — most scripts use these)
- [ ] Implement llSay, llShout, llWhisper, llRegionSay, llRegionSayTo — chat on channels with range limits (20m/100m/region)
- [ ] Implement llInstantMessage — send IM to avatar UUID via IM module
- [ ] Implement llEmail, llGetNextEmail — SMTP send via lettre, email queue polling
- [ ] Implement llHTTPRequest, llSetHTTPBody — outbound HTTP with throttling and URL filtering
- [ ] Implement llRequestURL, llRequestSecureURL, llReleaseURL — inbound HTTP for scripts (register cap URL, route requests to script)

### Object/Prim (most-used after communication)
- [ ] Implement remaining llSet* — llSetPos, llSetRot, llSetScale, llSetColor, llSetAlpha, llSetTexture, llSetPrimitiveParams (the big one — all BUILD_* constants from OpenSim)
- [ ] Implement remaining llGet* — llGetPos, llGetRot, llGetScale, llGetColor, llGetAlpha, llGetTexture, llGetPrimitiveParams, llGetObjectDetails, llGetObjectDesc, llGetObjectName
- [ ] Implement llRezObject, llRezAtRoot — rez from script inventory at position with velocity and start param
- [ ] Implement llDie — delete the object containing the script
- [ ] Implement llSetLinkPrimitiveParams, llSetLinkPrimitiveParamsFast — modify linked prims
- [ ] Implement llGetLinkKey, llGetLinkName, llGetLinkNumber, llGetNumberOfPrims — link set queries
- [ ] Implement llAttachToAvatar, llDetachFromAvatar, llAttachToAvatarTemp — attachment control

### Movement/Physics
- [ ] Implement llSetForce, llSetTorque, llApplyImpulse, llApplyRotationalImpulse — physics forces via Rapier
- [ ] Implement llSetBuoyancy, llSetHoverHeight — physics properties
- [ ] Implement llSetVehicleType and all llSetVehicle* — vehicle dynamics (TYPE_CAR, TYPE_BOAT, TYPE_AIRPLANE, TYPE_BALLOON, TYPE_SLED, 40+ VEHICLE_* constants from OpenSim)
- [ ] Implement llMoveToTarget, llStopMoveToTarget — PID-controlled movement
- [ ] Implement llTarget, llTargetRemove — position target events (at_target/not_at_target)
- [ ] Implement llRotTarget, llRotTargetRemove — rotation target events

### Avatar
- [ ] Implement llRequestAgentData — query agent info (online status, birth date, payment info) via ROBUST
- [ ] Implement llGetAgentInfo, llGetAgentSize, llGetAgentLanguage — avatar properties
- [ ] Implement llRequestPermissions, llGetPermissions — runtime permission system (PERMISSION_DEBIT, PERMISSION_TRIGGER_ANIMATION, PERMISSION_ATTACH, PERMISSION_TELEPORT, etc.)
- [ ] Implement llStartAnimation, llStopAnimation — play animations on avatar (requires permission)
- [ ] Implement llTeleportAgent, llTeleportAgentHome — teleport with permission check
- [ ] Implement llSitTarget, llUnSit, llAvatarOnSitTarget, llAvatarOnLinkSitTarget — sit targeting

### Inventory/Notecard
- [ ] Implement llGetInventoryName, llGetInventoryNumber, llGetInventoryType, llGetInventoryKey — query task inventory
- [ ] Implement llGiveInventory, llGiveInventoryList — give items to avatars/objects
- [ ] Implement llGetNotecardLine, llGetNumberOfNotecardLines — async notecard reading (dataserver event)
- [ ] Implement llRemoveInventory, llAllowInventoryDrop — inventory management

### Math/Utility (many already done, fill gaps)
- [ ] Implement remaining math — llAngleBetween, llAxes2Rot, llRot2Angle, llRot2Axis, llRot2Euler, llEuler2Rot, llRotBetween (rotation math), llListStatistics (stats on numeric lists)
- [ ] Implement llGetUnixTime, llGetTimestamp, llGetGMTclock, llGetWallclock — time functions
- [ ] Implement llGenerateKey — generate new UUID
- [ ] Implement llMD5String, llSHA1String, llSHA256String — crypto hashes
- [ ] Implement llBase64ToString, llStringToBase64, llXorBase64, llIntegerToBase64 — encoding

### Land/Parcel
- [ ] Implement llGetParcelDetails, llGetParcelFlags, llGetParcelMaxPrims, llGetParcelPrimCount — parcel queries
- [ ] Implement llOverMyLand, llGetLandOwnerAt — land ownership checks
- [ ] Implement llSetParcelMusicURL, llSetParcelMediaURL — parcel media control
