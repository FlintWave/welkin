// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL built-in function bindings for the Luau VM.
//!
//! Each sub-module registers a family of `ll*` functions as Lua globals so
//! that in-world scripts can call them.  Pure-math and string functions are
//! fully implemented; communication, physics and object functions are stubs
//! that log via [`tracing`] and return sensible defaults.

pub mod communication;
pub mod list;
pub mod math;
pub mod object;
pub mod physics;
pub mod string;

use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use crate::VmError;

/// Shared context available to every ll* function closure.
///
/// Wrapped in `Arc<RwLock<..>>` and cloned into each registered closure so
/// that stubs can read prim identity, position, etc.
pub struct ScriptContext {
    pub script_id: Uuid,
    pub prim_id: Uuid,
    pub prim_name: String,
    pub prim_desc: String,
    pub owner_id: Uuid,
    pub position: [f64; 3],
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
    pub link_num: i32,
    pub event_sender: Option<std::sync::mpsc::Sender<(Uuid, crate::ScriptEvent)>>,
}

impl Default for ScriptContext {
    fn default() -> Self {
        Self {
            script_id: Uuid::nil(),
            prim_id: Uuid::nil(),
            prim_name: String::new(),
            prim_desc: String::new(),
            owner_id: Uuid::nil(),
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            link_num: 0,
            event_sender: None,
        }
    }
}

/// Register **all** built-in `ll*` functions on `lua`.
///
/// Math and string functions are pure and need no context.
/// Communication, physics and object stubs use `ctx` for identity / defaults.
pub fn register_all(lua: &mlua::Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), VmError> {
    math::register_math(lua)?;
    string::register_string(lua)?;
    list::register_list(lua)?;
    communication::register_communication(lua, ctx)?;
    physics::register_physics(lua, ctx)?;
    object::register_object(lua, ctx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a non-sandboxed VM with all builtins registered.
    fn vm_with_builtins() -> (mlua::Lua, Arc<RwLock<ScriptContext>>) {
        let lua = mlua::Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            prim_name: "TestPrim".into(),
            prim_desc: "A test prim".into(),
            owner_id: Uuid::new_v4(),
            position: [128.0, 128.0, 25.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            link_num: 0,
            event_sender: None,
        }));
        register_all(&lua, &ctx).expect("register_all should succeed");
        (lua, ctx)
    }

    #[test]
    fn register_all_succeeds() {
        let _pair = vm_with_builtins();
    }

    #[test]
    fn end_to_end_math_and_string() {
        let (lua, _ctx) = vm_with_builtins();

        let result: f64 = lua
            .load("return llAbs(-42)")
            .eval()
            .expect("llAbs should work");
        assert!((result - 42.0).abs() < f64::EPSILON);

        let result: String = lua
            .load(r#"return llToUpper("hello")"#)
            .eval()
            .expect("llToUpper should work");
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn end_to_end_communication_stub() {
        let (lua, _ctx) = vm_with_builtins();

        // Should not panic — just logs.
        lua.load(r#"llSay(0, "Hello from test!")"#)
            .exec()
            .expect("llSay should not error");

        lua.load(r#"llWhisper(0, "psst")"#)
            .exec()
            .expect("llWhisper should not error");
    }

    #[test]
    fn end_to_end_object_read() {
        let (lua, _ctx) = vm_with_builtins();

        let name: String = lua
            .load("return llGetObjectName()")
            .eval()
            .expect("llGetObjectName should work");
        assert_eq!(name, "TestPrim");

        let desc: String = lua
            .load("return llGetObjectDesc()")
            .eval()
            .expect("llGetObjectDesc should work");
        assert_eq!(desc, "A test prim");
    }

    #[test]
    fn end_to_end_multi_function_script() {
        let (lua, _ctx) = vm_with_builtins();

        let script = r#"
            local name = llGetObjectName()
            local upper = llToUpper(name)
            local len = llStringLength(upper)
            llSay(0, upper)
            return len
        "#;

        let len: i64 = lua
            .load(script)
            .eval()
            .expect("multi-function script should work");
        assert_eq!(len, 8); // "TESTPRIM"
    }

    #[test]
    fn end_to_end_list_operations() {
        let (lua, _ctx) = vm_with_builtins();

        let script = r#"
            local list = {10, 20, 30}
            local len = llGetListLength(list)
            local val = llList2Integer(list, 1)
            return len + val
        "#;

        let result: i64 = lua.load(script).eval().expect("list ops should work");
        assert_eq!(result, 23); // 3 + 20
    }
}
