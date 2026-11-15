// SPDX-License-Identifier: LGPL-3.0-or-later
//! Stub OSSL functions: notecard, texture, parcel, agent, teleport, NPC,
//! region management, avatar speed, sun/environment, string helpers.
//!
//! These log via `tracing::debug!` and return sensible defaults until the
//! corresponding subsystems are built.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;

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
        // -- Region --
        OsslFunc {
            name: "osRegionNotice",
            level: OsslThreatLevel::Severe,
            register: register_os_region_notice,
        },
        // -- Terrain texture --
        OsslFunc {
            name: "osSetTerrainTexture",
            level: OsslThreatLevel::High,
            register: register_os_set_terrain_texture,
        },
        OsslFunc {
            name: "osSetTerrainTextureHeight",
            level: OsslThreatLevel::High,
            register: register_os_set_terrain_texture_height,
        },
        OsslFunc {
            name: "osGetSunParam",
            level: OsslThreatLevel::None,
            register: register_os_get_sun_param,
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
        if let Some(asset_id) = c.notecard_names.get(&name) {
            if let Some(text) = c.notecard_data.get(asset_id) {
                return Ok(text.clone());
            }
        }
        for text in c.notecard_data.values() {
            return Ok(text.clone());
        }
        Ok(String::new())
    })?;
    lua.globals().set("osGetNotecard", f)
}

fn register_os_get_notecard_line(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (name, line): (String, i32)| {
        let c = ctx.read();
        let text = if let Some(asset_id) = c.notecard_names.get(&name) {
            c.notecard_data.get(asset_id).cloned()
        } else {
            c.notecard_data.values().next().cloned()
        };
        if let Some(text) = text {
            let lines: Vec<&str> = text.lines().collect();
            if line >= 0 && (line as usize) < lines.len() {
                return Ok(lines[line as usize].to_string());
            }
        }
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
        let text = if let Some(asset_id) = c.notecard_names.get(&name) {
            c.notecard_data.get(asset_id).cloned()
        } else {
            c.notecard_data.values().next().cloned()
        };
        Ok(text.map_or(0i32, |t| t.lines().count() as i32))
    })?;
    lua.globals().set("osGetNumberOfNotecardLines", f)
}

fn register_os_make_notecard(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (name, contents): (String, String)| {
        let mut c = ctx.write();
        let asset_id = Uuid::new_v4();
        c.notecard_data.insert(asset_id, contents);
        c.notecard_names.insert(name.clone(), asset_id);
        tracing::info!(notecard = %name, asset = %asset_id, "osMakeNotecard created");
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
              (content_type, url, extra_params, timer, alpha): (
            String,
            String,
            String,
            i32,
            i32,
        )| {
            let mut c = ctx.write();
            let texture_id = Uuid::new_v4();
            c.texture = texture_id;
            if let Some(ref sender) = c.dynamic_texture_sender {
                let _ = sender.send(crate::builtins::DynamicTextureCommand::FromUrl {
                    prim_id: c.prim_id,
                    content_type: content_type.clone(),
                    url: url.clone(),
                    extra_params,
                    timer,
                    alpha,
                    texture_id,
                });
            }
            tracing::info!(
                script_id = %c.script_id,
                content_type = %content_type,
                url = %url,
                texture = %texture_id,
                "osSetDynamicTextureURL"
            );
            Ok(texture_id.to_string())
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
              (content_type, data, extra_params, timer, alpha): (
            String,
            String,
            String,
            i32,
            i32,
        )| {
            let mut c = ctx.write();
            let texture_id = Uuid::new_v4();
            c.texture = texture_id;
            if let Some(ref sender) = c.dynamic_texture_sender {
                let _ = sender.send(crate::builtins::DynamicTextureCommand::FromData {
                    prim_id: c.prim_id,
                    content_type: content_type.clone(),
                    data: data.clone(),
                    extra_params,
                    timer,
                    alpha,
                    texture_id,
                });
            }
            tracing::info!(
                script_id = %c.script_id,
                content_type = %content_type,
                data_len = data.len(),
                texture = %texture_id,
                "osSetDynamicTextureData"
            );
            Ok(texture_id.to_string())
        },
    )?;
    lua.globals().set("osSetDynamicTextureData", f)
}

// ---------------------------------------------------------------------------
// Parcel media
// ---------------------------------------------------------------------------

fn register_os_set_parcel_media_url(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, url: String| {
        let mut c = ctx.write();
        c.parcel_media_url = url.clone();
        tracing::info!(script_id = %c.script_id, url = %url, "osSetParcelMediaURL");
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
        let uuid = Uuid::parse_str(&agent_id).unwrap_or(Uuid::nil());
        if uuid == Uuid::nil() {
            return Ok(String::new());
        }
        if let Some(presence) = c.presence_list.iter().find(|p| p.uuid == uuid) {
            Ok(presence.ip_address.clone())
        } else {
            Ok(String::new())
        }
    })?;
    lua.globals().set("osGetAgentIP", f)
}

