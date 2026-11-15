<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Script engine

[← Back to docs](README.md) · [Architecture](architecture.md) · [Integration architecture](integration-architecture.md)

Welkin runs LSL (Linden Scripting Language) scripts by compiling them to Luau
bytecode and executing them in a sandboxed VM. This is different from
OpenSimulator's approach, which uses XEngine (interpreted) or YEngine (Mono/CIL
compiled). Welkin has no Mono dependency.

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

When a scripted object is rezzed, the region compiles its LSL source to Luau
bytecode and creates a `ScriptInstance`. The bytecode is executed once to
register all handler functions as Luau globals, then a `state_entry` event is
queued.

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

Each VM can have a memory cap set at creation. Scripts that exceed the limit
receive a `MemoryError` from Luau. The limit applies to both direct execution
and coroutine threads.

## Script lifecycle

1. **Rez:** object enters the scene, region creates a `ScriptInstance` from
   the LSL source
2. **Compile:** LSL source is compiled to Luau bytecode, bytecode is loaded
   and executed to register handlers
3. **state_entry:** the scheduler queues and dispatches a `StateEntry` event
4. **Running:** the scheduler delivers events (touch, timer, listen, collision,
   etc.) as they arrive, one per tick
5. **Reset:** `reset()` creates a fresh VM, reloads bytecode and queues a
   new `state_entry`
6. **Derez:** script state is serialized (bytecode + tracked globals as JSON)
   into a `ScriptState` for persistence
7. **Region transfer:** `RegionTransferBundle` captures script states, pending
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

## Built-in functions

### ll* functions (LSL API)

83 `ll*` functions are registered across six modules:

| Module | Count | Examples |
|---|---|---|
| math | 22 | llAbs, llSin, llCos, llSqrt, llPow, llVecDist, llVecNorm, llRot2Euler |
| string | varies | llToUpper, llToLower, llStringLength, llSubStringIndex, llGetSubString |
| list | 15 | llGetListLength, llList2Integer, llList2String, llListSort, llListFindList, llListStatistics |
| communication | varies | llSay, llWhisper, llShout, llListen, llRegionSay |
| object | varies | llGetObjectName, llGetObjectDesc, llGetPos, llGetRot, llGetScale, llGetOwner |
| physics | varies | llApplyImpulse, llSetForce, llGetVel, llGetAccel |

Pure-math and string functions are fully implemented. Communication, physics and
object functions that need region integration log the call and return sensible
defaults until the wiring is complete.

### OSSL functions

232 `os*` functions are registered across 13 modules: agent, crypto, draw,
inventory, math, misc, npc, sim_info, string, stubs, terrain, and time.
These provide the OpenSimulator-specific scripting extensions.

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
