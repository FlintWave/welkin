// SPDX-License-Identifier: GPL-3.0-or-later
//! Stub OSSL functions: notecard, texture, parcel, agent, teleport, NPC,
//! region management, avatar speed, sun/environment, string helpers.
//!
//! These log via `tracing::debug!` and return sensible defaults until the
//! corresponding subsystems are built.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::builtins::ScriptContext;
use super::{OsslFunc, OsslThreatLevel};

pub(crate) fn stub_functions() -> Vec<OsslFunc> {
    vec![
        // -- Notecard functions (VeryHigh) --
        OsslFunc {
            name: "osGetNotecard",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_get_notecard,
        },
        OsslFunc {
            name: "osGetNotecardLine",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_get_notecard_line,
        },
        OsslFunc {
            name: "osGetNumberOfNotecardLines",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_get_number_of_notecard_lines,
        },
        OsslFunc {
            name: "osMakeNotecard",
            level: OsslThreatLevel::High,
            register: register_os_make_notecard,
        },
        // -- Dynamic texture (VeryHigh / VeryLow) --
        OsslFunc {
            name: "osSetDynamicTextureURL",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_set_dynamic_texture_url,
        },
        OsslFunc {
            name: "osSetDynamicTextureData",
            level: OsslThreatLevel::VeryLow,
            register: register_os_set_dynamic_texture_data,
        },
        // -- Parcel media (VeryLow) --
        OsslFunc {
            name: "osSetParcelMediaURL",
            level: OsslThreatLevel::VeryLow,
            register: register_os_set_parcel_media_url,
        },
        // -- Agent / teleport --
        OsslFunc {
            name: "osGetAgentIP",
            level: OsslThreatLevel::Severe,
            register: register_os_get_agent_ip,
        },
        OsslFunc {
            name: "osTeleportAgent",
            level: OsslThreatLevel::Severe,
            register: register_os_teleport_agent,
        },
        OsslFunc {
            name: "osTeleportOwner",
            level: OsslThreatLevel::None,
            register: register_os_teleport_owner,
        },
        // -- Region management --
        OsslFunc {
            name: "osRegionRestart",
            level: OsslThreatLevel::Severe,
            register: register_os_region_restart,
        },
        // -- Avatar speed (Moderate) --
        OsslFunc {
            name: "osSetSpeed",
            level: OsslThreatLevel::Moderate,
            register: register_os_set_speed,
        },
        OsslFunc {
            name: "osGetSpeed",
            level: OsslThreatLevel::Moderate,
            register: register_os_get_speed,
        },
        // -- NPC functions (High) --
        OsslFunc {
            name: "osNpcCreate",
            level: OsslThreatLevel::High,
            register: register_os_npc_create,
        },
        OsslFunc {
            name: "osNpcRemove",
            level: OsslThreatLevel::High,
            register: register_os_npc_remove,
        },
        OsslFunc {
            name: "osNpcMoveTo",
            level: OsslThreatLevel::High,
            register: register_os_npc_move_to,
        },
        OsslFunc {
            name: "osNpcSay",
            level: OsslThreatLevel::High,
            register: register_os_npc_say,
        },
        // -- Simulator info (High) --
        OsslFunc {
            name: "osGetSimulatorVersion",
            level: OsslThreatLevel::High,
            register: register_os_get_simulator_version,
        },
        // -- Sun / environment --
        OsslFunc {
            name: "osGetCurrentSunHour",
            level: OsslThreatLevel::None,
            register: register_os_get_current_sun_hour,
        },
        OsslFunc {
            name: "osSetSunParam",
            level: OsslThreatLevel::None,
            register: register_os_set_sun_param,
        },
        OsslFunc {
            name: "osSetRegionWaterHeight",
            level: OsslThreatLevel::High,
            register: register_os_set_region_water_height,
        },
        // -- String helpers (VeryLow) --
        OsslFunc {
            name: "osFormatString",
            level: OsslThreatLevel::VeryLow,
            register: register_os_format_string,
        },
        OsslFunc {
            name: "osMakeHash",
            level: OsslThreatLevel::VeryLow,
            register: register_os_make_hash,
        },
    ]
}

// ---------------------------------------------------------------------------
// Notecard stubs
// ---------------------------------------------------------------------------

fn register_os_get_notecard(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, notecard = %name, "osGetNotecard");
        Ok(String::new())
    })?;
    lua.globals().set("osGetNotecard", f)
}

fn register_os_get_notecard_line(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (name, line): (String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, notecard = %name, line, "osGetNotecardLine");
        Ok(String::new())
    })?;
    lua.globals().set("osGetNotecardLine", f)
}

fn register_os_get_number_of_notecard_lines(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, notecard = %name, "osGetNumberOfNotecardLines");
        Ok(0i32)
    })?;
    lua.globals().set("osGetNumberOfNotecardLines", f)
}

fn register_os_make_notecard(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (name, contents): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, notecard = %name, len = contents.len(), "osMakeNotecard");
        Ok(())
    })?;
    lua.globals().set("osMakeNotecard", f)
}

// ---------------------------------------------------------------------------
// Dynamic texture stubs
// ---------------------------------------------------------------------------

fn register_os_set_dynamic_texture_url(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_,
              (content_type, url, _extra_params, _timer, _alpha): (
            String,
            String,
            String,
            i32,
            i32,
        )| {
            let c = ctx.read();
            tracing::debug!(
                script_id = %c.script_id,
                content_type = %content_type,
                url = %url,
                "osSetDynamicTextureURL"
            );
            Ok(Uuid::nil().to_string())
        },
    )?;
    lua.globals().set("osSetDynamicTextureURL", f)
}

