// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain, environment, parcel, and media OSSL functions.
//!
//! Ported from `OpenSim/Region/ScriptEngine/Shared/Api/Implementation/OSSL_Api.cs`.
//! Each function operates on the shared [`ScriptContext`] that represents the
//! region state visible to a running script.

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;

/// Default terrain height when no heightmap data has been loaded.
/// Matches `welkin_scene::terrain::DEFAULT_TERRAIN_HEIGHT`.
const DEFAULT_TERRAIN_HEIGHT: f32 = 21.0;

/// Maximum terrain height (matches `welkin_scene::terrain::REGION_HEIGHT_LIMIT`).
const REGION_HEIGHT_LIMIT: f32 = 4096.0;

/// Minimum interval (seconds) between successive `osTerrainFlush` calls.
/// Matches the 60-second cooldown in the C# implementation.
const TERRAIN_FLUSH_COOLDOWN: f64 = 60.0;

/// Default day length in seconds (4 real-time hours = 1 virtual day).
/// Matches the C# fallback when no environment module is present.
#[cfg(test)]
const DEFAULT_DAY_LENGTH: f64 = 14400.0;

// Parcel-detail codes matching C# `ScriptBaseClass.PARCEL_DETAILS_*`.
const PARCEL_DETAILS_NAME: i64 = 0;
const PARCEL_DETAILS_DESC: i64 = 1;
const PARCEL_DETAILS_OWNER: i64 = 2;
const PARCEL_DETAILS_GROUP: i64 = 3;
const PARCEL_DETAILS_AREA: i64 = 4;
const PARCEL_DETAILS_ID: i64 = 5;
const PARCEL_DETAILS_SEE_AVATARS: i64 = 6;

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Current time as fractional seconds since the UNIX epoch.
fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Read terrain height at `(x, y)`.  Returns `None` when coordinates are
/// outside the region bounds.  When the heightmap has not been initialised
/// the default terrain height (21 m) is returned for every in-bounds cell.
fn get_terrain_height(ctx: &ScriptContext, x: i32, y: i32) -> Option<f64> {
    let sx = ctx.region_size[0];
    let sy = ctx.region_size[1];
    if x < 0 || x >= sx || y < 0 || y >= sy {
        return None;
    }
    match &ctx.terrain_heights {
        Some(heights) => {
            let idx = (y * sx + x) as usize;
            Some(heights[idx] as f64)
        }
        None => Some(DEFAULT_TERRAIN_HEIGHT as f64),
    }
}

/// Write terrain height at `(x, y)`.  Returns 1 on success, 0 when the
/// coordinates are out of bounds (matching the C# `SetTerrainHeight`
/// contract).  Lazily allocates the heightmap on first write.
fn set_terrain_height(ctx: &mut ScriptContext, x: i32, y: i32, val: f64) -> i32 {
    let sx = ctx.region_size[0];
    let sy = ctx.region_size[1];
    if x < 0 || x >= sx || y < 0 || y >= sy {
        return 0;
    }
    // Lazy-init the heightmap on first write.
    if ctx.terrain_heights.is_none() {
        let len = (sx * sy) as usize;
        ctx.terrain_heights = Some(vec![DEFAULT_TERRAIN_HEIGHT; len]);
    }
    let heights = ctx.terrain_heights.as_mut().unwrap();
    let idx = (y * sx + x) as usize;
    let clamped = (val as f32).clamp(0.0, REGION_HEIGHT_LIMIT);
    heights[idx] = clamped;
    ctx.terrain_dirty = true;
    1
}

/// Extract integer codes from a `MultiValue`.  If the first element is a
/// Lua table the codes are read from its sequence; otherwise individual
/// integer/number values in the `MultiValue` are used.
fn extract_integer_codes(mv: &[LuaValue]) -> Vec<i64> {
    if let Some(LuaValue::Table(tbl)) = mv.first() {
        return tbl
            .sequence_values::<i64>()
            .filter_map(|r| r.ok())
            .collect();
    }
    mv.iter()
        .filter_map(|v| match v {
            LuaValue::Integer(n) => Some(*n),
            LuaValue::Number(n) => Some(*n as i64),
            _ => None,
        })
        .collect()
}

