# LSL Compiler — Design

## Architecture

```
LSL source (.lsl)
    |
    v
┌──────────────┐
│  Lexer       │  LSL tokens (keywords, literals, operators)
└──────┬───────┘
       v
┌──────────────┐
│  Parser      │  LSL AST (typed, with spans for error reporting)
└──────┬───────┘
       v
┌──────────────┐
│  Semantic    │  Type checking, implicit cast insertion,
│  Analysis    │  scope resolution, constant folding
└──────┬───────┘
       v
┌──────────────┐
│  Transpiler  │  Luau source code generation
└──────┬───────┘
       v
┌──────────────┐
│  mlua        │  Luau bytecode compilation (existing)
│  Compiler    │  via Lua::new().load(luau_source).compile()
└──────┬───────┘
       v
ScriptInstance (existing) → ScriptScheduler (existing)
```

## LSL Grammar

LSL is a C-like language with these distinctive features:

### Types
- `integer` (i32), `float` (f32), `string`, `key` (UUID as string)
- `vector` (`<x, y, z>` — 3 floats), `rotation` (`<x, y, z, s>` — quaternion)
- `list` (heterogeneous ordered collection)

### Implicit Cast Rules (from LSL spec)
| From | To | Rule |
|---|---|---|
| integer | float | automatic |
| string | key | automatic |
| key | string | automatic |
| integer | string | in string context (+) |
| float | string | in string context (+) |
| vector | string | `"<x, y, z>"` |
| rotation | string | `"<x, y, z, s>"` |

### State Machine
Every script has a `default` state and optionally named states.
Each state contains event handlers. `state <name>` transitions between states.

```lsl
default {
    state_entry() { llSay(0, "Hello"); }
    touch_start(integer num) { state active; }
}
state active {
    state_entry() { llSay(0, "Active"); }
    touch_start(integer num) { state default; }
}
```

### Events (from OpenSim, 36 event types)
```
state_entry, state_exit, touch_start, touch, touch_end,
collision_start, collision, collision_end,
land_collision_start, land_collision, land_collision_end,
timer, listen, sensor, no_sensor,
control, at_target, not_at_target, at_rot_target, not_at_rot_target,
money, email, run_time_permissions, changed,
attach, dataserver, moving_start, moving_end,
on_rez, object_rez, remote_data,
http_response, http_request,
link_message, transaction_result,
experience_permissions, experience_permissions_denied (OpenSim extension)
```

## Luau Code Generation Strategy

### State Machine → Dispatch Table

LSL state machine becomes a Luau table-of-tables:

```lua
-- Generated Luau output
local current_state = "default"

local states = {
    default = {
        state_entry = function()
            llSay(0, "Hello")
        end,
        touch_start = function(num)
            current_state = "active"
            -- fire state_exit for old state
            if states[current_state] and states[current_state].state_entry then
                states[current_state].state_entry()
            end
        end,
    },
    active = {
        state_entry = function()
            llSay(0, "Active")
        end,
    },
}

-- Event dispatch (called by scheduler)
function __dispatch(event_name, ...)
    local handler = states[current_state] and states[current_state][event_name]
    if handler then handler(...) end
end
```

### Type Coercions

Insert explicit conversion calls where LSL does implicit casts:

```lua
-- LSL: string s = (string)pos + " is here";
-- Luau:
local s = __vec2str(pos) .. " is here"
```

Runtime helpers registered in the Luau environment:
- `__int2float(i)`, `__float2int(f)` — numeric casts
- `__vec2str(v)`, `__rot2str(r)` — vector/rotation to string
- `__str2key(s)`, `__key2str(k)` — key/string interconversion
- `__list_add(a, b)` — LSL list concatenation
- `__list_get_int(l, i)`, etc. — typed list access

### Vector/Rotation as Luau Tables

```lua
-- LSL: vector pos = <1.0, 2.0, 3.0>;
local pos = {x = 1.0, y = 2.0, z = 3.0}

-- LSL: rotation rot = <0, 0, 0, 1>;
local rot = {x = 0, y = 0, z = 0, s = 1}

-- LSL: pos.x = 5.0;
pos.x = 5.0

-- LSL: vector sum = pos + other;
local sum = __vec_add(pos, other)
```

### LSL List Semantics

LSL lists are heterogeneous. In Luau they become arrays with typed wrappers:

```lua
-- LSL: list data = [1, "hello", <0,0,0>];
local data = {__lsl_int(1), __lsl_str("hello"), __lsl_vec({x=0,y=0,z=0})}

-- LSL: integer x = llList2Integer(data, 0);
local x = llList2Integer(data, 0)  -- already registered as builtin
```

## Region Integration

### Script Lifecycle

1. **Rez/Create**: Object rezzed in scene → check for script inventory items →
   for each script: fetch asset → compile LSL→Luau→bytecode → create
   ScriptInstance → register with ScriptScheduler → fire `on_rez` then
   `state_entry`

2. **Edit**: UpdateScriptAgent cap receives new source → compile → hot-swap
   ScriptInstance (preserve state if same globals) → fire `state_entry`

3. **Delete/Derez**: Object removed → ScriptScheduler removes instances →
   serialize state for inventory storage

4. **Region crossing**: Serialize VM state → transfer to destination region →
   restore ScriptInstance → continue execution

### Event Wiring (scene → scheduler)

| Scene Event | Script Event | Source |
|---|---|---|
| ObjectGrab packet | touch_start | PacketHandler |
| ObjectGrabUpdate packet | touch | PacketHandler |
| ObjectDeGrab packet | touch_end | PacketHandler |
| ChatFromViewer packet | listen | ChatModule |
| Money transfer | money | MoneyModule |
| Collision detect | collision_start/end | PhysicsScene |
| Timer tick | timer | ScriptScheduler internal |
| HTTP response | http_response | AsyncHTTPModule |
| Changed event | changed | Scene (inv change, region change, etc.) |
| Link message | link_message | ScriptAPI (llMessageLinked) |

### Main Loop Integration

The ScriptScheduler must tick in the region's main loop:

```rust
// In region main loop (every frame, ~45ms target)
physics_scene.step(dt);
script_scheduler.tick(max_instructions_per_frame);
scene.update_presences();
scene.send_updates();
```

`tick()` already exists in welkin-script-vm — it round-robins across active
ScriptInstances with instruction limits. Just needs to be called from the
region loop.

## Decisions

1. **Transpile to Luau source, not bytecode.** The Luau compiler (`mlua`)
   handles bytecode generation. We emit readable Luau so debugging and
   error messages map back to LSL structure.

2. **Hand-written recursive descent parser.** LSL grammar is small (no
   generics, no templates, no macros). A hand-written parser gives better
   error messages than a parser generator and has no external dependencies.

3. **Runtime helpers over inline expansion.** Type coercions and operator
   overloads dispatch to registered Luau functions rather than inlining
   the logic at every call site. Smaller generated code, one place to fix.

4. **Event dispatch table pattern.** Each state is a Luau table of handler
   functions. State transitions swap the current_state string. This matches
   how the existing ScriptInstance.dispatch_event() already works.

5. **Compile on rez, not ahead of time.** Scripts compile when the object
   enters the scene. No persistent bytecode cache initially — can be added
   later as an optimization.

6. **ll* function priority:** implement the most-used functions first
   (communication, object, movement, sensor, HTTP) and leave rare functions
   (experience, pathfinding) for later passes.
