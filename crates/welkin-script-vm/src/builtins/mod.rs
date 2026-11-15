// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL built-in function bindings for the Luau VM.
//!
//! Each sub-module registers a family of `ll*` functions as Lua globals so
//! that in-world scripts can call them.  Pure-math and string functions are
//! fully implemented; communication, physics and object functions are stubs
//! that log via [`tracing`] and return sensible defaults.

pub mod avatar;
pub mod communication;
pub mod inventory;
pub mod land;
pub mod list;
pub mod math;
pub mod object;
pub mod physics;
pub mod string;

use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use crate::VmError;

pub const INVENTORY_TEXTURE: i32 = 0;
pub const INVENTORY_SOUND: i32 = 1;
pub const INVENTORY_LANDMARK: i32 = 3;
pub const INVENTORY_CLOTHING: i32 = 5;
pub const INVENTORY_OBJECT: i32 = 6;
pub const INVENTORY_NOTECARD: i32 = 7;
pub const INVENTORY_SCRIPT: i32 = 10;
pub const INVENTORY_BODYPART: i32 = 13;
pub const INVENTORY_ANIMATION: i32 = 20;
pub const INVENTORY_GESTURE: i32 = 21;
pub const INVENTORY_ALL: i32 = -1;
pub const INVENTORY_NONE: i32 = -1;

#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub name: String,
    pub item_type: i32,
    pub asset_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ListenFilter {
    pub handle: i32,
    pub channel: i32,
    pub name: String,
    pub id: String,
    pub msg: String,
    pub active: bool,
}

/// Shared context available to every ll* function closure.
///
/// Wrapped in `Arc<RwLock<..>>` and cloned into each registered closure so
/// that builtins can read prim identity, position, etc.
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
    pub color: [f64; 3],
    pub alpha: f64,
    pub creator_id: Uuid,
    pub texture: Uuid,
    pub permissions_granted: u32,
    pub sit_target_pos: [f64; 3],
    pub sit_target_rot: [f64; 4],
    pub sitting_avatar: Option<Uuid>,
    pub inventory: Vec<InventoryItem>,
    pub allow_inventory_drop: bool,
    pub parcel_name: String,
    pub parcel_description: String,
    pub parcel_owner: Uuid,
    pub parcel_area: i32,
    pub parcel_flags: u32,
    pub event_sender: Option<std::sync::mpsc::Sender<(Uuid, crate::ScriptEvent)>>,
    pub chat_sender: Option<std::sync::mpsc::Sender<crate::ChatMessage>>,
    pub listen_handles: Vec<ListenFilter>,
    pub next_listen_handle: i32,
    pub physics_sender: Option<std::sync::mpsc::Sender<(Uuid, crate::PhysicsCommand)>>,
    pub velocity: [f64; 3],
    pub angular_velocity: [f64; 3],
    pub acceleration: [f64; 3],
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
            color: [1.0, 1.0, 1.0],
            alpha: 1.0,
            creator_id: Uuid::nil(),
            texture: Uuid::nil(),
            permissions_granted: 0,
            sit_target_pos: [0.0; 3],
            sit_target_rot: [0.0, 0.0, 0.0, 1.0],
            sitting_avatar: None,
            inventory: Vec::new(),
            allow_inventory_drop: false,
            parcel_name: String::new(),
            parcel_description: String::new(),
            parcel_owner: Uuid::nil(),
            parcel_area: 256,
            parcel_flags: 0,
            event_sender: None,
            chat_sender: None,
            listen_handles: Vec::new(),
            next_listen_handle: 1,
            physics_sender: None,
            velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            acceleration: [0.0; 3],
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
    avatar::register_avatar(lua, ctx)?;
    inventory::register_inventory(lua, ctx)?;
    land::register_land(lua, ctx)?;
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
            ..Default::default()
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