fn parse_vector(s: &str) -> [f64; 3] {
    let s = s.trim().trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 3 {
        [
            parts[0].trim().parse::<f64>().unwrap_or(0.0),
            parts[1].trim().parse::<f64>().unwrap_or(0.0),
            parts[2].trim().parse::<f64>().unwrap_or(0.0),
        ]
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn register_os_teleport_agent(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (agent_id, region_name, position, look_at): (String, String, String, String)| {
            let c = ctx.read();
            let agent = Uuid::parse_str(&agent_id).unwrap_or(Uuid::nil());
            let pos = parse_vector(&position);
            let look = parse_vector(&look_at);
            if let Some(ref sender) = c.teleport_sender {
                let _ = sender.send(crate::builtins::TeleportRequest {
                    agent_id: agent,
                    region_name: region_name.clone(),
                    position: pos,
                    look_at: look,
                });
            }
            tracing::info!(
                script_id = %c.script_id,
                agent = %agent_id,
                region = %region_name,
                pos = ?pos,
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
        move |_, (region_name, position, look_at): (String, String, String)| {
            let c = ctx.read();
            let pos = parse_vector(&position);
            let look = parse_vector(&look_at);
            if let Some(ref sender) = c.teleport_sender {
                let _ = sender.send(crate::builtins::TeleportRequest {
                    agent_id: c.owner_id,
                    region_name: region_name.clone(),
                    position: pos,
                    look_at: look,
                });
            }
            tracing::info!(
                script_id = %c.script_id,
                owner = %c.owner_id,
                region = %region_name,
                pos = ?pos,
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
        if let Some(ref sender) = c.restart_sender {
            if seconds < 15.0 {
                // Abort restart (like OpenSim)
                let _ = sender.send(crate::builtins::RegionRestartCommand {
                    delay_seconds: 0.0,
                    message: "Region restart has been aborted".to_string(),
                    requester_id: c.owner_id,
                });
                return Ok(1i32);
            }
            let _ = sender.send(crate::builtins::RegionRestartCommand {
                delay_seconds: seconds,
                message: String::new(),
                requester_id: c.owner_id,
            });
            tracing::info!(
                script_id = %c.script_id,
                delay_s = seconds,
                "osRegionRestart scheduled"
            );
            Ok(1i32)
        } else {
            Ok(0i32)
        }
    })?;
    lua.globals().set("osRegionRestart", f)
}

// ---------------------------------------------------------------------------
// Avatar speed
// ---------------------------------------------------------------------------

fn register_os_set_speed(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_agent_id, speed): (String, f64)| {
        let mut c = ctx.write();
        c.speed_multiplier = speed.max(0.0).min(10.0);
        tracing::info!(speed = c.speed_multiplier, "osSetSpeed");
        Ok(())
    })?;
    lua.globals().set("osSetSpeed", f)
}

fn register_os_get_speed(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _agent_id: String| {
        let c = ctx.read();
        Ok(c.speed_multiplier)
    })?;
    lua.globals().set("osGetSpeed", f)
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
        Ok("welkin 0.1.0".to_string())
    })?;
    lua.globals().set("osGetSimulatorVersion", f)
}

// ---------------------------------------------------------------------------
// Sun / environment
// ---------------------------------------------------------------------------

fn register_os_get_current_sun_hour(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        Ok(c.sun_hour)
    })?;
    lua.globals().set("osGetCurrentSunHour", f)
}

fn register_os_set_sun_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (param, value): (String, f64)| {
        let mut c = ctx.write();
        match param.to_lowercase().as_str() {
            "currenthour" | "day_length" => c.sun_hour = value.clamp(0.0, 24.0),
            "year_length" | "day_night_offset" => {}
            _ => {}
        }
        tracing::info!(param = %param, value, "osSetSunParam");
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
        let mut c = ctx.write();
        c.water_height = height.clamp(0.0, 1000.0);
        tracing::info!(height = c.water_height, "osSetRegionWaterHeight");
        Ok(())
    })?;
    lua.globals().set("osSetRegionWaterHeight", f)
}

// ---------------------------------------------------------------------------
// Region
// ---------------------------------------------------------------------------

fn register_os_region_notice(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, message: String| {
        let c = ctx.read();
        if let Some(ref sender) = c.chat_sender {
            let _ = sender.send(crate::ChatMessage {
                source_id: c.prim_id,
                source_name: "Region".to_string(),
                source_pos: [128.0, 128.0, 0.0],
                owner_id: c.owner_id,
                channel: -1,
                message,
                chat_type: crate::ChatType::Region,
                target_id: None,
            });
        }
        Ok(())
    })?;
    lua.globals().set("osRegionNotice", f)
}

// ---------------------------------------------------------------------------
// Terrain texture (not in fork files)
// ---------------------------------------------------------------------------

fn register_os_set_terrain_texture(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (corner, texture): (i32, String)| {
        let mut c = ctx.write();
        let idx = (corner.clamp(0, 3)) as usize;
        c.terrain_textures[idx] = Uuid::parse_str(&texture).unwrap_or(Uuid::nil());
        c.terrain_dirty = true;
        Ok(())
    })?;
    lua.globals().set("osSetTerrainTexture", f)
}

fn register_os_set_terrain_texture_height(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (corner, low, high): (i32, f64, f64)| {
        let mut c = ctx.write();
        let idx = (corner.clamp(0, 3)) as usize;
        c.terrain_texture_heights[idx] = [low, high];
        c.terrain_dirty = true;
        Ok(())
    })?;
    lua.globals().set("osSetTerrainTextureHeight", f)
}

fn register_os_get_sun_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, param: String| {
        let c = ctx.read();
        let val = match param.to_lowercase().as_str() {
            "currenthour" | "day_length" => c.sun_hour,
            _ => 0.0,
        };
        Ok(val)
    })?;
    lua.globals().set("osGetSunParam", f)
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
