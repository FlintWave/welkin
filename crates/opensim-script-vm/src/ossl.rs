// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSimulator OSSL extension functions for the Luau script VM.
//!
//! OSSL (OpenSim Scripting Language) extends LSL with os* functions that
//! give scripts access to simulator-specific features such as NPC control,
//! dynamic textures, teleportation, and region management.  Each function
//! carries a [`OsslThreatLevel`] that gates whether a particular region
//! configuration permits its use.
//!
//! Reference: `OpenSim/Region/ScriptEngine/Shared/Api/Implementation/OSSL_Api.cs`

use std::collections::HashSet;
use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::builtins::ScriptContext;

// ---------------------------------------------------------------------------
// OsslThreatLevel
// ---------------------------------------------------------------------------

/// Threat levels mirror the C# `ThreatLevel` enum used by OpenSimulator to
/// gate which OSSL functions a region permits.
///
/// The ordering is significant: a function is allowed when its level is
/// less than or equal to the configured maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OsslThreatLevel {
    /// No threat at all; no side effects.
    None = 0,
    /// Abuse can cause log spam or similar nuisance.
    Nuisance = 1,
    /// Extreme abuse can impair a region slightly.
    VeryLow = 2,
    /// Intentional abuse can cause recoverable crashes.
    Low = 3,
    /// Intentional abuse can cause denial of service with potential data loss.
    Moderate = 4,
    /// Casual abuse can impair functionality; intentional abuse causes data loss.
    High = 5,
    /// Normal use may cause service impairment depending on frequency.
    VeryHigh = 6,
    /// Even casual use is a danger to region stability.
    Severe = 7,
}

impl std::fmt::Display for OsslThreatLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::None => "None",
            Self::Nuisance => "Nuisance",
            Self::VeryLow => "VeryLow",
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "VeryHigh",
            Self::Severe => "Severe",
        };
        write!(f, "{name}")
    }
}

// ---------------------------------------------------------------------------
// OsslConfig
// ---------------------------------------------------------------------------

/// Runtime configuration that controls which OSSL functions a region
/// exposes to scripts.
#[derive(Debug, Clone)]
pub struct OsslConfig {
    /// The highest threat level that is implicitly allowed.
    pub max_threat_level: OsslThreatLevel,
    /// Function names that are explicitly allowed regardless of their
    /// threat level (e.g. `"osGetSimulatorVersion"`).
    pub allowed_functions: HashSet<String>,
}

impl Default for OsslConfig {
    fn default() -> Self {
        Self {
            max_threat_level: OsslThreatLevel::VeryLow,
            allowed_functions: HashSet::new(),
        }
    }
}

impl OsslConfig {
    /// Returns `true` if `func_name` at the given `level` is permitted
    /// under this configuration.
    pub fn is_allowed(&self, func_name: &str, level: OsslThreatLevel) -> bool {
        level <= self.max_threat_level || self.allowed_functions.contains(func_name)
    }
}

// ---------------------------------------------------------------------------
// Function registry  (name, threat-level, registration closure)
// ---------------------------------------------------------------------------

/// Describes one OSSL function for the purpose of registration.
struct OsslFunc {
    name: &'static str,
    level: OsslThreatLevel,
    /// Closure that registers the Lua global, capturing the context arc.
    register: fn(&Lua, &Arc<RwLock<ScriptContext>>) -> LuaResult<()>,
}

/// Master list of OSSL functions implemented by this module.
fn ossl_functions() -> Vec<OsslFunc> {
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
        // -- Draw-list helpers (None — pure string manipulation) --
        OsslFunc {
            name: "osDrawText",
            level: OsslThreatLevel::None,
            register: register_os_draw_text,
        },
        OsslFunc {
            name: "osDrawLine",
            level: OsslThreatLevel::None,
            register: register_os_draw_line,
        },
        OsslFunc {
            name: "osDrawRectangle",
            level: OsslThreatLevel::None,
            register: register_os_draw_rectangle,
        },
        OsslFunc {
            name: "osDrawEllipse",
            level: OsslThreatLevel::None,
            register: register_os_draw_ellipse,
        },
        OsslFunc {
            name: "osDrawImage",
            level: OsslThreatLevel::None,
            register: register_os_draw_image,
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
        // -- Weather (custom, from LSL corpus) --
        OsslFunc {
            name: "osWeatherMap",
            level: OsslThreatLevel::Low,
            register: register_os_weather_map,
        },
    ]
}

