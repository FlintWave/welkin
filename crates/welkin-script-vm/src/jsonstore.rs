// SPDX-License-Identifier: LGPL-3.0-or-later
//! JsonStore API — in-memory JSON document store for the Luau script VM.
//!
//! Provides `JsonCreateStore`, `JsonDestroyStore`, `JsonGetValue`, `JsonSetValue`,
//! and related functions as Lua globals, plus `JSON_NODETYPE_*` and
//! `JSON_VALUETYPE_*` constants.
//!
//! Reference: `OpenSim/Region/OptionalModules/Scripting/JsonStore/JsonStoreScriptModule.cs`

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use regex::Regex;
use uuid::Uuid;

use crate::builtins::ScriptContext;

const NULL_KEY: &str = "00000000-0000-0000-0000-000000000000";

// ---------------------------------------------------------------------------
// Path parsing
// ---------------------------------------------------------------------------

/// A single segment in a JSON path.
#[derive(Debug, Clone)]
enum PathSegment {
    Key(String),
    Index(usize),
}

/// Parse a JsonStore path string into segments.
///
/// Examples:
///   ""        -> [] (root)
///   "."       -> [] (root)
///   ".foo"    -> [Key("foo")]
///   ".foo.bar" -> [Key("foo"), Key("bar")]
///   ".foo[0]" -> [Key("foo"), Index(0)]
///   "[0]"     -> [Index(0)]
fn parse_path(path: &str) -> Vec<PathSegment> {
    let path = path.trim();
    if path.is_empty() || path == "." {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;

    // Skip leading dot
    if i < chars.len() && chars[i] == '.' {
        i += 1;
    }

    while i < chars.len() {
        if chars[i] == '[' {
            // Array index
            i += 1; // skip '['
            let start = i;
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
            if let Ok(idx) = chars[start..i].iter().collect::<String>().parse::<usize>() {
                segments.push(PathSegment::Index(idx));
            }
            if i < chars.len() {
                i += 1; // skip ']'
            }
        } else if chars[i] == '.' {
            i += 1; // skip '.'
        } else {
            // Key segment — read until '.' or '['
            let start = i;
            while i < chars.len() && chars[i] != '.' && chars[i] != '[' {
                i += 1;
            }
            let key: String = chars[start..i].iter().collect();
            if !key.is_empty() {
                segments.push(PathSegment::Key(key));
            }
        }
    }

    segments
}

// ---------------------------------------------------------------------------
// Path resolution helpers
// ---------------------------------------------------------------------------

fn resolve_path<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let segments = parse_path(path);
    let mut current = root;
    for seg in &segments {
        match seg {
            PathSegment::Key(k) => {
                current = current.as_object()?.get(k)?;
            }
            PathSegment::Index(idx) => {
                current = current.as_array()?.get(*idx)?;
            }
        }
    }
    Some(current)
}

#[allow(dead_code)]
fn resolve_path_mut<'a>(
    root: &'a mut serde_json::Value,
    path: &str,
) -> Option<&'a mut serde_json::Value> {
    let segments = parse_path(path);
    let mut current = root;
    for seg in &segments {
        match seg {
            PathSegment::Key(k) => {
                current = current.as_object_mut()?.get_mut(k)?;
            }
            PathSegment::Index(idx) => {
                current = current.as_array_mut()?.get_mut(*idx)?;
            }
        }
    }
    Some(current)
}

/// Remove the value at `path` from `root`, returning the removed value.
fn remove_at_path(root: &mut serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let segments = parse_path(path);
    if segments.is_empty() {
        // Removing root: replace with null and return old value
        let old = root.clone();
        *root = serde_json::Value::Object(serde_json::Map::new());
        return Some(old);
    }

    // Navigate to the parent, then remove the last segment
    let (parent_segs, last) = segments.split_at(segments.len() - 1);

    let mut current = root;
    for seg in parent_segs {
        match seg {
            PathSegment::Key(k) => {
                current = current.as_object_mut()?.get_mut(k)?;
            }
            PathSegment::Index(idx) => {
                current = current.as_array_mut()?.get_mut(*idx)?;
            }
        }
    }

    match &last[0] {
        PathSegment::Key(k) => current.as_object_mut()?.remove(k),
        PathSegment::Index(idx) => {
            let arr = current.as_array_mut()?;
            if *idx < arr.len() {
                Some(arr.remove(*idx))
            } else {
                None
            }
        }
    }
}

