// SPDX-License-Identifier: GPL-3.0-or-later
//! Agent, avatar, and teleport stub functions (Task 9.1).

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;

pub(super) fn ossl_agent_funcs() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osGetAgents",
            level: OsslThreatLevel::Moderate,
            register: register_os_get_agents,
        },
        OsslFunc {
            name: "osGetAvatarList",
            level: OsslThreatLevel::None,
            register: register_os_get_avatar_list,
        },
        OsslFunc {
            name: "osGetNPCList",
            level: OsslThreatLevel::None,
            register: register_os_get_npc_list,
        },
        OsslFunc {
            name: "osSetOwnerSpeed",
            level: OsslThreatLevel::Moderate,
            register: register_os_set_owner_speed,
        },
        OsslFunc {
            name: "osGetHealth",
            level: OsslThreatLevel::None,
            register: register_os_get_health,
        },
        OsslFunc {
            name: "osCauseHealing",
            level: OsslThreatLevel::High,
            register: register_os_cause_healing,
        },
        OsslFunc {
            name: "osCauseDamage",
            level: OsslThreatLevel::High,
            register: register_os_cause_damage,
        },
        OsslFunc {
            name: "osSetHealth",
            level: OsslThreatLevel::High,
            register: register_os_set_health,
        },
        OsslFunc {
            name: "osSetHealRate",
            level: OsslThreatLevel::High,
            register: register_os_set_heal_rate,
        },
        OsslFunc {
            name: "osGetHealRate",
            level: OsslThreatLevel::None,
            register: register_os_get_heal_rate,
        },
        OsslFunc {
            name: "osKickAvatar",
            level: OsslThreatLevel::Severe,
            register: register_os_kick_avatar,
        },
        OsslFunc {
            name: "osForceOtherSit",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_force_other_sit,
        },
        OsslFunc {
            name: "osAvatarType",
            level: OsslThreatLevel::None,
            register: register_os_avatar_type,
        },
        OsslFunc {
            name: "osGetGender",
            level: OsslThreatLevel::None,
            register: register_os_get_gender,
        },
        OsslFunc {
            name: "osDetectedCountry",
            level: OsslThreatLevel::None,
            register: register_os_detected_country,
        },
        OsslFunc {
            name: "osGetAgentCountry",
            level: OsslThreatLevel::None,
            register: register_os_get_agent_country,
        },
        OsslFunc {
            name: "osAvatarPlayAnimation",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_avatar_play_animation,
        },
        OsslFunc {
            name: "osAvatarStopAnimation",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_avatar_stop_animation,
        },
        OsslFunc {
            name: "osLocalTeleportAgent",
            level: OsslThreatLevel::Moderate,
            register: register_os_local_teleport_agent,
        },
        OsslFunc {
            name: "osTeleportObject",
            level: OsslThreatLevel::Severe,
            register: register_os_teleport_object,
        },
    ]
}

fn register_os_get_agents(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetAgents called (stub)");
        lua.create_table()
    })?;
    lua.globals().set("osGetAgents", f)
}

fn register_os_get_avatar_list(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetAvatarList called (stub)");
        lua.create_table()
    })?;
    lua.globals().set("osGetAvatarList", f)
}

fn register_os_get_npc_list(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetNPCList called (stub)");
        lua.create_table()
    })?;
    lua.globals().set("osGetNPCList", f)
}

fn register_os_set_owner_speed(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _factor: f64| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetOwnerSpeed called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetOwnerSpeed", f)
}

fn register_os_get_health(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _avatar: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetHealth called (stub)");
        Ok(100.0f64)
    })?;
    lua.globals().set("osGetHealth", f)
}

fn register_os_cause_healing(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _amount): (String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osCauseHealing called (stub)");
        Ok(())
    })?;
    lua.globals().set("osCauseHealing", f)
}

fn register_os_cause_damage(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _amount): (String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osCauseDamage called (stub)");
        Ok(())
    })?;
    lua.globals().set("osCauseDamage", f)
}

fn register_os_set_health(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _health): (String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetHealth called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetHealth", f)
}

fn register_os_set_heal_rate(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _rate): (String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetHealRate called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetHealRate", f)
}

fn register_os_get_heal_rate(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _avatar: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetHealRate called (stub)");
        Ok(1.0f64)
    })?;
    lua.globals().set("osGetHealRate", f)
}

fn register_os_kick_avatar(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_first, _last, _msg): (String, String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osKickAvatar called (stub)");
        Ok(())
    })?;
    lua.globals().set("osKickAvatar", f)
}

fn register_os_force_other_sit(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _target): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceOtherSit called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceOtherSit", f)
}

fn register_os_avatar_type(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _avatar: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osAvatarType called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osAvatarType", f)
}

fn register_os_get_gender(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _avatar: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetGender called (stub)");
        Ok("unknown".to_string())
    })?;
    lua.globals().set("osGetGender", f)
}

fn register_os_detected_country(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _index: i32| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osDetectedCountry called (stub)");
        Ok(String::new())
    })?;
    lua.globals().set("osDetectedCountry", f)
}

fn register_os_get_agent_country(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _avatar: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetAgentCountry called (stub)");
        Ok(String::new())
    })?;
    lua.globals().set("osGetAgentCountry", f)
}

fn register_os_avatar_play_animation(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _anim): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osAvatarPlayAnimation called (stub)");
        Ok(())
    })?;
    lua.globals().set("osAvatarPlayAnimation", f)
}

fn register_os_avatar_stop_animation(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _anim): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osAvatarStopAnimation called (stub)");
        Ok(())
    })?;
    lua.globals().set("osAvatarStopAnimation", f)
}

fn register_os_local_teleport_agent(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (_agent, _pos, _vel, _lookat, _flags): (String, String, String, String, i32)| {
            let c = ctx.read();
            tracing::debug!(script_id = %c.script_id, "osLocalTeleportAgent called (stub)");
            Ok(())
        },
    )?;
    lua.globals().set("osLocalTeleportAgent", f)
}

fn register_os_teleport_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f =
        lua.create_function(
            move |_,
                  (_obj_key, _region_x, _region_y, _pos, _rot): (
                String,
                i32,
                i32,
                String,
                String,
            )| {
                let c = ctx.read();
                tracing::debug!(script_id = %c.script_id, "osTeleportObject called (stub)");
                Ok(0i32)
            },
        )?;
    lua.globals().set("osTeleportObject", f)
}