// ---------------------------------------------------------------------------
// Public registration entry point
// ---------------------------------------------------------------------------

/// Register OSSL functions into the Lua VM, gated by the provided config.
///
/// Only functions whose threat level is at or below `config.max_threat_level`,
/// or that appear in `config.allowed_functions`, are registered.  All others
/// are silently skipped.
pub fn register_ossl(
    lua: &Lua,
    config: &OsslConfig,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let globals = lua.globals();

    for func in ossl_functions() {
        if config.is_allowed(func.name, func.level) {
            (func.register)(lua, ctx)?;
            tracing::debug!(
                name = func.name,
                level = %func.level,
                "registered OSSL function"
            );
        } else {
            tracing::debug!(
                name = func.name,
                level = %func.level,
                max = %config.max_threat_level,
                "skipped OSSL function (threat level too high)"
            );
        }
    }

    // Register the OSSL version constant
    globals.set("OS_VERSION", "opensim-rust 0.1.0")?;

    Ok(())
}

/// Returns the list of OSSL function names and their threat levels.
///
/// Useful for tooling and introspection.
pub fn function_catalog() -> Vec<(&'static str, OsslThreatLevel)> {
    ossl_functions().into_iter().map(|f| (f.name, f.level)).collect()
}

// ---------------------------------------------------------------------------
// Individual function registrations
// ---------------------------------------------------------------------------
// Each function is a stub that logs the call via tracing::debug! and returns
// a sensible default.  Real implementations will be filled in as the
// corresponding subsystems (inventory, NPC manager, region services) are
// built.

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

fn register_os_set_dynamic_texture_url(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (content_type, url, _extra_params, _timer, _alpha): (String, String, String, i32, i32)| {
            let c = ctx.read();
            tracing::debug!(
                script_id = %c.script_id,
                content_type = %content_type,
                url = %url,
                "osSetDynamicTextureURL"
            );
            // Returns a UUID string for the texture; stub returns nil UUID.
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
        move |_, (content_type, data, _extra_params, _timer, _alpha): (String, String, String, i32, i32)| {
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

fn register_os_get_agent_ip(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, agent_id: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, agent_id = %agent_id, "osGetAgentIP");
        // Stub: return empty string (never leak real IPs from a stub).
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
    let f = lua.create_function(move |_, (region_name, _position, _look_at): (String, String, String)| {
        let c = ctx.read();
        tracing::debug!(
            script_id = %c.script_id,
            owner = %c.owner_id,
            region = %region_name,
            "osTeleportOwner"
        );
        Ok(())
    })?;
    lua.globals().set("osTeleportOwner", f)
}

fn register_os_region_restart(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, seconds: f64| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, delay_s = seconds, "osRegionRestart");
        Ok(())
    })?;
    lua.globals().set("osRegionRestart", f)
}

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
        Ok(1.0f64) // default speed
    })?;
    lua.globals().set("osGetSpeed", f)
}

fn register_os_npc_create(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (first_name, last_name, _position, _clone_from): (String, String, String, String)| {
            let c = ctx.read();
            tracing::debug!(
                script_id = %c.script_id,
                name = %format!("{first_name} {last_name}"),
                "osNpcCreate"
            );
            // Return a new UUID representing the NPC.
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

// -- Draw-list helpers (pure string manipulation, no side effects) -----------

fn register_os_draw_text(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, text): (String, String)| {
        Ok(format!("{draw_list}Text {text}; "))
    })?;
    lua.globals().set("osDrawText", f)
}

fn register_os_draw_line(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (draw_list, start_x, start_y, end_x, end_y): (String, i32, i32, i32, i32)| {
            Ok(format!(
                "{draw_list}MoveTo {start_x},{start_y}; LineTo {end_x},{end_y}; "
            ))
        },
    )?;
    lua.globals().set("osDrawLine", f)
}

fn register_os_draw_rectangle(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, width, height): (String, i32, i32)| {
        Ok(format!("{draw_list}Rectangle {width},{height}; "))
    })?;
    lua.globals().set("osDrawRectangle", f)
}