/// Extract a flat list of `LuaValue` entries from a `MultiValue`.  If the
/// first element is a table its sequence values are flattened; otherwise the
/// raw `MultiValue` values are returned.
fn flatten_rules(mv: Vec<LuaValue>) -> Vec<LuaValue> {
    if let Some(LuaValue::Table(tbl)) = mv.first() {
        return tbl
            .sequence_values::<LuaValue>()
            .filter_map(|r| r.ok())
            .collect();
    }
    mv
}

/// Normalise a sun hour into the 0..24 range (matching the C# while-loops).
fn normalise_sun_hour(mut h: f64) -> f64 {
    while h > 24.0 {
        h -= 24.0;
    }
    while h < 0.0 {
        h += 24.0;
    }
    h
}

// ---------------------------------------------------------------------------
// Terrain
// ---------------------------------------------------------------------------

/// `osTerrainSetHeight(x, y, val)` -- deprecated alias.
fn register_os_terrain_set_height(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (x, y, val): (i32, i32, f64)| {
        let mut c = ctx.write();
        Ok(set_terrain_height(&mut c, x, y, val))
    })?;
    lua.globals().set("osTerrainSetHeight", f)
}

/// `osSetTerrainHeight(x, y, val)` -- set a single heightmap cell.
fn register_os_set_terrain_height(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (x, y, val): (i32, i32, f64)| {
        let mut c = ctx.write();
        Ok(set_terrain_height(&mut c, x, y, val))
    })?;
    lua.globals().set("osSetTerrainHeight", f)
}

/// `osTerrainGetHeight(x, y)` -- deprecated alias.
fn register_os_terrain_get_height(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (x, y): (i32, i32)| {
        let c = ctx.read();
        match get_terrain_height(&c, x, y) {
            Some(h) => Ok(h),
            None => Err(LuaError::RuntimeError(
                "osTerrainGetHeight: Coordinate out of bounds".into(),
            )),
        }
    })?;
    lua.globals().set("osTerrainGetHeight", f)
}

/// `osGetTerrainHeight(x, y)` -- read a single heightmap cell.
fn register_os_get_terrain_height(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (x, y): (i32, i32)| {
        let c = ctx.read();
        match get_terrain_height(&c, x, y) {
            Some(h) => Ok(h),
            None => Err(LuaError::RuntimeError(
                "osGetTerrainHeight: Coordinate out of bounds".into(),
            )),
        }
    })?;
    lua.globals().set("osGetTerrainHeight", f)
}

/// `osTerrainFlush()` -- mark terrain dirty so the viewer sees changes.
/// Rate-limited to one flush per 60 seconds (matching the C# cooldown).
fn register_os_terrain_flush(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let mut c = ctx.write();
        let now = now_secs();
        if now - c.last_terrain_flush < TERRAIN_FLUSH_COOLDOWN {
            return Ok(());
        }
        c.last_terrain_flush = now;
        c.terrain_dirty = true;
        Ok(())
    })?;
    lua.globals().set("osTerrainFlush", f)
}

// ---------------------------------------------------------------------------
// Sun / environment
// ---------------------------------------------------------------------------

/// `osSunGetParam(param)` -- read a named sun/environment parameter.
///
/// Supported parameters (case-insensitive, matching the C# `GetSunParam`):
/// - `"day_length"` -- virtual day length in seconds (default 14 400)
/// - `"year_length"` -- 365
/// - `"day_night_offset"` -- 0
/// - `"update_interval"` -- 0.1
/// - `"day_time_sun_hour_scale"` -- 1
fn register_os_sun_get_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, param: String| {
        let c = ctx.read();
        let val = match param.to_lowercase().as_str() {
            "day_length" => c.day_length,
            "year_length" => 365.0,
            "day_night_offset" => 0.0,
            "update_interval" => 0.1,
            "day_time_sun_hour_scale" => 1.0,
            _ => 0.0,
        };
        Ok(val)
    })?;
    lua.globals().set("osSunGetParam", f)
}