fn register_os_set_dynamic_texture_data(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_,
              (content_type, data, _extra_params, _timer, _alpha): (
            String,
            String,
            String,
            i32,
            i32,
        )| {
            let c = ctx.read();
            tracing::debug!(
                script_id = %c.script_id,
                content_type = %content_type,
                data_len = data.len(),
                "osSetDynamicTextureData"
            );
            Ok(Uuid::nil().to_string())
        },
    )?;
    lua.globals().set("osSetDynamicTextureData", f)
}

// ---------------------------------------------------------------------------
// Parcel media
// ---------------------------------------------------------------------------

fn register_os_set_parcel_media_url(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, url: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, url = %url, "osSetParcelMediaURL");
        Ok(())
    })?;
    lua.globals().set("osSetParcelMediaURL", f)
}

// ---------------------------------------------------------------------------
// Agent / teleport
// ---------------------------------------------------------------------------

fn register_os_get_agent_ip(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, agent_id: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, agent_id = %agent_id, "osGetAgentIP");
        Ok(String::new())
    })?;
    lua.globals().set("osGetAgentIP", f)
}

fn register_os_teleport_agent(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (agent_id, region_name, _position, _look_at): (String, String, String, String)| {
            let c = ctx.read();
            tracing::debug!(
                script_id = %c.script_id,
                agent = %agent_id,
                region = %region_name,
                "osTeleportAgent"
            );
            Ok(())
        },
    )?;
    lua.globals().set("osTeleportAgent", f)
}

fn register_os_teleport_owner(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (region_name, _position, _look_at): (String, String, String)| {
            let c = ctx.read();
            tracing::debug!(
                script_id = %c.script_id,
                owner = %c.owner_id,
                region = %region_name,
                "osTeleportOwner"
            );
            Ok(())
        },
    )?;
    lua.globals().set("osTeleportOwner", f)
}

// ---------------------------------------------------------------------------
// Region management
// ---------------------------------------------------------------------------

fn register_os_region_restart(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, seconds: f64| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, delay_s = seconds, "osRegionRestart");
        Ok(())
    })?;
    lua.globals().set("osRegionRestart", f)
}

// ---------------------------------------------------------------------------
// Avatar speed
// ---------------------------------------------------------------------------

fn register_os_set_speed(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (agent_id, speed): (String, f64)| {
        let c = ctx.read();
        tracing::debug!(
            script_id = %c.script_id,
            agent = %agent_id,
            speed,
            "osSetSpeed"
        );
        Ok(())
    })?;
    lua.globals().set("osSetSpeed", f)
}

fn register_os_get_speed(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, agent_id: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, agent = %agent_id, "osGetSpeed");
        Ok(1.0f64)
    })?;
    lua.globals().set("osGetSpeed", f)
}

// ---------------------------------------------------------------------------
// NPC functions
// ---------------------------------------------------------------------------

fn register_os_npc_create(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_,
              (first_name, last_name, _position, _clone_from): (String, String, String, String)| {
            let c = ctx.read();
            tracing::debug!(
                script_id = %c.script_id,
                name = %format!("{first_name} {last_name}"),
                "osNpcCreate"
            );
            Ok(Uuid::new_v4().to_string())
        },
    )?;
    lua.globals().set("osNpcCreate", f)
}

fn register_os_npc_remove(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc_id: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, npc = %npc_id, "osNpcRemove");
        Ok(())
    })?;
    lua.globals().set("osNpcRemove", f)
}

fn register_os_npc_move_to(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc_id, position): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, npc = %npc_id, pos = %position, "osNpcMoveTo");
        Ok(())
    })?;
    lua.globals().set("osNpcMoveTo", f)
}

fn register_os_npc_say(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc_id, channel, message): (String, i32, String)| {
        let c = ctx.read();
        tracing::debug!(
            script_id = %c.script_id,
            npc = %npc_id,
            channel,
            message = %message,
            "osNpcSay"
        );
        Ok(())
    })?;
    lua.globals().set("osNpcSay", f)
}

// ---------------------------------------------------------------------------
// Simulator info
// ---------------------------------------------------------------------------

fn register_os_get_simulator_version(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| {
        tracing::debug!("osGetSimulatorVersion");
        Ok("opensim-rust 0.1.0".to_string())
    })?;
    lua.globals().set("osGetSimulatorVersion", f)
}

// ---------------------------------------------------------------------------
// Sun / environment
// ---------------------------------------------------------------------------

fn register_os_get_current_sun_hour(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| {
        tracing::debug!("osGetCurrentSunHour");
        Ok(12.0f64)
    })?;
    lua.globals().set("osGetCurrentSunHour", f)
}

fn register_os_set_sun_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (param, value): (String, f64)| {
        let c = ctx.read();
        tracing::debug!(
            script_id = %c.script_id,
            param = %param,
            value,
            "osSetSunParam"
        );
        Ok(())
    })?;
    lua.globals().set("osSetSunParam", f)
}

fn register_os_set_region_water_height(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, height: f64| {
        let c = ctx.read();
        tracing::debug!(
            script_id = %c.script_id,
            height,
            "osSetRegionWaterHeight"
        );
        Ok(())
    })?;
    lua.globals().set("osSetRegionWaterHeight", f)
}

// ---------------------------------------------------------------------------
// String helpers
// ---------------------------------------------------------------------------

fn register_os_format_string(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (fmt_str, args): (String, Vec<String>)| {
        let mut result = fmt_str;
        for (i, arg) in args.iter().enumerate() {
            let placeholder = format!("{{{i}}}");
            result = result.replace(&placeholder, arg);
        }
        Ok(result)
    })?;
    lua.globals().set("osFormatString", f)
}

fn register_os_make_hash(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, input: String| {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in input.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Ok(format!("{hash:016x}"))
    })?;
    lua.globals().set("osMakeHash", f)
}
