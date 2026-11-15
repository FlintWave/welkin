// SPDX-License-Identifier: GPL-3.0-or-later
//! NPC stub functions (Task 8.1) — extends the base 4 NPC stubs to the full 24.

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn ossl_npc_funcs() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osIsNpc",
            level: OsslThreatLevel::High,
            register: register_os_is_npc,
        },
        OsslFunc {
            name: "osNpcSaveAppearance",
            level: OsslThreatLevel::High,
            register: register_os_npc_save_appearance,
        },
        OsslFunc {
            name: "osNpcLoadAppearance",
            level: OsslThreatLevel::High,
            register: register_os_npc_load_appearance,
        },
        OsslFunc {
            name: "osNpcGetOwner",
            level: OsslThreatLevel::High,
            register: register_os_npc_get_owner,
        },
        OsslFunc {
            name: "osNpcGetPos",
            level: OsslThreatLevel::High,
            register: register_os_npc_get_pos,
        },
        OsslFunc {
            name: "osNpcMoveToTarget",
            level: OsslThreatLevel::High,
            register: register_os_npc_move_to_target,
        },
        OsslFunc {
            name: "osNpcGetRot",
            level: OsslThreatLevel::High,
            register: register_os_npc_get_rot,
        },
        OsslFunc {
            name: "osNpcSetRot",
            level: OsslThreatLevel::High,
            register: register_os_npc_set_rot,
        },
        OsslFunc {
            name: "osNpcStopMoveToTarget",
            level: OsslThreatLevel::High,
            register: register_os_npc_stop_move_to_target,
        },
        OsslFunc {
            name: "osNpcSetProfileAbout",
            level: OsslThreatLevel::High,
            register: register_os_npc_set_profile_about,
        },
        OsslFunc {
            name: "osNpcSetProfileImage",
            level: OsslThreatLevel::High,
            register: register_os_npc_set_profile_image,
        },
        OsslFunc {
            name: "osNpcSayTo",
            level: OsslThreatLevel::High,
            register: register_os_npc_say_to,
        },
        OsslFunc {
            name: "osNpcShout",
            level: OsslThreatLevel::High,
            register: register_os_npc_shout,
        },
        OsslFunc {
            name: "osNpcSit",
            level: OsslThreatLevel::High,
            register: register_os_npc_sit,
        },
        OsslFunc {
            name: "osNpcStand",
            level: OsslThreatLevel::High,
            register: register_os_npc_stand,
        },
        OsslFunc {
            name: "osNpcPlayAnimation",
            level: OsslThreatLevel::High,
            register: register_os_npc_play_animation,
        },
        OsslFunc {
            name: "osNpcStopAnimation",
            level: OsslThreatLevel::High,
            register: register_os_npc_stop_animation,
        },
        OsslFunc {
            name: "osNpcTouch",
            level: OsslThreatLevel::High,
            register: register_os_npc_touch,
        },
        OsslFunc {
            name: "osNpcWhisper",
            level: OsslThreatLevel::High,
            register: register_os_npc_whisper,
        },
        OsslFunc {
            name: "osNpcLookAt",
            level: OsslThreatLevel::High,
            register: register_os_npc_look_at,
        },
    ]
}

// -- osIsNpc(key) -> 0 --
fn register_os_is_npc(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _key: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osIsNpc called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osIsNpc", f)
}

// -- osNpcSaveAppearance(npc, notecard) -> null key --
fn register_os_npc_save_appearance(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _notecard): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcSaveAppearance called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osNpcSaveAppearance", f)
}

// -- osNpcLoadAppearance(npc, notecard) -> void --
fn register_os_npc_load_appearance(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _notecard): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcLoadAppearance called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcLoadAppearance", f)
}

// -- osNpcGetOwner(npc) -> null key --
fn register_os_npc_get_owner(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _npc: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcGetOwner called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osNpcGetOwner", f)
}

// -- osNpcGetPos(npc) -> zero vector "<0,0,0>" --
fn register_os_npc_get_pos(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _npc: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcGetPos called (stub)");
        Ok("<0, 0, 0>".to_string())
    })?;
    lua.globals().set("osNpcGetPos", f)
}

// -- osNpcMoveToTarget(npc, target, options) -> void --
fn register_os_npc_move_to_target(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _target, _options): (String, String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcMoveToTarget called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcMoveToTarget", f)
}

// -- osNpcGetRot(npc) -> identity rotation "<0,0,0,1>" --
fn register_os_npc_get_rot(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _npc: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcGetRot called (stub)");
        Ok("<0, 0, 0, 1>".to_string())
    })?;
    lua.globals().set("osNpcGetRot", f)
}

// -- osNpcSetRot(npc, rot) -> void --
fn register_os_npc_set_rot(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _rot): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcSetRot called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcSetRot", f)
}

// -- osNpcStopMoveToTarget(npc) -> void --
fn register_os_npc_stop_move_to_target(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _npc: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcStopMoveToTarget called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcStopMoveToTarget", f)
}

// -- osNpcSetProfileAbout(npc, text) -> void --
fn register_os_npc_set_profile_about(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _text): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcSetProfileAbout called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcSetProfileAbout", f)
}

// -- osNpcSetProfileImage(npc, texture) -> void --
fn register_os_npc_set_profile_image(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _texture): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcSetProfileImage called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcSetProfileImage", f)
}

// -- osNpcSayTo(npc, target, channel, msg) -> void --
fn register_os_npc_say_to(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (_npc, _target, _channel, _msg): (String, String, i32, String)| {
            let c = ctx.read();
            tracing::debug!(script_id = %c.script_id, "osNpcSayTo called (stub)");
            Ok(())
        },
    )?;
    lua.globals().set("osNpcSayTo", f)
}

// -- osNpcShout(npc, channel, msg) -> void --
fn register_os_npc_shout(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _channel, _msg): (String, i32, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcShout called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcShout", f)
}

// -- osNpcSit(npc, target, options) -> void --
fn register_os_npc_sit(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _target, _options): (String, String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcSit called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcSit", f)
}

// -- osNpcStand(npc) -> void --
fn register_os_npc_stand(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _npc: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcStand called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcStand", f)
}

// -- osNpcPlayAnimation(npc, anim) -> void --
fn register_os_npc_play_animation(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _anim): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcPlayAnimation called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcPlayAnimation", f)
}

// -- osNpcStopAnimation(npc, anim) -> void --
fn register_os_npc_stop_animation(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _anim): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcStopAnimation called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcStopAnimation", f)
}

// -- osNpcTouch(npc, object, link) -> void --
fn register_os_npc_touch(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _object, _link): (String, String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcTouch called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcTouch", f)
}

// -- osNpcWhisper(npc, channel, msg) -> void --
fn register_os_npc_whisper(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _channel, _msg): (String, i32, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcWhisper called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcWhisper", f)
}

// -- osNpcLookAt(npc, target) -> void --
fn register_os_npc_look_at(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_npc, _target): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osNpcLookAt called (stub)");
        Ok(())
    })?;
    lua.globals().set("osNpcLookAt", f)
}