/// `osSetEstateSunSettings(world, fixed, hour)` -- change estate sun state.
///
/// `world` is non-zero to use global time; `fixed` is non-zero to lock the
/// sun position; `hour` is 0..24 with 0 just after sunrise.
fn register_os_set_estate_sun_settings(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_world, fixed, hour): (i32, i32, f64)| {
        let mut c = ctx.write();
        let h = normalise_sun_hour(hour);
        c.use_estate_sun = true;
        c.sun_fixed = fixed != 0;
        c.sun_hour = h;
        Ok(())
    })?;
    lua.globals().set("osSetEstateSunSettings", f)
}

/// `osSetRegionSunSettings(fixed, sun_hour, day_length, year_length, day_offset)`
///
/// Sets region-specific sun parameters.  `fixed` non-zero locks the sun;
/// `sun_hour` is 0..24; `day_length` is the virtual day in seconds.
/// `year_length` and `day_offset` are accepted but only `day_length` is
/// stored (year length is always 365 and offset is always 0, matching the
/// C# implementation).
fn register_os_set_region_sun_settings(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_,
              (fixed, sun_hour, day_length, _year_length, _day_offset): (
            i32,
            f64,
            f64,
            f64,
            f64,
        )| {
            let mut c = ctx.write();
            let h = normalise_sun_hour(sun_hour);
            c.use_estate_sun = false;
            c.sun_fixed = fixed != 0;
            c.sun_hour = h;
            if day_length > 0.0 {
                c.day_length = day_length;
            }
            Ok(())
        },
    )?;
    lua.globals().set("osSetRegionSunSettings", f)
}

// ---------------------------------------------------------------------------
// Wind
// ---------------------------------------------------------------------------

/// `osGetWindParam(plugin, param)` -- read a named wind-model parameter.
///
/// Parameters are stored per-plugin, keyed as `"plugin:param"`.
fn register_os_get_wind_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (plugin, param): (String, String)| {
        let c = ctx.read();
        let key = format!("{plugin}:{param}");
        Ok(c.wind_params.get(&key).copied().unwrap_or(0.0))
    })?;
    lua.globals().set("osGetWindParam", f)
}

/// `osSetWindParam(plugin, param, value)` -- write a named wind-model parameter.
fn register_os_set_wind_param(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (plugin, param, value): (String, String, f64)| {
        let mut c = ctx.write();
        let key = format!("{plugin}:{param}");
        c.wind_params.insert(key, value);
        Ok(())
    })?;
    lua.globals().set("osSetWindParam", f)
}

// ---------------------------------------------------------------------------
// Water
// ---------------------------------------------------------------------------

/// `osGetRegionWaterHeight()` -- return the region's water level.
fn register_os_get_region_water_height(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        Ok(c.water_height)
    })?;
    lua.globals().set("osGetRegionWaterHeight", f)
}

// ---------------------------------------------------------------------------
// Parcel
// ---------------------------------------------------------------------------

/// `osParcelSetDetails(pos, rules)` -- deprecated alias for `osSetParcelDetails`.
fn register_os_parcel_set_details(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_pos, rules): (String, mlua::MultiValue)| {
        let mut c = ctx.write();
        apply_parcel_rules(&mut c, rules);
        Ok(())
    })?;
    lua.globals().set("osParcelSetDetails", f)
}

/// `osSetParcelDetails(pos, rules)` -- modify parcel properties.
///
/// `rules` is a flat list of `[code, value, code, value, ...]` pairs using
/// the `PARCEL_DETAILS_*` constants.  Supported codes: NAME (0), DESC (1),
/// OWNER (2).
fn register_os_set_parcel_details(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_pos, rules): (String, mlua::MultiValue)| {
        let mut c = ctx.write();
        apply_parcel_rules(&mut c, rules);
        Ok(())
    })?;
    lua.globals().set("osSetParcelDetails", f)
}

