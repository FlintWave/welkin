<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Extension modules

[← Back to docs](index.md) · [Script engine](scripting.md) · [OSSL function reference](ossl.md) · [Architecture](architecture.md)

Three optional extension modules give scripts capabilities beyond the core
LSL and OSSL APIs: an in-memory JSON document store, Windlight environment
control, and a bridge that lets scripts call into arbitrary region modules.
All three live in `crates/welkin-script-vm/src/` and register their functions
as Lua globals when the VM starts.

## JsonStore

Code: `crates/welkin-script-vm/src/jsonstore.rs` (888 lines, 20 tests).
Reference: `OpenSim/Region/OptionalModules/Scripting/JsonStore/JsonStoreScriptModule.cs`.

JsonStore gives scripts a per-VM in-memory JSON document store. Each store
is a `serde_json::Value` tree identified by a UUID. Scripts create a store,
read and write values by dot-separated paths (`.person.name`, `.items[2]`),
and destroy it when done. Stores do not persist across region restarts; they
exist for the lifetime of the script VM.

### Functions

| Function | Signature | Returns |
|---|---|---|
| `JsonCreateStore` | `(json: string) → string` | UUID of the new store, or `NULL_KEY` on invalid JSON. An empty string creates an empty object. |
| `JsonDestroyStore` | `(store_id: string) → integer` | 1 if removed, 0 if not found. |
| `JsonTestStore` | `(store_id: string) → integer` | 1 if the store exists, 0 otherwise. |
| `JsonGetValue` | `(store_id: string, path: string) → string` | Value as a string. Booleans become `"1"` or `"0"`, numbers become their decimal representation. Objects and arrays become JSON text. Returns empty string if not found. |
| `JsonSetValue` | `(store_id: string, path: string, value: string) → integer` | 1 on success. The value is parsed as JSON first; if that fails, it is stored as a raw string. Creates intermediate objects when needed. |
| `JsonRemoveValue` | `(store_id: string, path: string) → integer` | 1 if removed, 0 if nothing was at that path. |
| `JsonGetArrayLength` | `(store_id: string, path: string) → integer` | Length of the array at `path`, or -1 if the path does not point to an array. |
| `JsonGetNodeType` | `(store_id: string, path: string) → integer` | `JSON_NODETYPE_OBJECT` (1), `JSON_NODETYPE_ARRAY` (2), `JSON_NODETYPE_VALUE` (3), or `JSON_NODETYPE_UNDEF` (-1). |
| `JsonGetValueType` | `(store_id: string, path: string) → integer` | `JSON_VALUETYPE_BOOLEAN` (1), `INTEGER` (2), `FLOAT` (3), `STRING` (4), `UUID` (5), or `UNDEF` (-1). UUID strings are detected by format. |
| `JsonGetJson` | `(store_id: string, path: string) → string` | The value as raw JSON text (strings are quoted, objects are serialized). |
| `JsonSetJson` | `(store_id: string, path: string, json: string) → integer` | 1 on success. The value must be valid JSON; returns 0 if parsing fails. |
| `JsonTakeValue` | `(store_id: string, path: string) → string` | Same as `JsonGetValue`, but removes the value from the store. |
| `JsonTakeValueJson` | `(store_id: string, path: string) → string` | Same as `JsonGetJson`, but removes the value from the store. |
| `JsonList2Path` | `(list: table) → string` | Converts a Lua list into a JSON path string. Numeric elements become array indices (`[0]`), string elements become dot-separated keys (`.name`). |

### Constants

| Constant | Value |
|---|---|
| `JSON_NODETYPE_UNDEF` | -1 |
| `JSON_NODETYPE_OBJECT` | 1 |
| `JSON_NODETYPE_ARRAY` | 2 |
| `JSON_NODETYPE_VALUE` | 3 |
| `JSON_VALUETYPE_UNDEF` | -1 |
| `JSON_VALUETYPE_BOOLEAN` | 1 |
| `JSON_VALUETYPE_INTEGER` | 2 |
| `JSON_VALUETYPE_FLOAT` | 3 |
| `JSON_VALUETYPE_STRING` | 4 |
| `JSON_VALUETYPE_UUID` | 5 |

### Path syntax