fn register_os_draw_ellipse(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (draw_list, width, height): (String, i32, i32)| {
        Ok(format!("{draw_list}Ellipse {width},{height}; "))
    })?;
    lua.globals().set("osDrawEllipse", f)
}

fn register_os_draw_image(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (draw_list, width, height, image_url): (String, i32, i32, String)| {
            Ok(format!(
                "{draw_list}Image {width},{height},{image_url}; "
            ))
        },
    )?;
    lua.globals().set("osDrawImage", f)
}

// -- Sun / environment -------------------------------------------------------

fn register_os_get_current_sun_hour(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| {
        tracing::debug!("osGetCurrentSunHour");
        // Stub: return noon.
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

// -- String helpers -----------------------------------------------------------

fn register_os_format_string(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    // osFormatString("{0} is {1}", ["Hello", "world"]) -> "Hello is world"
    // Uses C#-style {N} positional placeholders.
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
    // Simple hash function: returns a hex-string hash of the input.
    let f = lua.create_function(|_, input: String| {
        // Use a simple FNV-1a-style hash for determinism; not cryptographic.
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in input.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Ok(format!("{hash:016x}"))
    })?;
    lua.globals().set("osMakeHash", f)
}

// -- Weather ------------------------------------------------------------------

fn register_os_weather_map(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osWeatherMap");
        // Stub: return an empty string representing the weather map data.
        Ok(String::new())
    })?;
    lua.globals().set("osWeatherMap", f)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            ..ScriptContext::default()
        }))
    }

    fn make_vm() -> Lua {
        Lua::new()
    }

    // 1. OsslThreatLevel ordering
    #[test]
    fn threat_level_ordering() {
        assert!(OsslThreatLevel::None < OsslThreatLevel::Nuisance);
        assert!(OsslThreatLevel::Nuisance < OsslThreatLevel::VeryLow);
        assert!(OsslThreatLevel::VeryLow < OsslThreatLevel::Low);
        assert!(OsslThreatLevel::Low < OsslThreatLevel::Moderate);
        assert!(OsslThreatLevel::Moderate < OsslThreatLevel::High);
        assert!(OsslThreatLevel::High < OsslThreatLevel::VeryHigh);
        assert!(OsslThreatLevel::VeryHigh < OsslThreatLevel::Severe);

        // Equality
        assert_eq!(OsslThreatLevel::High, OsslThreatLevel::High);

        // Total count
        assert_eq!(OsslThreatLevel::Severe as u8, 7);
    }

    // 2. Config allows appropriate functions
    #[test]
    fn config_allows_appropriate_functions() {
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Moderate,
            allowed_functions: HashSet::new(),
        };

        // None through Moderate should be allowed
        assert!(config.is_allowed("osDrawText", OsslThreatLevel::None));
        assert!(config.is_allowed("osFormatString", OsslThreatLevel::VeryLow));
        assert!(config.is_allowed("osSetSpeed", OsslThreatLevel::Moderate));

        // High and above should be blocked
        assert!(!config.is_allowed("osNpcCreate", OsslThreatLevel::High));
        assert!(!config.is_allowed("osGetAgentIP", OsslThreatLevel::Severe));
    }

    // 3. osGetSimulatorVersion returns correct string
    #[test]
    fn os_get_simulator_version_returns_correct_string() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Severe,
            allowed_functions: HashSet::new(),
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        let version: String = lua
            .load("return osGetSimulatorVersion()")
            .eval()
            .unwrap();
        assert_eq!(version, "opensim-rust 0.1.0");
    }

    // 4. High threat function blocked at Low config
    #[test]
    fn high_threat_blocked_at_low_config() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Low,
            allowed_functions: HashSet::new(),
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        // osNpcCreate (High) should not be registered
        let result: LuaResult<mlua::Value> = lua.load("return osNpcCreate").eval();
        match result {
            Ok(mlua::Value::Nil) => {} // global not set -> nil
            Ok(mlua::Value::Function(_)) => panic!("osNpcCreate should not be registered at Low threat level"),
            Ok(_) => {} // nil or other non-function is fine
            Err(_) => {} // error accessing is also acceptable
        }

        // osGetSimulatorVersion (High) also blocked
        let result: LuaResult<mlua::Value> = lua.load("return osGetSimulatorVersion").eval();
        match result {
            Ok(mlua::Value::Function(_)) => {
                panic!("osGetSimulatorVersion should not be registered at Low threat level")
            }
            _ => {}
        }

        // But osDrawText (None) should work
        let result: String = lua
            .load(r#"return osDrawText("", "hello")"#)
            .eval()
            .unwrap();
        assert_eq!(result, "Text hello; ");
    }

    // 5. Function in allowed list overrides threat level
    #[test]
    fn allowed_list_overrides_threat_level() {
        let lua = make_vm();
        let ctx = make_ctx();

        let mut allowed = HashSet::new();
        allowed.insert("osGetSimulatorVersion".to_string());

        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::None,
            allowed_functions: allowed,
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        // osGetSimulatorVersion is High but explicitly allowed
        let version: String = lua
            .load("return osGetSimulatorVersion()")
            .eval()
            .unwrap();
        assert_eq!(version, "opensim-rust 0.1.0");

        // osNpcCreate (High) should still be blocked (not in allowed list)
        let result: LuaResult<mlua::Value> = lua.load("return osNpcCreate").eval();
        match result {
            Ok(mlua::Value::Function(_)) => panic!("osNpcCreate should still be blocked"),
            _ => {}
        }
    }

    // 6. Registration doesn't panic
    #[test]
    fn registration_does_not_panic() {
        let _lua = make_vm();
        let ctx = make_ctx();

        // Test with every possible threat level
        for level in [
            OsslThreatLevel::None,
            OsslThreatLevel::Nuisance,
            OsslThreatLevel::VeryLow,
            OsslThreatLevel::Low,
            OsslThreatLevel::Moderate,
            OsslThreatLevel::High,
            OsslThreatLevel::VeryHigh,
            OsslThreatLevel::Severe,
        ] {
            let config = OsslConfig {
                max_threat_level: level,
                allowed_functions: HashSet::new(),
            };
            let fresh_lua = make_vm();
            register_ossl(&fresh_lua, &config, &ctx).unwrap();
        }
    }

    // 7. Full roundtrip: create VM, register OSSL at Moderate, verify availability
    #[test]
    fn full_roundtrip_moderate_level() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Moderate,
            allowed_functions: HashSet::new(),
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        // Functions at or below Moderate should be available:

        // osDrawText (None) - draw-list helper
        let result: String = lua
            .load(r#"return osDrawText("prefix;", "hello")"#)
            .eval()
            .unwrap();
        assert_eq!(result, "prefix;Text hello; ");

        // osDrawLine (None)
        let result: String = lua
            .load(r#"return osDrawLine("", 0, 0, 100, 100)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "MoveTo 0,0; LineTo 100,100; ");

        // osDrawRectangle (None)
        let result: String = lua
            .load(r#"return osDrawRectangle("", 50, 30)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "Rectangle 50,30; ");

        // osDrawEllipse (None)
        let result: String = lua
            .load(r#"return osDrawEllipse("", 20, 10)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "Ellipse 20,10; ");

        // osFormatString (VeryLow)
        let result: String = lua
            .load(r#"return osFormatString("{0} is {1}", {"Hello", "world"})"#)
            .eval()
            .unwrap();
        assert_eq!(result, "Hello is world");

        // osSetSpeed (Moderate) - should be callable but is a stub
        let result = lua
            .load(r#"osSetSpeed("some-uuid", 2.0)"#)
            .exec();
        assert!(result.is_ok());

        // osGetSpeed (Moderate)
        let speed: f64 = lua
            .load(r#"return osGetSpeed("some-uuid")"#)
            .eval()
            .unwrap();
        assert!((speed - 1.0).abs() < f64::EPSILON);

        // osGetCurrentSunHour (None)
        let hour: f64 = lua
            .load("return osGetCurrentSunHour()")
            .eval()
            .unwrap();
        assert!((hour - 12.0).abs() < f64::EPSILON);

        // Functions above Moderate should NOT be available:

        // osNpcCreate (High)
        let v: mlua::Value = lua.load("return osNpcCreate").eval().unwrap();
        assert!(matches!(v, mlua::Value::Nil), "osNpcCreate should not be registered");

        // osGetSimulatorVersion (High)
        let v: mlua::Value = lua.load("return osGetSimulatorVersion").eval().unwrap();
        assert!(matches!(v, mlua::Value::Nil), "osGetSimulatorVersion should not be registered");

        // osGetAgentIP (Severe)
        let v: mlua::Value = lua.load("return osGetAgentIP").eval().unwrap();
        assert!(matches!(v, mlua::Value::Nil), "osGetAgentIP should not be registered");

        // osTeleportAgent (Severe)
        let v: mlua::Value = lua.load("return osTeleportAgent").eval().unwrap();
        assert!(matches!(v, mlua::Value::Nil), "osTeleportAgent should not be registered");

        // osRegionRestart (Severe)
        let v: mlua::Value = lua.load("return osRegionRestart").eval().unwrap();
        assert!(matches!(v, mlua::Value::Nil), "osRegionRestart should not be registered");

        // OS_VERSION constant should always be set
        let ver: String = lua.load("return OS_VERSION").eval().unwrap();
        assert_eq!(ver, "opensim-rust 0.1.0");
    }

    // 8. osFormatString basic formatting
    #[test]
    fn os_format_string_basic() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::VeryLow,
            allowed_functions: HashSet::new(),
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        // Simple substitution
        let r: String = lua
            .load(r#"return osFormatString("{0} {1}!", {"Hello", "World"})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "Hello World!");

        // Repeated placeholder
        let r: String = lua
            .load(r#"return osFormatString("{0} and {0}", {"echo"})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "echo and echo");

        // No placeholders
        let r: String = lua
            .load(r#"return osFormatString("plain text", {})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "plain text");

        // Multiple numbered placeholders out of order
        let r: String = lua
            .load(r#"return osFormatString("{1} before {0}", {"second", "first"})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "first before second");
    }

    // 9. osMakeHash produces deterministic output
    #[test]
    fn os_make_hash_deterministic() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::VeryLow,
            allowed_functions: HashSet::new(),
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        let h1: String = lua
            .load(r#"return osMakeHash("test")"#)
            .eval()
            .unwrap();
        let h2: String = lua
            .load(r#"return osMakeHash("test")"#)
            .eval()
            .unwrap();
        assert_eq!(h1, h2, "same input must produce same hash");
        assert_eq!(h1.len(), 16, "hash should be 16 hex chars");

        // Different input should produce different hash
        let h3: String = lua
            .load(r#"return osMakeHash("different")"#)
            .eval()
            .unwrap();
        assert_ne!(h1, h3, "different inputs should produce different hashes");
    }

    // 10. Draw functions chain correctly
    #[test]
    fn draw_functions_chain() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::None,
            allowed_functions: HashSet::new(),
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        // Chain multiple draw operations like a real script would
        let result: String = lua
            .load(
                r#"
                local dl = ""
                dl = osDrawLine(dl, 0, 0, 100, 100)
                dl = osDrawRectangle(dl, 50, 50)
                dl = osDrawText(dl, "label")
                return dl
            "#,
            )
            .eval()
            .unwrap();

        assert_eq!(
            result,
            "MoveTo 0,0; LineTo 100,100; Rectangle 50,50; Text label; "
        );
    }

    // 11. function_catalog returns all functions
    #[test]
    fn function_catalog_completeness() {
        let catalog = function_catalog();
        assert!(catalog.len() >= 25, "expected at least 25 OSSL functions, got {}", catalog.len());

        // Spot-check some entries
        let names: Vec<&str> = catalog.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"osGetSimulatorVersion"));
        assert!(names.contains(&"osNpcCreate"));
        assert!(names.contains(&"osDrawText"));
        assert!(names.contains(&"osFormatString"));
        assert!(names.contains(&"osRegionRestart"));
    }

    // 12. Default config blocks High functions
    #[test]
    fn default_config_blocks_high() {
        let config = OsslConfig::default();
        assert_eq!(config.max_threat_level, OsslThreatLevel::VeryLow);

        assert!(config.is_allowed("osDrawText", OsslThreatLevel::None));
        assert!(config.is_allowed("osFormatString", OsslThreatLevel::VeryLow));
        assert!(!config.is_allowed("osSetSpeed", OsslThreatLevel::Moderate));
        assert!(!config.is_allowed("osNpcCreate", OsslThreatLevel::High));
        assert!(!config.is_allowed("osGetAgentIP", OsslThreatLevel::Severe));
    }
}