/// Apply parcel detail rules from a `[code, value, ...]` sequence.
fn apply_parcel_rules(ctx: &mut ScriptContext, rules: mlua::MultiValue) {
    let entries = flatten_rules(rules.into_iter().collect());
    let mut i = 0;
    while i + 1 < entries.len() {
        let code = match &entries[i] {
            LuaValue::Integer(n) => *n,
            LuaValue::Number(n) => *n as i64,
            _ => {
                i += 2;
                continue;
            }
        };
        match code {
            PARCEL_DETAILS_NAME => {
                if let LuaValue::String(s) = &entries[i + 1] {
                    if let Ok(st) = s.to_str() {
                        ctx.parcel_name = st.to_string();
                    }
                }
            }
            PARCEL_DETAILS_DESC => {
                if let LuaValue::String(s) = &entries[i + 1] {
                    if let Ok(st) = s.to_str() {
                        ctx.parcel_description = st.to_string();
                    }
                }
            }
            PARCEL_DETAILS_OWNER => {
                if let LuaValue::String(s) = &entries[i + 1] {
                    if let Ok(st) = s.to_str() {
                        if let Ok(id) = uuid::Uuid::parse_str(&*st) {
                            ctx.parcel_owner = id;
                        }
                    }
                }
            }
            _ => {} // Other codes (GROUP, CLAIMDATE, etc.) require subsystems not yet wired.
        }
        i += 2;
    }
}

/// `osGetParcelDetails(pos, params...)` -- read parcel properties.
///
/// Returns a Lua table whose sequential values correspond to the requested
/// `PARCEL_DETAILS_*` codes.
fn register_os_get_parcel_details(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, (_pos, params): (String, mlua::MultiValue)| {
        let c = ctx.read();
        let param_vec: Vec<LuaValue> = params.into_iter().collect();
        let codes = extract_integer_codes(&param_vec);
        let result = lua.create_table()?;
        for (seq, code) in codes.into_iter().enumerate() {
            let detail: LuaValue = match code {
                PARCEL_DETAILS_NAME => {
                    LuaValue::String(lua.create_string(&c.parcel_name)?)
                }
                PARCEL_DETAILS_DESC => {
                    LuaValue::String(lua.create_string(&c.parcel_description)?)
                }
                PARCEL_DETAILS_OWNER => {
                    LuaValue::String(lua.create_string(&c.parcel_owner.to_string())?)
                }
                PARCEL_DETAILS_GROUP => {
                    LuaValue::String(lua.create_string(&uuid::Uuid::nil().to_string())?)
                }
                PARCEL_DETAILS_AREA => LuaValue::Integer(c.parcel_area as i64),
                PARCEL_DETAILS_ID => {
                    LuaValue::String(lua.create_string(&uuid::Uuid::nil().to_string())?)
                }
                PARCEL_DETAILS_SEE_AVATARS => LuaValue::Integer(1),
                _ => LuaValue::String(lua.create_string("")?),
            };
            result.set((seq + 1) as i64, detail)?;
        }
        Ok(result)
    })?;
    lua.globals().set("osGetParcelDetails", f)
}

/// `osGetParcelDwell(pos)` -- return the accumulated dwell for the parcel.
fn register_os_get_parcel_dwell(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _pos: String| {
        let c = ctx.read();
        Ok(c.parcel_dwell)
    })?;
    lua.globals().set("osGetParcelDwell", f)
}

/// `osGetParcelPrimCount(pos)` -- return the prim count for the parcel.
fn register_os_get_parcel_prim_count(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _pos: String| {
        let c = ctx.read();
        Ok(c.parcel_prim_count)
    })?;
    lua.globals().set("osGetParcelPrimCount", f)
}

// ---------------------------------------------------------------------------
// Media
// ---------------------------------------------------------------------------

/// `osSetParcelMusicURL(url)` -- set the parcel's streaming music URL.
fn register_os_set_parcel_music_url(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, url: String| {
        let mut c = ctx.write();
        c.parcel_music_url = url;
        Ok(())
    })?;
    lua.globals().set("osSetParcelMusicURL", f)
}

/// `osSetParcelSIPAddress(addr)` -- set the SIP address for voice on this
/// parcel.  In the C# implementation this checks land ownership and the
/// voice module.  Here we store the value directly; permission checks are
/// handled by the OSSL threat-level gate.
fn register_os_set_parcel_sip_address(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, addr: String| {
        let mut c = ctx.write();
        c.parcel_sip_address = addr;
        Ok(())
    })?;
    lua.globals().set("osSetParcelSIPAddress", f)
}