Paths use dot notation for object keys and bracket notation for array
indices: `.person.name`, `.items[0]`, `.a[0].b`. An empty string or `"."`
refers to the root of the store. `JsonList2Path` can build paths from a
Lua list: `JsonList2Path({"items", "0"})` produces `".items[0]"`.

### Example

```lua
local store = JsonCreateStore('{"users":[]}')
JsonSetValue(store, ".users[0]", '{"name":"Alice","score":100}')
local name = JsonGetValue(store, ".users[0].name")  -- "Alice"
local count = JsonGetArrayLength(store, ".users")     -- 1
JsonDestroyStore(store)
```

---

## LightShare

Code: `crates/welkin-script-vm/src/lightshare.rs` (475 lines, 6 tests).

LightShare is the script-side API for Windlight environment settings: sky
color, water appearance, cloud behavior, haze, and sun position. It
implements the `ls*` functions originally provided by the LightShare region
module in OpenSimulator. Viewers that support Windlight (Aurora, Firestorm)
pick up these settings and render the environment accordingly.

The underlying data is a `WindlightData` struct with 36 fields covering
water, sky, and cloud parameters. When a script calls `lsSetWindlightScene`,
the changes are applied to the script context and, if a `windlight_sender`
channel is configured, broadcast to the region for viewer delivery.

### Functions

| Function | Signature | Returns |
|---|---|---|
| `lsGetWindlightScene` | `(rules: table) → table` | Reads the current windlight settings. Takes a list of rule IDs and returns a flat list of `[rule, value, rule, value, ...]` pairs. |
| `lsSetWindlightScene` | `(rules: table) → integer` | Applies windlight changes region-wide. Takes a flat list of `[rule, value, rule, value, ...]` pairs. Returns 1 on success. |
| `lsSetWindlightSceneTargeted` | `(rules: table, target: string) → integer` | Same as `lsSetWindlightScene` but intended for a specific avatar UUID. Currently applies region-wide (per-avatar targeting is logged but not yet filtered). |

### Rule constants

36 `WL_*` constants are registered, one per windlight parameter:

| Group | Constants |
|---|---|
| Water | `WL_WATER_COLOR` (vec3), `WL_WATER_FOG_DENSITY_EXPONENT`, `WL_UNDERWATER_FOG_MODIFIER`, `WL_REFLECTION_WAVELET_SCALE` (vec3), `WL_FRESNEL_SCALE`, `WL_FRESNEL_OFFSET`, `WL_REFRACT_SCALE_ABOVE`, `WL_REFRACT_SCALE_BELOW`, `WL_BLUR_MULTIPLIER`, `WL_BIG_WAVE_DIRECTION` (vec2), `WL_LITTLE_WAVE_DIRECTION` (vec2), `WL_NORMAL_MAP_TEXTURE` (string UUID) |
| Sky | `WL_HORIZON` (vec4), `WL_HAZE_DENSITY`, `WL_HAZE_HORIZON`, `WL_DENSITY_MULTIPLIER`, `WL_DISTANCE_MULTIPLIER`, `WL_MAX_ALTITUDE`, `WL_SUN_MOON_COLOR` (vec4), `WL_SUN_MOON_POSITION`, `WL_AMBIENT` (vec4), `WL_EAST_ANGLE`, `WL_SUN_GLOW_FOCUS`, `WL_SUN_GLOW_SIZE`, `WL_SCENE_GAMMA`, `WL_STAR_BRIGHTNESS` |
| Clouds | `WL_CLOUD_COLOR` (vec4), `WL_CLOUD_XY_DENSITY` (vec3), `WL_CLOUD_COVERAGE`, `WL_CLOUD_SCALE`, `WL_CLOUD_DETAIL_XY_DENSITY` (vec3), `WL_CLOUD_SCROLL_X`, `WL_CLOUD_SCROLL_Y`, `WL_CLOUD_SCROLL_X_LOCK`, `WL_CLOUD_SCROLL_Y_LOCK`, `WL_DRAW_CLASSIC_CLOUDS` |

Vector values are Lua tables with `x`, `y`, `z` (and `w` for vec4) fields.

### Example

```lua
-- Set a sunset scene
lsSetWindlightScene({
    WL_SUN_MOON_POSITION, 0.8,
    WL_SCENE_GAMMA, 1.4,
    WL_STAR_BRIGHTNESS, 0.3,
    WL_CLOUD_COVERAGE, 0.6
})

-- Read the current water color
local result = lsGetWindlightScene({WL_WATER_COLOR})
local color = result[2]  -- {x=4, y=38, z=64}
```