/// Set a value at the given path, creating intermediate objects as needed.
fn set_at_path(
    root: &mut serde_json::Value,
    path: &str,
    value: serde_json::Value,
) -> bool {
    let segments = parse_path(path);
    if segments.is_empty() {
        *root = value;
        return true;
    }

    let (parent_segs, last) = segments.split_at(segments.len() - 1);

    let mut current = root;
    for seg in parent_segs {
        match seg {
            PathSegment::Key(k) => {
                if !current.is_object() {
                    return false;
                }
                let obj = current.as_object_mut().unwrap();
                if !obj.contains_key(k) {
                    obj.insert(k.clone(), serde_json::Value::Object(serde_json::Map::new()));
                }
                current = obj.get_mut(k).unwrap();
            }
            PathSegment::Index(idx) => {
                if let Some(arr) = current.as_array_mut() {
                    if *idx < arr.len() {
                        current = &mut arr[*idx];
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }
    }

    match &last[0] {
        PathSegment::Key(k) => {
            if let Some(obj) = current.as_object_mut() {
                obj.insert(k.clone(), value);
                true
            } else {
                false
            }
        }
        PathSegment::Index(idx) => {
            if let Some(arr) = current.as_array_mut() {
                if *idx < arr.len() {
                    arr[*idx] = value;
                    true
                } else if *idx == arr.len() {
                    arr.push(value);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Value formatting helpers
// ---------------------------------------------------------------------------

fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => if *b { "1".into() } else { "0".into() },
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(v).unwrap_or_default()
        }
    }
}

fn value_to_json_string(v: &serde_json::Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn is_uuid_string(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    // Thread-local compiled regex for performance
    thread_local! {
        static UUID_RE: Regex = Regex::new(
            r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
        ).unwrap();
    }
    UUID_RE.with(|re| re.is_match(s))
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all `Json*` functions and `JSON_*` constants as Lua globals.
pub fn register_jsonstore(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let globals = lua.globals();

    // JSON_NODETYPE_* constants
    globals.set("JSON_NODETYPE_UNDEF", -1i64)?;
    globals.set("JSON_NODETYPE_OBJECT", 1i64)?;
    globals.set("JSON_NODETYPE_ARRAY", 2i64)?;
    globals.set("JSON_NODETYPE_VALUE", 3i64)?;

    // JSON_VALUETYPE_* constants
    globals.set("JSON_VALUETYPE_UNDEF", -1i64)?;
    globals.set("JSON_VALUETYPE_BOOLEAN", 1i64)?;
    globals.set("JSON_VALUETYPE_INTEGER", 2i64)?;
    globals.set("JSON_VALUETYPE_FLOAT", 3i64)?;
    globals.set("JSON_VALUETYPE_STRING", 4i64)?;
    globals.set("JSON_VALUETYPE_UUID", 5i64)?;

    // JsonCreateStore(value: String) -> String
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, value: String| {
            let json_val: serde_json::Value = if value.is_empty() {
                serde_json::Value::Object(serde_json::Map::new())
            } else {
                match serde_json::from_str(&value) {
                    Ok(v) => v,
                    Err(_) => return Ok(NULL_KEY.to_string()),
                }
            };
            let id = Uuid::new_v4();
            let id_str = id.to_string();
            ctx.write().json_stores.insert(id, json_val);
            Ok(id_str)
        })?;
        globals.set("JsonCreateStore", f)?;
    }

    // JsonDestroyStore(store_id: String) -> i64
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, store_id: String| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(0i64),
            };
            let removed = ctx.write().json_stores.remove(&id).is_some();
            Ok(if removed { 1i64 } else { 0i64 })
        })?;
        globals.set("JsonDestroyStore", f)?;
    }

    // JsonTestStore(store_id: String) -> i64
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, store_id: String| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(0i64),
            };
            let exists = ctx.read().json_stores.contains_key(&id);
            Ok(if exists { 1i64 } else { 0i64 })
        })?;
        globals.set("JsonTestStore", f)?;
    }

    // JsonGetValue(store_id: String, path: String) -> String
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path): (String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(String::new()),
            };
            let guard = ctx.read();
            let store = match guard.json_stores.get(&id) {
                Some(s) => s,
                None => return Ok(String::new()),
            };
            match resolve_path(store, &path) {
                Some(v) => Ok(value_to_string(v)),
                None => Ok(String::new()),
            }
        })?;
        globals.set("JsonGetValue", f)?;
    }

    // JsonSetValue(store_id: String, path: String, value: String) -> i64
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path, value): (String, String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(0i64),
            };
            // Try to parse as JSON first; fall back to raw string
            let json_val: serde_json::Value = serde_json::from_str(&value)
                .unwrap_or_else(|_| serde_json::Value::String(value));
            let mut guard = ctx.write();
            let store = match guard.json_stores.get_mut(&id) {
                Some(s) => s,
                None => return Ok(0i64),
            };
            Ok(if set_at_path(store, &path, json_val) { 1i64 } else { 0i64 })
        })?;
        globals.set("JsonSetValue", f)?;
    }

    // JsonRemoveValue(store_id: String, path: String) -> i64
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path): (String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(0i64),
            };
            let mut guard = ctx.write();
            let store = match guard.json_stores.get_mut(&id) {
                Some(s) => s,
                None => return Ok(0i64),
            };
            Ok(if remove_at_path(store, &path).is_some() { 1i64 } else { 0i64 })
        })?;
        globals.set("JsonRemoveValue", f)?;
    }

    // JsonGetArrayLength(store_id: String, path: String) -> i64
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path): (String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(-1i64),
            };
            let guard = ctx.read();
            let store = match guard.json_stores.get(&id) {
                Some(s) => s,
                None => return Ok(-1i64),
            };
            match resolve_path(store, &path) {
                Some(serde_json::Value::Array(arr)) => Ok(arr.len() as i64),
                _ => Ok(-1i64),
            }
        })?;
        globals.set("JsonGetArrayLength", f)?;
    }

    // JsonGetNodeType(store_id: String, path: String) -> i64
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path): (String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(-1i64),
            };
            let guard = ctx.read();
            let store = match guard.json_stores.get(&id) {
                Some(s) => s,
                None => return Ok(-1i64),
            };
            match resolve_path(store, &path) {
                Some(serde_json::Value::Object(_)) => Ok(1i64),
                Some(serde_json::Value::Array(_)) => Ok(2i64),
                Some(_) => Ok(3i64),
                None => Ok(-1i64),
            }
        })?;
        globals.set("JsonGetNodeType", f)?;
    }

    // JsonGetValueType(store_id: String, path: String) -> i64
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path): (String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(-1i64),
            };
            let guard = ctx.read();
            let store = match guard.json_stores.get(&id) {
                Some(s) => s,
                None => return Ok(-1i64),
            };
            match resolve_path(store, &path) {
                Some(serde_json::Value::Bool(_)) => Ok(1i64),
                Some(serde_json::Value::Number(n)) => {
                    if n.is_f64() && n.as_i64().is_none() {
                        Ok(3i64) // float
                    } else {
                        Ok(2i64) // integer
                    }
                }
                Some(serde_json::Value::String(s)) => {
                    if is_uuid_string(s) {
                        Ok(5i64)
                    } else {
                        Ok(4i64)
                    }
                }
                Some(serde_json::Value::Object(_)) | Some(serde_json::Value::Array(_)) => {
                    Ok(-1i64) // containers are not "values" in the LSL sense
                }
                _ => Ok(-1i64),
            }
        })?;
        globals.set("JsonGetValueType", f)?;
    }

    // JsonList2Path(list: LuaTable) -> String
    {
        let f = lua.create_function(move |_, list: LuaTable| {
            let mut path = String::new();
            let len = list.raw_len();
            for i in 1..=len {
                let component: String = list.get(i)?;
                if component.parse::<usize>().is_ok() {
                    path.push_str(&format!("[{}]", component));
                } else {
                    path.push('.');
                    path.push_str(&component);
                }
            }
            Ok(path)
        })?;
        globals.set("JsonList2Path", f)?;
    }

    // JsonGetJson(store_id: String, path: String) -> String
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path): (String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(String::new()),
            };
            let guard = ctx.read();
            let store = match guard.json_stores.get(&id) {
                Some(s) => s,
                None => return Ok(String::new()),
            };
            match resolve_path(store, &path) {
                Some(v) => Ok(value_to_json_string(v)),
                None => Ok(String::new()),
            }
        })?;
        globals.set("JsonGetJson", f)?;
    }

    // JsonSetJson(store_id: String, path: String, json: String) -> i64
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path, json): (String, String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(0i64),
            };
            let json_val: serde_json::Value = match serde_json::from_str(&json) {
                Ok(v) => v,
                Err(_) => return Ok(0i64),
            };
            let mut guard = ctx.write();
            let store = match guard.json_stores.get_mut(&id) {
                Some(s) => s,
                None => return Ok(0i64),
            };
            Ok(if set_at_path(store, &path, json_val) { 1i64 } else { 0i64 })
        })?;
        globals.set("JsonSetJson", f)?;
    }

    // JsonTakeValue(store_id: String, path: String) -> String
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path): (String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(String::new()),
            };
            let mut guard = ctx.write();
            let store = match guard.json_stores.get_mut(&id) {
                Some(s) => s,
                None => return Ok(String::new()),
            };
            match remove_at_path(store, &path) {
                Some(v) => Ok(value_to_string(&v)),
                None => Ok(String::new()),
            }
        })?;
        globals.set("JsonTakeValue", f)?;
    }

    // JsonTakeValueJson(store_id: String, path: String) -> String
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (store_id, path): (String, String)| {
            let id = match Uuid::parse_str(&store_id) {
                Ok(id) => id,
                Err(_) => return Ok(String::new()),
            };
            let mut guard = ctx.write();
            let store = match guard.json_stores.get_mut(&id) {
                Some(s) => s,
                None => return Ok(String::new()),
            };
            match remove_at_path(store, &path) {
                Some(v) => Ok(value_to_json_string(&v)),
                None => Ok(String::new()),
            }
        })?;
        globals.set("JsonTakeValueJson", f)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_vm() -> (Lua, Arc<RwLock<ScriptContext>>) {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext::default()));
        register_jsonstore(&lua, &ctx).unwrap();
        (lua, ctx)
    }

    #[test]
    fn test_create_store_and_test_store() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"name":"Alice","age":30}')
            return JsonTestStore(id)
        "#;
        let result: i64 = lua.load(script).eval().unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_create_store_empty_string() {
        let (lua, ctx) = make_test_vm();
        let id_str: String = lua
            .load(r#"return JsonCreateStore("")"#)
            .eval()
            .unwrap();
        assert_ne!(id_str, NULL_KEY);
        let id = Uuid::parse_str(&id_str).unwrap();
        let guard = ctx.read();
        let store = guard.json_stores.get(&id).unwrap();
        assert!(store.is_object());
        assert_eq!(store.as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_create_store_invalid_json() {
        let (lua, _ctx) = make_test_vm();
        let id_str: String = lua
            .load(r#"return JsonCreateStore("{bad json")"#)
            .eval()
            .unwrap();
        assert_eq!(id_str, NULL_KEY);
    }

    #[test]
    fn test_destroy_store() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"x":1}')
            local before = JsonTestStore(id)
            local removed = JsonDestroyStore(id)
            local after = JsonTestStore(id)
            return before, removed, after
        "#;
        let (before, removed, after): (i64, i64, i64) = lua.load(script).eval().unwrap();
        assert_eq!(before, 1);
        assert_eq!(removed, 1);
        assert_eq!(after, 0);
    }

    #[test]
    fn test_get_value_at_path() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"person":{"name":"Bob","age":25}}')
            local name = JsonGetValue(id, ".person.name")
            local age = JsonGetValue(id, ".person.age")
            return name, age
        "#;
        let (name, age): (String, String) = lua.load(script).eval().unwrap();
        assert_eq!(name, "Bob");
        assert_eq!(age, "25");
    }

    #[test]
    fn test_set_get_roundtrip_nested() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"a":{"b":{}}}')
            JsonSetValue(id, ".a.b.c", "hello")
            return JsonGetValue(id, ".a.b.c")
        "#;
        let result: String = lua.load(script).eval().unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_get_array_length() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"items":[10,20,30,40]}')
            return JsonGetArrayLength(id, ".items")
        "#;
        let result: i64 = lua.load(script).eval().unwrap();
        assert_eq!(result, 4);
    }

    #[test]
    fn test_get_array_length_not_array() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"items":"not an array"}')
            return JsonGetArrayLength(id, ".items")
        "#;
        let result: i64 = lua.load(script).eval().unwrap();
        assert_eq!(result, -1);
    }

    #[test]
    fn test_get_node_type() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"obj":{"a":1},"arr":[1,2],"val":42}')
            local obj_type = JsonGetNodeType(id, ".obj")
            local arr_type = JsonGetNodeType(id, ".arr")
            local val_type = JsonGetNodeType(id, ".val")
            local missing = JsonGetNodeType(id, ".nope")
            return obj_type, arr_type, val_type, missing
        "#;
        let (obj_t, arr_t, val_t, missing): (i64, i64, i64, i64) =
            lua.load(script).eval().unwrap();
        assert_eq!(obj_t, 1);  // JSON_NODETYPE_OBJECT
        assert_eq!(arr_t, 2);  // JSON_NODETYPE_ARRAY
        assert_eq!(val_t, 3);  // JSON_NODETYPE_VALUE
        assert_eq!(missing, -1); // JSON_NODETYPE_UNDEF
    }

    #[test]
    fn test_get_value_type() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"s":"hello","i":42,"f":3.14,"b":true,"u":"550e8400-e29b-41d4-a716-446655440000"}')
            local s = JsonGetValueType(id, ".s")
            local i = JsonGetValueType(id, ".i")
            local f = JsonGetValueType(id, ".f")
            local b = JsonGetValueType(id, ".b")
            local u = JsonGetValueType(id, ".u")
            local missing = JsonGetValueType(id, ".nope")
            return s, i, f, b, u, missing
        "#;
        let (s, i, f, b, u, missing): (i64, i64, i64, i64, i64, i64) =
            lua.load(script).eval().unwrap();
        assert_eq!(s, 4); // STRING
        assert_eq!(i, 2); // INTEGER
        assert_eq!(f, 3); // FLOAT
        assert_eq!(b, 1); // BOOLEAN
        assert_eq!(u, 5); // UUID
        assert_eq!(missing, -1); // UNDEF
    }

    #[test]
    fn test_json_list2path() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local p1 = JsonList2Path({"person", "name"})
            local p2 = JsonList2Path({"arr", "0"})
            local p3 = JsonList2Path({"a", "0", "b"})
            return p1, p2, p3
        "#;
        let (p1, p2, p3): (String, String, String) = lua.load(script).eval().unwrap();
        assert_eq!(p1, ".person.name");
        assert_eq!(p2, ".arr[0]");
        assert_eq!(p3, ".a[0].b");
    }

    #[test]
    fn test_take_value_removes() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"x":99,"y":100}')
            local taken = JsonTakeValue(id, ".x")
            local after = JsonGetValue(id, ".x")
            local y_still = JsonGetValue(id, ".y")
            return taken, after, y_still
        "#;
        let (taken, after, y_still): (String, String, String) =
            lua.load(script).eval().unwrap();
        assert_eq!(taken, "99");
        assert_eq!(after, ""); // removed
        assert_eq!(y_still, "100"); // untouched
    }

    #[test]
    fn test_get_json_serialization() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"greeting":"hello","nums":[1,2]}')
            local s = JsonGetJson(id, ".greeting")
            local a = JsonGetJson(id, ".nums")
            return s, a
        "#;
        let (s, a): (String, String) = lua.load(script).eval().unwrap();
        assert_eq!(s, "\"hello\""); // JSON-quoted string
        assert_eq!(a, "[1,2]");
    }

    #[test]
    fn test_set_json() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{}')
            local ok = JsonSetJson(id, ".data", '{"nested":true}')
            local result = JsonGetValue(id, ".data.nested")
            return ok, result
        "#;
        let (ok, result): (i64, String) = lua.load(script).eval().unwrap();
        assert_eq!(ok, 1);
        assert_eq!(result, "1"); // true becomes "1"
    }

    #[test]
    fn test_set_json_invalid() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{}')
            return JsonSetJson(id, ".x", "{bad")
        "#;
        let result: i64 = lua.load(script).eval().unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_array_index_path() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"items":["a","b","c"]}')
            local first = JsonGetValue(id, ".items[0]")
            local last = JsonGetValue(id, ".items[2]")
            return first, last
        "#;
        let (first, last): (String, String) = lua.load(script).eval().unwrap();
        assert_eq!(first, "a");
        assert_eq!(last, "c");
    }

    #[test]
    fn test_root_path() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"a":1}')
            local root_type = JsonGetNodeType(id, ".")
            local root_type2 = JsonGetNodeType(id, "")
            return root_type, root_type2
        "#;
        let (t1, t2): (i64, i64) = lua.load(script).eval().unwrap();
        assert_eq!(t1, 1); // OBJECT
        assert_eq!(t2, 1); // OBJECT
    }

    #[test]
    fn test_take_value_json() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"obj":{"inner":true}}')
            local taken = JsonTakeValueJson(id, ".obj")
            local after = JsonGetValue(id, ".obj")
            return taken, after
        "#;
        let (taken, after): (String, String) = lua.load(script).eval().unwrap();
        assert_eq!(taken, "{\"inner\":true}");
        assert_eq!(after, ""); // removed
    }

    #[test]
    fn test_remove_value() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            local id = JsonCreateStore('{"a":1,"b":2}')
            local ok = JsonRemoveValue(id, ".a")
            local missing = JsonRemoveValue(id, ".nonexistent")
            local a_gone = JsonGetValue(id, ".a")
            local b_still = JsonGetValue(id, ".b")
            return ok, missing, a_gone, b_still
        "#;
        let (ok, missing, a_gone, b_still): (i64, i64, String, String) =
            lua.load(script).eval().unwrap();
        assert_eq!(ok, 1);
        assert_eq!(missing, 0);
        assert_eq!(a_gone, "");
        assert_eq!(b_still, "2");
    }

    #[test]
    fn test_constants_registered() {
        let (lua, _ctx) = make_test_vm();
        let script = r#"
            return JSON_NODETYPE_UNDEF, JSON_NODETYPE_OBJECT, JSON_NODETYPE_ARRAY,
                   JSON_NODETYPE_VALUE, JSON_VALUETYPE_UNDEF, JSON_VALUETYPE_BOOLEAN,
                   JSON_VALUETYPE_INTEGER, JSON_VALUETYPE_FLOAT, JSON_VALUETYPE_STRING,
                   JSON_VALUETYPE_UUID
        "#;
        let vals: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) =
            lua.load(script).eval().unwrap();
        assert_eq!(vals, (-1, 1, 2, 3, -1, 1, 2, 3, 4, 5));
    }
}