/// `osSetPrimFloatOnWater(enable)` -- toggle buoyancy for the host prim.
/// Non-zero enables floating; zero disables it.
fn register_os_set_prim_float_on_water(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, enable: i32| {
        let mut c = ctx.write();
        c.float_on_water = enable != 0;
        Ok(())
    })?;
    lua.globals().set("osSetPrimFloatOnWater", f)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::ScriptContext;
    use crate::ossl::{OsslConfig, OsslThreatLevel, register_ossl};
    use uuid::Uuid;

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            ..ScriptContext::default()
        }))
    }

    fn make_vm(ctx: &Arc<RwLock<ScriptContext>>) -> Lua {
        let lua = Lua::new();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Severe,
            ..Default::default()
        };
        register_ossl(&lua, &config, ctx).unwrap();
        lua
    }

    #[test]
    fn terrain_get_default_height() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let h: f64 = lua
            .load("return osGetTerrainHeight(128, 128)")
            .eval()
            .unwrap();
        assert!((h - DEFAULT_TERRAIN_HEIGHT as f64).abs() < f64::EPSILON);
    }

    #[test]
    fn terrain_set_then_get() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let r: i32 = lua
            .load("return osSetTerrainHeight(10, 20, 55.0)")
            .eval()
            .unwrap();
        assert_eq!(r, 1);
        let h: f64 = lua
            .load("return osGetTerrainHeight(10, 20)")
            .eval()
            .unwrap();
        assert!((h - 55.0).abs() < f64::EPSILON);
        // Neighbouring cell still at default.
        let h2: f64 = lua
            .load("return osGetTerrainHeight(10, 21)")
            .eval()
            .unwrap();
        assert!((h2 - DEFAULT_TERRAIN_HEIGHT as f64).abs() < f64::EPSILON);
    }

    #[test]
    fn terrain_out_of_bounds() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let r: i32 = lua
            .load("return osSetTerrainHeight(256, 0, 10.0)")
            .eval()
            .unwrap();
        assert_eq!(r, 0);

        let err = lua
            .load("return osGetTerrainHeight(256, 0)")
            .eval::<f64>();
        assert!(err.is_err());
    }

    #[test]
    fn terrain_clamping() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        lua.load("osSetTerrainHeight(0, 0, -100.0)")
            .exec()
            .unwrap();
        let h: f64 = lua
            .load("return osGetTerrainHeight(0, 0)")
            .eval()
            .unwrap();
        assert!((h - 0.0).abs() < f64::EPSILON);

        lua.load("osSetTerrainHeight(0, 0, 9999.0)")
            .exec()
            .unwrap();
        let h2: f64 = lua
            .load("return osGetTerrainHeight(0, 0)")
            .eval()
            .unwrap();
        assert!((h2 - REGION_HEIGHT_LIMIT as f64).abs() < f64::EPSILON);
    }

    #[test]
    fn terrain_flush_marks_dirty() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        // Set last_terrain_flush far in the past so cooldown doesn't block.
        ctx.write().last_terrain_flush = 0.0;
        lua.load("osTerrainFlush()").exec().unwrap();
        assert!(ctx.read().terrain_dirty);
    }

    #[test]
    fn terrain_flush_cooldown() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        // First flush succeeds.
        ctx.write().last_terrain_flush = 0.0;
        lua.load("osTerrainFlush()").exec().unwrap();
        assert!(ctx.read().terrain_dirty);
        // Clear the flag and try again immediately -- cooldown should block.
        ctx.write().terrain_dirty = false;
        lua.load("osTerrainFlush()").exec().unwrap();
        assert!(!ctx.read().terrain_dirty);
    }

    #[test]
    fn sun_get_param_day_length() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let dl: f64 = lua
            .load(r#"return osSunGetParam("day_length")"#)
            .eval()
            .unwrap();
        assert!((dl - DEFAULT_DAY_LENGTH).abs() < f64::EPSILON);
    }

    #[test]
    fn sun_get_param_year_length() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let yl: f64 = lua
            .load(r#"return osSunGetParam("year_length")"#)
            .eval()
            .unwrap();
        assert!((yl - 365.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sun_get_param_unknown() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let v: f64 = lua
            .load(r#"return osSunGetParam("nonexistent")"#)
            .eval()
            .unwrap();
        assert!((v - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn estate_sun_settings() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        lua.load("osSetEstateSunSettings(0, 1, 12.5)")
            .exec()
            .unwrap();
        let c = ctx.read();
        assert!(c.use_estate_sun);
        assert!(c.sun_fixed);
        assert!((c.sun_hour - 12.5).abs() < f64::EPSILON);
    }

    #[test]
    fn region_sun_settings() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        lua.load("osSetRegionSunSettings(1, 18.0, 7200.0, 365.0, 0.0)")
            .exec()
            .unwrap();
        let c = ctx.read();
        assert!(!c.use_estate_sun);
        assert!(c.sun_fixed);
        assert!((c.sun_hour - 18.0).abs() < f64::EPSILON);
        assert!((c.day_length - 7200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sun_hour_normalisation() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        lua.load("osSetEstateSunSettings(0, 1, 30.0)")
            .exec()
            .unwrap();
        let h = ctx.read().sun_hour;
        assert!((h - 6.0).abs() < f64::EPSILON); // 30 - 24 = 6
    }

    #[test]
    fn wind_params_roundtrip() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        lua.load(r#"osSetWindParam("SimpleRandomWind", "strength", 3.5)"#)
            .exec()
            .unwrap();
        let v: f64 = lua
            .load(r#"return osGetWindParam("SimpleRandomWind", "strength")"#)
            .eval()
            .unwrap();
        assert!((v - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wind_param_missing_returns_zero() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let v: f64 = lua
            .load(r#"return osGetWindParam("NoSuchPlugin", "foo")"#)
            .eval()
            .unwrap();
        assert!((v - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn water_height() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let w: f64 = lua
            .load("return osGetRegionWaterHeight()")
            .eval()
            .unwrap();
        assert!((w - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parcel_dwell_default() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let d: f64 = lua
            .load(r#"return osGetParcelDwell("<128,128,0>")"#)
            .eval()
            .unwrap();
        assert!((d - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parcel_prim_count_default() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        let n: i32 = lua
            .load(r#"return osGetParcelPrimCount("<128,128,0>")"#)
            .eval()
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn set_parcel_music_url() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        lua.load(r#"osSetParcelMusicURL("http://example.com/stream")"#)
            .exec()
            .unwrap();
        assert_eq!(
            ctx.read().parcel_music_url,
            "http://example.com/stream"
        );
    }

    #[test]
    fn set_parcel_sip_address() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        lua.load(r#"osSetParcelSIPAddress("sip:voice@example.com")"#)
            .exec()
            .unwrap();
        assert_eq!(
            ctx.read().parcel_sip_address,
            "sip:voice@example.com"
        );
    }

    #[test]
    fn set_prim_float_on_water() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        assert!(!ctx.read().float_on_water);
        lua.load("osSetPrimFloatOnWater(1)").exec().unwrap();
        assert!(ctx.read().float_on_water);
        lua.load("osSetPrimFloatOnWater(0)").exec().unwrap();
        assert!(!ctx.read().float_on_water);
    }

    #[test]
    fn deprecated_aliases_work() {
        let ctx = make_ctx();
        let lua = make_vm(&ctx);
        // osTerrainSetHeight is the deprecated alias for osSetTerrainHeight.
        let r: i32 = lua
            .load("return osTerrainSetHeight(5, 5, 42.0)")
            .eval()
            .unwrap();
        assert_eq!(r, 1);
        // osTerrainGetHeight is the deprecated alias for osGetTerrainHeight.
        let h: f64 = lua
            .load("return osTerrainGetHeight(5, 5)")
            .eval()
            .unwrap();
        assert!((h - 42.0).abs() < f64::EPSILON);
    }
}