---

## MOD API

Code: `crates/welkin-script-vm/src/mod_api.rs` (395 lines, 5 tests).
Reference: `OpenSim/Region/ScriptEngine/Shared/Api/Implementation/MOD_Api.cs`.

The MOD API is a typed callback bridge between scripts and region modules.
Region modules register named functions in the `mod_invoke_registry` on the
`ScriptContext`; scripts then call those functions through the `modInvoke*`
family. This is how third-party or optional modules expose functionality to
LSL/OSSL scripts without adding new os* functions.

The bridge also includes `modSendCommand`, a fire-and-forget channel for
sending named commands to modules.

### Invoke functions

Each variant calls the registered callback and casts the return value to a
specific type:

| Function | Return type |
|---|---|
| `modInvokeN(fname, ...)` | nothing (void) |
| `modInvokeS(fname, ...)` | string |
| `modInvokeI(fname, ...)` | integer |
| `modInvokeF(fname, ...)` | float |
| `modInvokeK(fname, ...)` | key (UUID string) |
| `modInvokeV(fname, ...)` | vector (`{x, y, z}` table) |
| `modInvokeR(fname, ...)` | rotation (`{x, y, z, w}` table) |
| `modInvokeL(fname, ...)` | list (Lua table) |

Arguments after `fname` are variadic. They are serialized to JSON values
internally (integers, floats, strings, booleans) and passed to the callback.
If the named function is not in the registry, the invoke returns a neutral
default: empty string, 0, `NULL_KEY`, or a zero vector/rotation.

### modSendCommand

```
modSendCommand(module: string, command: string, key: string) → string
```

Sends a `ModCommand` (module name, command string, and a caller-supplied key)
through the script context's `mod_command_sender` channel. Returns the key.
The receiving module picks up the command asynchronously. If no command sender
is configured, the call is a silent no-op.

### How modules register

Region modules register callbacks by inserting entries into
`ScriptContext.mod_invoke_registry`, a `HashMap<String, ModuleCallback>`.
A `ModuleCallback` is `Arc<dyn Fn(&[serde_json::Value]) -> serde_json::Value>`.

```rust
ctx.write().mod_invoke_registry.insert(
    "myCustomFunction".to_string(),
    Arc::new(|args: &[serde_json::Value]| {
        // Process args, return a JSON value
        serde_json::Value::String("result".into())
    }),
);
```

Scripts then call the function by name: `modInvokeS("myCustomFunction", "arg1", 42)`.

### Example

```lua
-- Call a module function that returns a string
local greeting = modInvokeS("getGreeting", "Alice")

-- Send a command to a module
modSendCommand("weatherModule", "setRain", "request-001")
```

---

## Registration and wiring

All three modules register their globals during VM startup in
`builtins/mod.rs`:

```rust
crate::lightshare::register_lightshare(lua, ctx)?;
crate::mod_api::register_mod_api(lua, ctx)?;
crate::jsonstore::register_jsonstore(lua, ctx)?;
```

The `ScriptContext` struct holds the backing state:

- `json_stores: HashMap<Uuid, serde_json::Value>` for JsonStore
- `windlight: WindlightData` plus `windlight_sender` channel for LightShare
- `mod_invoke_registry: HashMap<String, ModuleCallback>` plus `mod_command_sender` channel for the MOD API

### Testing

31 unit tests across the three modules (20 JsonStore, 6 LightShare, 5 MOD API)
cover store lifecycle, path traversal, type introspection, windlight get/set
roundtrips, callback dispatch, and command channel delivery.

```sh
cargo test -p welkin-script-vm jsonstore
cargo test -p welkin-script-vm lightshare
cargo test -p welkin-script-vm mod_api
```

## Commits

| Hash | Description |
|---|---|
| `6450844` | JsonStore: in-memory JSON document store module |
| `ce02304` | LightShare and MOD API extension modules |

## See also

- [Script engine](scripting.md) for the core LSL and Luau pipeline
- [OSSL function reference](ossl.md) for the 236 os* extension functions
- [Architecture](architecture.md) for the full crate map
