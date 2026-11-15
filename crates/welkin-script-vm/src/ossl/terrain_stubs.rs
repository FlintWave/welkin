// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain, environment, and parcel stub functions (Task 10.1).

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;

pub(super) fn ossl_terrain_funcs() -> Vec<OsslFunc> {
    vec![
        // Terrain (High)
        OsslFunc {
            name: "osTerrainSetHeight",
            level: OsslThreatLevel::High,
            register: register_os_terrain_set_height,
        },
        OsslFunc {
            name: "osSetTerrainHeight",
            level: OsslThreatLevel::High,
            register: register_os_set_terrain_height,
        },
        OsslFunc {
            name: "osTerrainGetHeight",
            level: OsslThreatLevel::High,
            register: register_os_terrain_get_height,
        },
        OsslFunc {
            name: "osGetTerrainHeight",
            level: OsslThreatLevel::High,
            register: register_os_get_terrain_height,
        },
        OsslFunc {
            name: "osTerrainFlush",
            level: OsslThreatLevel::High,
            register: register_os_terrain_flush,
        },
        // Environment/sun/wind
        OsslFunc {
            name: "osSunGetParam",
            level: OsslThreatLevel::None,
            register: register_os_sun_get_param,
        },
        OsslFunc {
            name: "osSetEstateSunSettings",
            level: OsslThreatLevel::Moderate,
            register: register_os_set_estate_sun_settings,
        },
        OsslFunc {
            name: "osSetRegionSunSettings",
            level: OsslThreatLevel::Moderate,
            register: register_os_set_region_sun_settings,
        },
        OsslFunc {
            name: "osGetWindParam",
            level: OsslThreatLevel::None,
            register: register_os_get_wind_param,
        },
        OsslFunc {
            name: "osSetWindParam",
            level: OsslThreatLevel::VeryLow,
            register: register_os_set_wind_param,
        },
        OsslFunc {
            name: "osGetRegionWaterHeight",
            level: OsslThreatLevel::None,
            register: register_os_get_region_water_height,
        },
        // Parcel/land
        OsslFunc {
            name: "osParcelSetDetails",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_parcel_set_details,
        },
        OsslFunc {
            name: "osGetParcelDetails",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_get_parcel_details,
        },
        OsslFunc {
            name: "osGetParcelDwell",
            level: OsslThreatLevel::None,
            register: register_os_get_parcel_dwell,
        },
        OsslFunc {
            name: "osGetParcelPrimCount",
            level: OsslThreatLevel::None,
            register: register_os_get_parcel_prim_count,
        },
        OsslFunc {
            name: "osSetParcelDetails",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_set_parcel_details,
        },
        // Media
        OsslFunc {
            name: "osSetParcelMusicURL",
            level: OsslThreatLevel::VeryLow,
            register: register_os_set_parcel_music_url,
        },
        OsslFunc {
            name: "osSetParcelSIPAddress",
            level: OsslThreatLevel::Moderate,
            register: register_os_set_parcel_sip_address,
        },
        OsslFunc {
            name: "osSetPrimFloatOnWater",
            level: OsslThreatLevel::VeryLow,
            register: register_os_set_prim_float_on_water,
        },
    ]
}

// -- Terrain --

fn register_os_terrain_set_height(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_x, _y, _val): (i32, i32, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osTerrainSetHeight called (stub)");
        Ok(())
    })?;
    lua.globals().set("osTerrainSetHeight", f)
}

fn register_os_set_terrain_height(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_x, _y, _val): (i32, i32, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetTerrainHeight called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetTerrainHeight", f)
}

fn register_os_terrain_get_height(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_x, _y): (i32, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osTerrainGetHeight called (stub)");
        Ok(25.0f64)
    })?;
    lua.globals().set("osTerrainGetHeight", f)
}

fn register_os_get_terrain_height(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_x, _y): (i32, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetTerrainHeight called (stub)");
        Ok(25.0f64)
    })?;
    lua.globals().set("osGetTerrainHeight", f)
}

fn register_os_terrain_flush(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osTerrainFlush called (stub)");
        Ok(())
    })?;
    lua.globals().set("osTerrainFlush", f)
}

// -- Environment/sun/wind --

fn register_os_sun_get_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _param: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSunGetParam called (stub)");
        Ok(6.0f64)
    })?;
    lua.globals().set("osSunGetParam", f)
}

fn register_os_set_estate_sun_settings(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_world, _fixed, _hour): (i32, i32, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetEstateSunSettings called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetEstateSunSettings", f)
}

fn register_os_set_region_sun_settings(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_,
              (_fixed, _sun_hour, _day_length, _year_length, _day_offset): (
            i32,
            f64,
            f64,
            f64,
            f64,
        )| {
            let c = ctx.read();
            tracing::debug!(script_id = %c.script_id, "osSetRegionSunSettings called (stub)");
            Ok(())
        },
    )?;
    lua.globals().set("osSetRegionSunSettings", f)
}

fn register_os_get_wind_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_plugin, _param): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetWindParam called (stub)");
        Ok(0.0f64)
    })?;
    lua.globals().set("osGetWindParam", f)
}

fn register_os_set_wind_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_plugin, _param, _value): (String, String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetWindParam called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetWindParam", f)
}

fn register_os_get_region_water_height(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetRegionWaterHeight called (stub)");
        Ok(20.0f64)
    })?;
    lua.globals().set("osGetRegionWaterHeight", f)
}

// -- Parcel/land --

fn register_os_parcel_set_details(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_pos, _rules): (String, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osParcelSetDetails called (stub)");
        Ok(())
    })?;
    lua.globals().set("osParcelSetDetails", f)
}

fn register_os_get_parcel_details(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, (_pos, _params): (String, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetParcelDetails called (stub)");
        lua.create_table()
    })?;
    lua.globals().set("osGetParcelDetails", f)
}

fn register_os_get_parcel_dwell(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _pos: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetParcelDwell called (stub)");
        Ok(0.0f64)
    })?;
    lua.globals().set("osGetParcelDwell", f)
}

fn register_os_get_parcel_prim_count(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _pos: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetParcelPrimCount called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osGetParcelPrimCount", f)
}

fn register_os_set_parcel_details(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_pos, _rules): (String, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetParcelDetails called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetParcelDetails", f)
}

// -- Media --

fn register_os_set_parcel_music_url(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _url: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetParcelMusicURL called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetParcelMusicURL", f)
}

fn register_os_set_parcel_sip_address(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _addr: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetParcelSIPAddress called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetParcelSIPAddress", f)
}

fn register_os_set_prim_float_on_water(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _enable: i32| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetPrimFloatOnWater called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetPrimFloatOnWater", f)
}
