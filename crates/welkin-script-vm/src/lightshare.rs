// SPDX-License-Identifier: LGPL-3.0-or-later
//! LightShare windlight API — controls region environment/windlight settings.
//!
//! Provides `lsGetWindlightScene`, `lsSetWindlightScene`, and
//! `lsSetWindlightSceneTargeted` as Lua globals, plus WL_* rule constants.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use crate::builtins::{ScriptContext, WindlightData};

// ---------------------------------------------------------------------------
// Helper: build a vec3 Lua table from a 3-element array
// ---------------------------------------------------------------------------

fn vec3_table(lua: &Lua, v: &[f64; 3]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    t.set("z", v[2])?;
    Ok(t)
}

fn vec2_table(lua: &Lua, v: &[f64; 2]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    Ok(t)
}

fn vec4_table(lua: &Lua, v: &[f64; 4]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    t.set("z", v[2])?;
    t.set("w", v[3])?;
    Ok(t)
}

// ---------------------------------------------------------------------------
// Helper: read vec2/vec3/vec4 from a Lua table
// ---------------------------------------------------------------------------

fn read_vec3(t: &LuaTable) -> LuaResult<[f64; 3]> {
    Ok([t.get::<f64>("x")?, t.get::<f64>("y")?, t.get::<f64>("z")?])
}

fn read_vec2(t: &LuaTable) -> LuaResult<[f64; 2]> {
    Ok([t.get::<f64>("x")?, t.get::<f64>("y")?])
}

fn read_vec4(t: &LuaTable) -> LuaResult<[f64; 4]> {
    Ok([
        t.get::<f64>("x")?,
        t.get::<f64>("y")?,
        t.get::<f64>("z")?,
        t.get::<f64>("w")?,
    ])
}

// ---------------------------------------------------------------------------
// Helper: append a windlight rule value to the result table
// ---------------------------------------------------------------------------

