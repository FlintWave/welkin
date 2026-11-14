# LSL Corpus Analysis (Spike Task 1.3 + 1.4)

Analysed 18 LSL scripts (780 lines) from
`opensim-0.9.3.0-source/bin/assets/ScriptsAssetSet/`.

## Script inventory

| # | File | Lines | Description |
|---|------|------:|-------------|
| 1 | DefaultSay.lsl | 7 | Minimal: `state_entry` calls `llSay` |
| 2 | DefaultSayAndTouch.lsl | 13 | `state_entry` + `touch_start` events |
| 3 | DefaultScript.lsl | 7 | Identical to DefaultSay (hello-world) |
| 4 | GrafittiBoard.lsl | 74 | Dynamic-texture graffiti board with user-defined functions, `listen` event, string manipulation, OSSL call `osSetDynamicTextureData` |
| 5 | llAddToLandBanList.lsl | 84 | Land-management script: chat commands, sensor sweep, parcel ban/pass list |
| 6 | llAddToLandPassList.lsl | 84 | Identical to #5 (duplicate asset) |
| 7 | llAdjustSoundVolume.lsl | 13 | Listen for chat, cast to float, adjust sound |
| 8 | llAllowInventoryDrop.lsl | 17 | Toggle inventory-drop permission on touch; `changed` event with bitwise test |
| 9 | llAngleBetween.lsl | 11 | Rotation maths: `llAngleBetween`, `llGetRot`, `ZERO_ROTATION` |
| 10 | llApplyImpulse.lsl | 16 | Physics: `llGetObjectDetails`, `llSetStatus`, `llVecNorm`, `llApplyImpulse` |
| 11 | llAvatarOnSitTarget.lsl | 20 | Sit-target detection, `changed` event, key-as-boolean test |
| 12 | llRemoveFromLandBanList.lsl | 84 | Land management (same script as #5) |
| 13 | llRemoveFromLandPassList.lsl | 84 | Land management (same script as #5) |
| 14 | llResetLandBanList.lsl | 84 | Land management (same script as #5) |
| 15 | llResetLandPassList.lsl | 84 | Land management (same script as #5) |
| 16 | llSetParcelMusicURL.lsl | 7 | Single call to `llSetParcelMusicURL` |
| 17 | osTextBoard.lsl | 48 | OSSL dynamic-texture text board with user-defined functions |
| 18 | osWeatherMap.lsl | 43 | Rotating weather-map URLs via timer, OSSL `osSetDynamicTextureURL` |

Note: scripts 5, 6, 12-15 are copies of the same land-management script.
The **unique** scripts number 10; the corpus is small but covers the core LSL
patterns found in real-world OpenSimulator content.

## LSL language features inventory

### Types used

| LSL type | Luau equivalent | Notes |
|----------|----------------|-------|
| `integer` | `number` (i32 semantics via runtime) | 32-bit signed; Luau has only f64 numbers |
| `float` | `number` | Direct map |
| `string` | `string` | Direct map |
| `key` | `string` (tagged) | UUID string; LSL treats it as a distinct type with truthiness = non-NULL_KEY |
| `vector` | userdata or table `{x,y,z}` | Literal syntax `<x,y,z>` needs transpiler support |
| `rotation` | userdata or table `{x,y,z,s}` | Literal syntax `<x,y,z,s>` |
| `list` | table (array) | Heterogeneous; LSL lists are value-typed and immutable |

### Events used

| Event | Occurrences | Parameters |
|-------|-------------|------------|
| `state_entry` | 18 (all scripts) | none |
| `touch_start` | 4 | `integer num_detected` |
| `listen` | 4 | `integer channel, string name, key id, string message` |
| `sensor` | 1 (x5 copies) | `integer num_detected` |
| `no_sensor` | 1 (x5 copies) | none |
| `changed` | 2 | `integer change` |
| `on_rez` | 1 (x5 copies) | `integer param` |
| `timer` | 1 | none |

YEngine supports 42 event types total (see `ScriptEventCode` enum).  The
corpus exercises only 8 distinct events, but the transpiler must handle all 42.

### Built-in functions used

**ll* (Linden Lab standard API):**

| Function | Category |
|----------|----------|
| `llSay` | Chat |
| `llOwnerSay` | Chat |
| `llListen` | Chat |
| `llSetText` | Prim properties |
| `llGetOwner` | Prim/agent |
| `llGetRot` | Prim properties |
| `llSitTarget` | Prim properties |
| `llAvatarOnSitTarget` | Agent |
| `llKey2Name` | Agent |
| `llAngleBetween` | Math/rotation |
| `llVecNorm` | Math/vector |
| `llGetObjectDetails` | Object queries |
| `llSetStatus` | Physics |
| `llApplyImpulse` | Physics |
| `llAdjustSoundVolume` | Sound |
| `llAllowInventoryDrop` | Inventory |
| `llSetParcelMusicURL` | Parcel |
| `llAddToLandBanList` | Parcel |
| `llRemoveFromLandBanList` | Parcel |
| `llAddToLandPassList` | Parcel |
| `llRemoveFromLandPassList` | Parcel |
| `llResetLandBanList` | Parcel |
| `llResetLandPassList` | Parcel |
| `llSensor` | Sensor |
| `llDetectedKey` | Detection |
| `llResetScript` | Script control |
| `llSetTimerEvent` | Timer |
| `llGetPos` | Position |
| `llStringLength` | String |
| `llGetSubString` | String |
| `llDeleteSubString` | String |
| `llParseString2List` | String/list |
| `llList2String` | List |
| `llList2Vector` | List |
| `llToLower` | String |

**os* (OSSL extensions):**

| Function | Category |
|----------|----------|
| `osSetDynamicTextureData` | Dynamic textures |
| `osSetDynamicTextureURL` | Dynamic textures |

### Control flow and language constructs

| Construct | Example | Transpiler complexity |
|-----------|---------|----------------------|
| `default { ... }` state block | All scripts | **Medium** -- map to Luau module structure |
| Event handlers inside states | All scripts | **Medium** -- become Luau functions registered by name |
| User-defined functions | `addGraffiti()`, `clearGraffiti()`, `draw()`, `push_text()`, `compile_text()`, `draw_text()`, `refresh_texture()` | **Trivial** -- direct map to Luau functions |
| `if` / `else if` / `else` | Land scripts, osWeatherMap | **Trivial** |
| `while` loop | GrafittiBoard | **Trivial** |
| `for` loop | Land scripts | **Trivial** -- `for (i=0; i<num; ++i)` maps to Luau `for` |
| C-style type casts | `(float)msg`, `(string)value` | **Medium** -- map to explicit conversion functions |
| `+=` string concatenation | GrafittiBoard, osTextBoard | **Trivial** |
| Bitwise `&` operator | llAllowInventoryDrop (`change & CHANGED_ALLOWED_DROP`) | **Trivial** -- Luau has `bit32` library |
| Boolean negation `!` | llAllowInventoryDrop (`!allow`) | **Trivial** |
| Vector literal `<x,y,z>` | GrafittiBoard (`<0.0, 1.0, 0.0>`) | **Medium** -- angle-bracket literals need parser |
| Constants | `NULL_KEY`, `ZERO_ROTATION`, `PI`, `AGENT`, `STATUS_PHYSICS`, `CHANGED_LINK`, `CHANGED_ALLOWED_DROP`, `OBJECT_POS` | **Easy** -- emit as Luau module constants |
| List literal `[a, b]` | `llGetObjectDetails(id, [OBJECT_POS])`, `llParseString2List(msg,[":"],[])` | **Medium** -- square-bracket list literals |
| Key-as-boolean | `if (av)` where av is `key` | **Medium** -- need truthiness wrapper: key is truthy iff non-null and non-zero UUID |
| Global variables at file scope | `string text = ""`, `integer count = 0`, etc. | **Medium** -- map to module-level locals |
| Implicit string concat with `+` | `"Hello " + llKey2Name(av)` | **Trivial** -- Luau `..` operator |
| Comments `//` and `/* */` | Throughout | **Trivial** |

## What an LSL-to-Luau transpiler must handle

### Trivial (mechanical mapping)

1. **Basic control flow**: `if`/`else`/`while`/`for`/`do-while`/`jump`/labels --
   Luau has direct equivalents for all of these.
2. **User-defined functions**: direct map to Luau `local function`.
3. **Comments**: strip or convert (both languages use `//` and `/* */`; Luau
   also supports `--`).
4. **Arithmetic and comparison operators**: identical semantics.
5. **String concatenation**: `+` in LSL becomes `..` in Luau.
6. **Constants**: emit a prelude table of all LSL/OSSL constants
   (`NULL_KEY`, `ZERO_ROTATION`, `PI`, `AGENT`, `STATUS_PHYSICS`, etc.).

### Medium complexity

7. **State machine**: LSL scripts are structured as named states (`default`,
   plus user-defined states), each containing event handlers.  The transpiler
   emits a Luau module that registers handler functions per state.  The runtime
   dispatches events to the current state's handler.

8. **Type system mapping**:
   - `integer` -> Luau number (runtime enforces i32 wrap semantics via
     `bit32.band(n, 0xFFFFFFFF)` or a cast helper).
   - `float` -> Luau number (native f64).
   - `string` -> Luau string.
   - `key` -> Luau string with a wrapper for truthiness (non-null UUID check).
   - `vector` -> Luau userdata or table; literal `<x,y,z>` must be parsed and
     emitted as a constructor call.
   - `rotation` -> Luau userdata or table; literal `<x,y,z,s>` likewise.
   - `list` -> Luau table (array part); LSL lists are heterogeneous and
     value-typed.

9. **Type casts**: LSL uses C-style `(type)expr`.  The transpiler converts
   these to explicit conversion calls (`toFloat(x)`, `toString(x)`, etc.).

10. **Angle-bracket literals**: `<0.0, 1.0, 0.0>` for vectors and rotations.
    The LSL parser must recognise these as constructors, not comparison
    operators.

11. **List literals**: `[a, b, c]` constructs an LSL list.  Map to Luau table
    constructors `{a, b, c}`.

12. **Global (file-scope) variables**: LSL globals live across event
    invocations within the same state.  Map to Luau upvalues or module-scope
    locals.

13. **Key-as-boolean**: `if (av)` where `av` is a `key` must check
    `av ~= NULL_KEY and av ~= ""`.

14. **Built-in function stubs**: ~400 `ll*` functions and ~200 `os*` functions
    must be provided as Rust-side host functions registered in mlua.  The
    corpus uses 35 `ll*` and 2 `os*` functions; the full API is much larger.

### Needs careful handling

15. **State transitions**: `state new_state` in LSL triggers `state_exit` in
    the old state and `state_entry` in the new state, and discards the pending
    event queue.  Must be implemented as a control-flow mechanism in the
    transpiled Luau (e.g., coroutine yield with a state-change sentinel, or
    error/longjmp pattern).

16. **Implicit type coercions**: LSL implicitly coerces between integer, float,
    string, key, vector, and rotation in many contexts (function arguments,
    concatenation, comparison).  The transpiler must insert explicit
    conversions.

17. **Integer overflow semantics**: LSL integers are 32-bit signed with
    wrapping arithmetic.  Luau numbers are f64.  The transpiler or runtime must
    enforce wrapping for integer operations that could overflow.

18. **Detected-event functions**: `llDetectedKey(i)`, `llDetectedName(i)`, etc.
    work only inside certain event handlers and refer to the detection list
    passed with the event.  The runtime must provide this context.

---

## YEngine compilation pipeline (Spike Task 1.4)

### Architecture overview

YEngine (formerly XMREngine, formerly MMREngine) is the primary LSL script
engine in OpenSimulator 0.9.3.  It compiles LSL source code to .NET CIL
(Common Intermediate Language) and executes the resulting code on the Mono/.NET
runtime.

The pipeline has four stages:

```
LSL source text
    |
    v
[1. Tokenizer]  (MMRScriptTokenize.cs)
    |  raw token list: keywords, identifiers, literals, operators
    v
[2. Parser/Reducer]  (MMRScriptReduce.cs)
    |  abstract syntax tree (TokenScript)
    v
[3. Code Generator]  (MMRScriptCodeGen.cs)
    |  CIL instructions written to a binary .yobj file
    v
[4. Object Code Loader]  (MMRScriptObjCode.cs)
    |  ScriptObjCode: callable delegates, state/event handler table
    v
[Runtime]  (XMRInstRun.cs, XMRScriptUThread.cs)
    event dispatch, microthread scheduling, state capture/restore
```

### Stage 1: Tokenizer (MMRScriptTokenize.cs)

- Parses raw LSL source into a doubly-linked list of `Token` objects.
- Recognises LSL types (`integer`, `float`, `string`, `key`, `vector`,
  `rotation`, `list`), keywords (`default`, `state`, `if`, `else`, `for`,
  `while`, `do`, `jump`, `return`), operators, and literals.
- LSL-specific tokens: vector/rotation angle-bracket literals, `state` keyword.
- Maximum name length: 255 chars.  Maximum string literal: 4096 chars.

### Stage 2: Parser/Reducer (MMRScriptReduce.cs)

- Converts the flat token list into an abstract syntax tree (`TokenScript`).
- Operator precedence table follows the LSL wiki specification.
- Handles states, event handlers, user-defined functions, variable
  declarations, and all expression forms.
- Supports YEngine extensions beyond standard LSL: `try`/`catch`/`throw`,
  classes (SDT = script-defined types), access modifiers
  (`private`/`protected`/`public`/`static`/`virtual`/`override`/`abstract`/
  `final`/`new`).

### Stage 3: Code Generator (MMRScriptCodeGen.cs, ~5600 lines)

- Walks the AST and emits CIL instructions via `System.Reflection.Emit`.
- LSL types map to .NET types:
  - `integer` -> `LSL_Integer` (wraps `int`)
  - `float` -> `LSL_Float` (wraps `double`)
  - `string` -> `LSL_String` (wraps `string`)
  - `key` -> `LSL_Key` (same as `LSL_String`)
  - `vector` -> `LSL_Vector` (wraps three `double`s)
  - `rotation` -> `LSL_Rotation` (wraps four `double`s)
  - `list` -> `LSL_List` (wraps `object[]`)
- Generates a **state/event handler table (SEHT)** -- a 2D array indexed by
  `[state_index][event_code]` mapping to CIL method entry points.
- Inserts `CheckRun()` calls at loop back-edges and function entry for
  voluntary preemption (cooperative multitasking).
- Supports stack capture/restore for microthread hibernation: each function
  gets a `callMode` switch (`NORMAL`, `SAVE`, `RESTORE`) and a `callNo`
  variable for resumption at the correct point after hibernation.

### Stage 4: Runtime (XMRInstRun.cs, XMRScriptUThread.cs)

- **Event queue**: each script instance has a `LinkedList<EventParams>` event
  queue with per-event-type count limits.  Events are posted by the simulator
  (touch, listen, timer, etc.) and dequeued by the script thread.
- **RunOne()**: runs a script for one time-slice.  Dequeues the next event,
  sets up the event handler's arguments, and calls the CIL code.
- **Microthread scheduling**: YEngine uses real .NET threads (one per script
  thread pool) but implements cooperative multitasking via `CheckRun()` calls
  inserted by the compiler.  When a time slice expires, `CheckRun()` triggers
  a `StackHibernateException` that unwinds the stack, saving frame state along
  the way.
- **State capture**: `GetExecutionState()` serializes the script's globals and
  stack frames to an XML document with a Base64-encoded binary snapshot.  This
  enables sim-crossing, persistence, and region restart without losing script
  state.
- **42 event codes** are defined in `ScriptEventCode` enum, from `attach` (0)
  through `linkset_data` (41).

### Comparison: YEngine vs. our Luau approach

| Aspect | YEngine (C#/.NET) | opensim-rust (Luau/mlua) |
|--------|-------------------|--------------------------|
| **Source language** | LSL (parsed directly) | LSL (transpiled to Luau) |
| **Compilation target** | CIL bytecode (.yobj) | Luau bytecode (mlua compile) |
| **Runtime** | .NET CLR / Mono | Luau VM via mlua |
| **Type system** | LSL types as .NET wrapper structs | LSL types as Luau userdata or tables |
| **Preemption** | `CheckRun()` CIL calls + `StackHibernateException` | `Lua::set_interrupt()` fires at function calls and back-edges |
| **State capture** | Full stack serialization (binary snapshot of CIL frames) | **NOT possible** for coroutine stacks; checkpoint pattern instead (save globals + bytecode) |
| **Memory limits** | .NET heap tracking (`XMRHeapTracker`) | `Lua::set_memory_limit()` (native Luau support) |
| **Sandbox** | Runs in .NET AppDomain with restricted permissions | Luau sandbox: no `io`/`os`/`debug`; only registered host functions |
| **State machine** | SEHT (state/event handler table) as CIL method array | Luau module returns handler tables per state; runtime dispatches |
| **Event dispatch** | `PostEvent()` -> queue -> `RunOne()` -> call CIL method | Post event -> queue -> call Luau function via mlua |
| **Extensions** | YEngine adds `try`/`catch`, classes, delegates | We can add Luau-native features (type annotations, generics) |

### Key differences and trade-offs

1. **No true stack serialization**: YEngine can freeze and resume a script
   mid-function because it captures CIL stack frames.  Luau does not expose
   coroutine internals.  Our workaround (spike 1.2) is a checkpoint pattern:
   scripts persist global state as JSON between events; long operations use
   explicit yield points.  This is adequate for most LSL scripts, which are
   event-driven and short-lived per handler invocation.

2. **Transpilation vs. direct parsing**: YEngine parses LSL directly into its
   own AST.  We add a transpilation step (LSL -> Luau source -> Luau bytecode).
   This extra step has trade-offs:
   - **Pro**: Luau is a well-tested language with good tooling, type
     annotations, and a mature VM.
   - **Pro**: we avoid writing and maintaining a full compiler backend.
   - **Con**: error messages from Luau refer to transpiled line numbers; we
     need a source-map mechanism for good diagnostics.
   - **Con**: the transpiler itself is new code that must be written and tested.

3. **Preemption model**: YEngine inserts `CheckRun()` at every back-edge and
   function call.  Luau's `set_interrupt` fires at the same points (function
   calls and loop iterations).  The semantics are equivalent: both provide
   cooperative preemption with the ability to yield or abort the script.

4. **OSSL extensions**: Both approaches need the same host-function stubs.
   YEngine registers them via .NET reflection; we register them via mlua's
   `create_function` / `set`.