fn append_rule_value(
    lua: &Lua,
    result: &LuaTable,
    idx: &mut i64,
    rule: i64,
    wl: &WindlightData,
) -> LuaResult<()> {
    *idx += 1;
    result.set(*idx, rule)?;
    *idx += 1;

    match rule {
        0 => result.set(*idx, vec3_table(lua, &wl.water_color)?)?,
        1 => result.set(*idx, wl.water_fog_density_exponent)?,
        2 => result.set(*idx, wl.underwater_fog_modifier)?,
        3 => result.set(*idx, vec3_table(lua, &wl.reflection_wavelet_scale)?)?,
        4 => result.set(*idx, wl.fresnel_scale)?,
        5 => result.set(*idx, wl.fresnel_offset)?,
        6 => result.set(*idx, wl.refract_scale_above)?,
        7 => result.set(*idx, wl.refract_scale_below)?,
        8 => result.set(*idx, wl.blur_multiplier)?,
        9 => result.set(*idx, vec2_table(lua, &wl.big_wave_direction)?)?,
        10 => result.set(*idx, vec2_table(lua, &wl.little_wave_direction)?)?,
        11 => result.set(*idx, wl.normal_map_texture.as_str())?,
        12 => result.set(*idx, vec4_table(lua, &wl.horizon)?)?,
        13 => result.set(*idx, wl.haze_density)?,
        14 => result.set(*idx, wl.haze_horizon)?,
        15 => result.set(*idx, wl.density_multiplier)?,
        16 => result.set(*idx, wl.distance_multiplier)?,
        17 => result.set(*idx, wl.max_altitude as i64)?,
        18 => result.set(*idx, vec4_table(lua, &wl.sun_moon_color)?)?,
        19 => result.set(*idx, wl.sun_moon_position)?,
        20 => result.set(*idx, vec4_table(lua, &wl.ambient)?)?,
        21 => result.set(*idx, wl.east_angle)?,
        22 => result.set(*idx, wl.sun_glow_focus)?,
        23 => result.set(*idx, wl.sun_glow_size)?,
        24 => result.set(*idx, wl.scene_gamma)?,
        25 => result.set(*idx, wl.star_brightness)?,
        26 => result.set(*idx, vec4_table(lua, &wl.cloud_color)?)?,
        27 => result.set(*idx, vec3_table(lua, &wl.cloud_xy_density)?)?,
        28 => result.set(*idx, wl.cloud_coverage)?,
        29 => result.set(*idx, wl.cloud_scale)?,
        30 => result.set(*idx, vec3_table(lua, &wl.cloud_detail_xy_density)?)?,
        31 => result.set(*idx, wl.cloud_scroll_x)?,
        32 => result.set(*idx, wl.cloud_scroll_y)?,
        33 => result.set(*idx, if wl.cloud_scroll_x_lock { 1i64 } else { 0i64 })?,
        34 => result.set(*idx, if wl.cloud_scroll_y_lock { 1i64 } else { 0i64 })?,
        35 => result.set(*idx, if wl.draw_classic_clouds { 1i64 } else { 0i64 })?,
        _ => {
            // Unknown rule: still append something so the flat list stays paired.
            result.set(*idx, LuaNil)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: apply one rule from a set-windlight flat list
// ---------------------------------------------------------------------------

fn apply_rule(wl: &mut WindlightData, rule: i64, value: &LuaValue) -> LuaResult<()> {
    match rule {
        0 => {
            let t = table_from_lua(value, "WL_WATER_COLOR")?;
            wl.water_color = read_vec3(&t)?;
        }
        1 => wl.water_fog_density_exponent = f64_from_lua(value)?,
        2 => wl.underwater_fog_modifier = f64_from_lua(value)?,
        3 => {
            let t = table_from_lua(value, "WL_REFLECTION_WAVELET_SCALE")?;
            wl.reflection_wavelet_scale = read_vec3(&t)?;
        }
        4 => wl.fresnel_scale = f64_from_lua(value)?,
        5 => wl.fresnel_offset = f64_from_lua(value)?,
        6 => wl.refract_scale_above = f64_from_lua(value)?,
        7 => wl.refract_scale_below = f64_from_lua(value)?,
        8 => wl.blur_multiplier = f64_from_lua(value)?,
        9 => {
            let t = table_from_lua(value, "WL_BIG_WAVE_DIRECTION")?;
            wl.big_wave_direction = read_vec2(&t)?;
        }
        10 => {
            let t = table_from_lua(value, "WL_LITTLE_WAVE_DIRECTION")?;
            wl.little_wave_direction = read_vec2(&t)?;
        }
        11 => {
            wl.normal_map_texture = string_from_lua(value)?;
        }
        12 => {
            let t = table_from_lua(value, "WL_HORIZON")?;
            wl.horizon = read_vec4(&t)?;
        }
        13 => wl.haze_density = f64_from_lua(value)?,
        14 => wl.haze_horizon = f64_from_lua(value)?,
        15 => wl.density_multiplier = f64_from_lua(value)?,
        16 => wl.distance_multiplier = f64_from_lua(value)?,
        17 => wl.max_altitude = i64_from_lua(value)? as u16,
        18 => {
            let t = table_from_lua(value, "WL_SUN_MOON_COLOR")?;
            wl.sun_moon_color = read_vec4(&t)?;
        }
        19 => wl.sun_moon_position = f64_from_lua(value)?,
        20 => {
            let t = table_from_lua(value, "WL_AMBIENT")?;
            wl.ambient = read_vec4(&t)?;
        }
        21 => wl.east_angle = f64_from_lua(value)?,
        22 => wl.sun_glow_focus = f64_from_lua(value)?,
        23 => wl.sun_glow_size = f64_from_lua(value)?,
        24 => wl.scene_gamma = f64_from_lua(value)?,
        25 => wl.star_brightness = f64_from_lua(value)?,
        26 => {
            let t = table_from_lua(value, "WL_CLOUD_COLOR")?;
            wl.cloud_color = read_vec4(&t)?;
        }
        27 => {
            let t = table_from_lua(value, "WL_CLOUD_XY_DENSITY")?;
            wl.cloud_xy_density = read_vec3(&t)?;
        }
        28 => wl.cloud_coverage = f64_from_lua(value)?,
        29 => wl.cloud_scale = f64_from_lua(value)?,
        30 => {
            let t = table_from_lua(value, "WL_CLOUD_DETAIL_XY_DENSITY")?;
            wl.cloud_detail_xy_density = read_vec3(&t)?;
        }
        31 => wl.cloud_scroll_x = f64_from_lua(value)?,
        32 => wl.cloud_scroll_y = f64_from_lua(value)?,
        33 => wl.cloud_scroll_x_lock = i64_from_lua(value)? != 0,
        34 => wl.cloud_scroll_y_lock = i64_from_lua(value)? != 0,
        35 => wl.draw_classic_clouds = i64_from_lua(value)? != 0,
        _ => {
            tracing::warn!(rule, "unknown windlight rule in lsSetWindlightScene");
        }
    }
    Ok(())
}

fn f64_from_lua(v: &LuaValue) -> LuaResult<f64> {
    match v {
        LuaValue::Number(n) => Ok(*n),
        LuaValue::Integer(n) => Ok(*n as f64),
        _ => Err(mlua::Error::runtime("expected number")),
    }
}

fn i64_from_lua(v: &LuaValue) -> LuaResult<i64> {
    match v {
        LuaValue::Integer(n) => Ok(*n),
        LuaValue::Number(n) => Ok(*n as i64),
        _ => Err(mlua::Error::runtime("expected integer")),
    }
}

fn string_from_lua(v: &LuaValue) -> LuaResult<String> {
    match v {
        LuaValue::String(s) => Ok(s.to_str()?.to_string()),
        _ => Err(mlua::Error::runtime("expected string")),
    }
}

fn table_from_lua<'a>(v: &'a LuaValue, name: &str) -> LuaResult<LuaTable> {
    match v {
        LuaValue::Table(t) => Ok(t.clone()),
        _ => Err(mlua::Error::runtime(format!("{name} expects a table"))),
    }
}

// ---------------------------------------------------------------------------
// Core set-windlight logic shared by both set functions
// ---------------------------------------------------------------------------

fn do_set_windlight(_lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>, rules: LuaTable) -> LuaResult<i64> {
    let len = rules.len()? as i64;
    let mut ctx = ctx.write();

    let mut i = 1i64;
    while i < len {
        let rule: i64 = rules.get(i)?;
        i += 1;
        let value: LuaValue = rules.get(i)?;
        i += 1;
        apply_rule(&mut ctx.windlight, rule, &value)?;
    }

    if let Some(ref tx) = ctx.windlight_sender {
        let _ = tx.send(ctx.windlight.clone());
    }

    Ok(1)
}

// ---------------------------------------------------------------------------
// Public registration
// ---------------------------------------------------------------------------

/// Register LightShare windlight functions and WL_* constants as Lua globals.
pub fn register_lightshare(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let globals = lua.globals();

    // Register WL_* constants
    globals.set("WL_WATER_COLOR", 0)?;
    globals.set("WL_WATER_FOG_DENSITY_EXPONENT", 1)?;
    globals.set("WL_UNDERWATER_FOG_MODIFIER", 2)?;
    globals.set("WL_REFLECTION_WAVELET_SCALE", 3)?;
    globals.set("WL_FRESNEL_SCALE", 4)?;
    globals.set("WL_FRESNEL_OFFSET", 5)?;
    globals.set("WL_REFRACT_SCALE_ABOVE", 6)?;
    globals.set("WL_REFRACT_SCALE_BELOW", 7)?;
    globals.set("WL_BLUR_MULTIPLIER", 8)?;
    globals.set("WL_BIG_WAVE_DIRECTION", 9)?;
    globals.set("WL_LITTLE_WAVE_DIRECTION", 10)?;
    globals.set("WL_NORMAL_MAP_TEXTURE", 11)?;
    globals.set("WL_HORIZON", 12)?;
    globals.set("WL_HAZE_DENSITY", 13)?;
    globals.set("WL_HAZE_HORIZON", 14)?;
    globals.set("WL_DENSITY_MULTIPLIER", 15)?;
    globals.set("WL_DISTANCE_MULTIPLIER", 16)?;
    globals.set("WL_MAX_ALTITUDE", 17)?;
    globals.set("WL_SUN_MOON_COLOR", 18)?;
    globals.set("WL_SUN_MOON_POSITION", 19)?;
    globals.set("WL_AMBIENT", 20)?;
    globals.set("WL_EAST_ANGLE", 21)?;
    globals.set("WL_SUN_GLOW_FOCUS", 22)?;
    globals.set("WL_SUN_GLOW_SIZE", 23)?;
    globals.set("WL_SCENE_GAMMA", 24)?;
    globals.set("WL_STAR_BRIGHTNESS", 25)?;
    globals.set("WL_CLOUD_COLOR", 26)?;
    globals.set("WL_CLOUD_XY_DENSITY", 27)?;
    globals.set("WL_CLOUD_COVERAGE", 28)?;
    globals.set("WL_CLOUD_SCALE", 29)?;
    globals.set("WL_CLOUD_DETAIL_XY_DENSITY", 30)?;
    globals.set("WL_CLOUD_SCROLL_X", 31)?;
    globals.set("WL_CLOUD_SCROLL_Y", 32)?;
    globals.set("WL_CLOUD_SCROLL_X_LOCK", 33)?;
    globals.set("WL_CLOUD_SCROLL_Y_LOCK", 34)?;
    globals.set("WL_DRAW_CLASSIC_CLOUDS", 35)?;

    // lsGetWindlightScene(rules)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, rules: LuaTable| {
            let ctx = ctx.read();
            let wl = &ctx.windlight;
            let result = lua.create_table()?;
            let mut idx = 0i64;

            let len = rules.len()?;
            for i in 1..=len {
                let rule: i64 = rules.get(i)?;
                append_rule_value(lua, &result, &mut idx, rule, wl)?;
            }

            Ok(result)
        })?;
        globals.set("lsGetWindlightScene", f)?;
    }

    // lsSetWindlightScene(rules)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, rules: LuaTable| {
            do_set_windlight(lua, &ctx, rules)
        })?;
        globals.set("lsSetWindlightScene", f)?;
    }

    // lsSetWindlightSceneTargeted(rules, target)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, (rules, target): (LuaTable, String)| {
            tracing::debug!(target = %target, "lsSetWindlightSceneTargeted");
            do_set_windlight(lua, &ctx, rules)
        })?;
        globals.set("lsSetWindlightSceneTargeted", f)?;
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
        register_lightshare(&lua, &ctx).unwrap();
        (lua, ctx)
    }

    #[test]
    fn test_wl_constants_registered() {
        let (lua, _ctx) = make_test_vm();
        let v: i64 = lua.load("return WL_WATER_COLOR").eval().unwrap();
        assert_eq!(v, 0);
        let v: i64 = lua.load("return WL_DRAW_CLASSIC_CLOUDS").eval().unwrap();
        assert_eq!(v, 35);
        let v: i64 = lua.load("return WL_MAX_ALTITUDE").eval().unwrap();
        assert_eq!(v, 17);
    }

    #[test]
    fn test_get_windlight_scene_defaults() {
        let (lua, _ctx) = make_test_vm();

        // Query rules 0 (water_color vec3), 1 (fog density float),
        // 17 (max_altitude int), 35 (draw_classic_clouds bool-as-int)
        let script = r#"
            local result = lsGetWindlightScene({0, 1, 17, 35})
            -- result should be: [0, {x=4,y=38,z=64}, 1, 4.0, 17, 1605, 35, 1]
            -- Check rule ids come back in order
            assert(result[1] == 0, "rule 0 id")
            local wc = result[2]
            assert(wc.x == 4.0, "water_color.x")
            assert(wc.y == 38.0, "water_color.y")
            assert(wc.z == 64.0, "water_color.z")

            assert(result[3] == 1, "rule 1 id")
            assert(result[4] == 4.0, "water_fog_density_exponent")

            assert(result[5] == 17, "rule 17 id")
            assert(result[6] == 1605, "max_altitude")

            assert(result[7] == 35, "rule 35 id")
            assert(result[8] == 1, "draw_classic_clouds")

            return 1
        "#;

        let r: i64 = lua.load(script).eval().unwrap();
        assert_eq!(r, 1);
    }

    #[test]
    fn test_set_windlight_scene() {
        let (lua, ctx) = make_test_vm();

        let script = r#"
            return lsSetWindlightScene({1, 8.0, 17, 2000})
        "#;

        let r: i64 = lua.load(script).eval().unwrap();
        assert_eq!(r, 1);

        let ctx = ctx.read();
        assert!((ctx.windlight.water_fog_density_exponent - 8.0).abs() < f64::EPSILON);
        assert_eq!(ctx.windlight.max_altitude, 2000);
    }

    #[test]
    fn test_set_windlight_scene_targeted() {
        let (lua, ctx) = make_test_vm();

        let script = r#"
            return lsSetWindlightSceneTargeted(
                {1, 5.5, 28, 0.75},
                "12345678-1234-1234-1234-123456789abc"
            )
        "#;

        let r: i64 = lua.load(script).eval().unwrap();
        assert_eq!(r, 1);

        let ctx = ctx.read();
        assert!((ctx.windlight.water_fog_density_exponent - 5.5).abs() < f64::EPSILON);
        assert!((ctx.windlight.cloud_coverage - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_set_and_get_roundtrip() {
        let (lua, _ctx) = make_test_vm();

        let script = r#"
            lsSetWindlightScene({24, 2.2, 25, 0.5})
            local result = lsGetWindlightScene({24, 25})
            assert(result[1] == 24, "rule 24 id")
            assert(math.abs(result[2] - 2.2) < 0.001, "scene_gamma")
            assert(result[3] == 25, "rule 25 id")
            assert(math.abs(result[4] - 0.5) < 0.001, "star_brightness")
            return 1
        "#;

        let r: i64 = lua.load(script).eval().unwrap();
        assert_eq!(r, 1);
    }

    #[test]
    fn test_windlight_sender_fires() {
        let lua = Lua::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            windlight_sender: Some(tx),
            ..ScriptContext::default()
        }));
        register_lightshare(&lua, &ctx).unwrap();

        lua.load("lsSetWindlightScene({1, 9.0})")
            .exec()
            .unwrap();

        let received = rx.try_recv().unwrap();
        assert!((received.water_fog_density_exponent - 9.0).abs() < f64::EPSILON);
    }
}
