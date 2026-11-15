// SPDX-License-Identifier: LGPL-3.0-or-later
//! llSetPrimitiveParams / llGetPrimitiveParams and link variants.
//!
//! Implements the full PRIM_* constant set from OpenSim's LSL_Constants.cs
//! and the list-based set/get API for prim properties.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;
use crate::VmError;

// ── PRIM_* top-level constants (from OpenSim LSL_Constants.cs) ──────────

pub const PRIM_MATERIAL: i32 = 2;
pub const PRIM_PHYSICS: i32 = 3;
pub const PRIM_TEMP_ON_REZ: i32 = 4;
pub const PRIM_PHANTOM: i32 = 5;
pub const PRIM_POSITION: i32 = 6;
pub const PRIM_SIZE: i32 = 7;
pub const PRIM_ROTATION: i32 = 8;
pub const PRIM_TYPE: i32 = 9;
pub const PRIM_TEXTURE: i32 = 17;
pub const PRIM_COLOR: i32 = 18;
pub const PRIM_BUMP_SHINY: i32 = 19;
pub const PRIM_FULLBRIGHT: i32 = 20;
pub const PRIM_FLEXIBLE: i32 = 21;
pub const PRIM_TEXGEN: i32 = 22;
pub const PRIM_POINT_LIGHT: i32 = 23;
pub const PRIM_CAST_SHADOWS: i32 = 24;
pub const PRIM_GLOW: i32 = 25;
pub const PRIM_TEXT: i32 = 26;
pub const PRIM_NAME: i32 = 27;
pub const PRIM_DESC: i32 = 28;
pub const PRIM_ROT_LOCAL: i32 = 29;
pub const PRIM_PHYSICS_SHAPE_TYPE: i32 = 30;
pub const PRIM_PHYSICS_MATERIAL: i32 = 31;
pub const PRIM_OMEGA: i32 = 32;
pub const PRIM_POS_LOCAL: i32 = 33;
pub const PRIM_LINK_TARGET: i32 = 34;
pub const PRIM_SLICE: i32 = 35;
pub const PRIM_SPECULAR: i32 = 36;
pub const PRIM_NORMAL: i32 = 37;
pub const PRIM_ALPHA_MODE: i32 = 38;
pub const PRIM_ALLOW_UNSIT: i32 = 39;
pub const PRIM_SCRIPTED_SIT_ONLY: i32 = 40;
pub const PRIM_SIT_TARGET: i32 = 41;
pub const PRIM_PROJECTOR: i32 = 42;
pub const PRIM_REFLECTION_PROBE: i32 = 44;
pub const PRIM_GLTF_NORMAL: i32 = 45;
pub const PRIM_GLTF_EMISSIVE: i32 = 46;
pub const PRIM_GLTF_METALLIC_ROUGHNESS: i32 = 47;
pub const PRIM_GLTF_BASE_COLOR: i32 = 48;
pub const PRIM_RENDER_MATERIAL: i32 = 49;

// ── PRIM_TYPE sub-constants ─────────────────────────────────────────────

pub const PRIM_TYPE_BOX: i32 = 0;
pub const PRIM_TYPE_CYLINDER: i32 = 1;
pub const PRIM_TYPE_PRISM: i32 = 2;
pub const PRIM_TYPE_SPHERE: i32 = 3;
pub const PRIM_TYPE_TORUS: i32 = 4;
pub const PRIM_TYPE_TUBE: i32 = 5;
pub const PRIM_TYPE_RING: i32 = 6;
pub const PRIM_TYPE_SCULPT: i32 = 7;

// ── PRIM_HOLE sub-constants ─────────────────────────────────────────────

pub const PRIM_HOLE_DEFAULT: i32 = 0;
pub const PRIM_HOLE_CIRCLE: i32 = 16;
pub const PRIM_HOLE_SQUARE: i32 = 32;
pub const PRIM_HOLE_TRIANGLE: i32 = 48;

// ── PRIM_MATERIAL sub-constants ─────────────────────────────────────────

pub const PRIM_MATERIAL_STONE: i32 = 0;
pub const PRIM_MATERIAL_METAL: i32 = 1;
pub const PRIM_MATERIAL_GLASS: i32 = 2;
pub const PRIM_MATERIAL_WOOD: i32 = 3;
pub const PRIM_MATERIAL_FLESH: i32 = 4;
pub const PRIM_MATERIAL_PLASTIC: i32 = 5;
pub const PRIM_MATERIAL_RUBBER: i32 = 6;
pub const PRIM_MATERIAL_LIGHT: i32 = 7;

// ── PRIM_SHINY sub-constants ────────────────────────────────────────────

pub const PRIM_SHINY_NONE: i32 = 0;
pub const PRIM_SHINY_LOW: i32 = 1;
pub const PRIM_SHINY_MEDIUM: i32 = 2;
pub const PRIM_SHINY_HIGH: i32 = 3;

// ── PRIM_BUMP sub-constants ─────────────────────────────────────────────

pub const PRIM_BUMP_NONE: i32 = 0;
pub const PRIM_BUMP_BRIGHT: i32 = 1;
pub const PRIM_BUMP_DARK: i32 = 2;
pub const PRIM_BUMP_WOOD: i32 = 3;
pub const PRIM_BUMP_BARK: i32 = 4;
pub const PRIM_BUMP_BRICKS: i32 = 5;
pub const PRIM_BUMP_CHECKER: i32 = 6;
pub const PRIM_BUMP_CONCRETE: i32 = 7;
pub const PRIM_BUMP_TILE: i32 = 8;
pub const PRIM_BUMP_STONE: i32 = 9;
pub const PRIM_BUMP_DISKS: i32 = 10;
pub const PRIM_BUMP_GRAVEL: i32 = 11;
pub const PRIM_BUMP_BLOBS: i32 = 12;
pub const PRIM_BUMP_SIDING: i32 = 13;
pub const PRIM_BUMP_LARGETILE: i32 = 14;
pub const PRIM_BUMP_STUCCO: i32 = 15;
pub const PRIM_BUMP_SUCTION: i32 = 16;
pub const PRIM_BUMP_WEAVE: i32 = 17;

// ── PRIM_TEXGEN sub-constants ───────────────────────────────────────────

pub const PRIM_TEXGEN_DEFAULT: i32 = 0;
pub const PRIM_TEXGEN_PLANAR: i32 = 1;

// ── PRIM_ALPHA_MODE sub-constants ───────────────────────────────────────

pub const PRIM_ALPHA_MODE_NONE: i32 = 0;
pub const PRIM_ALPHA_MODE_BLEND: i32 = 1;
pub const PRIM_ALPHA_MODE_MASK: i32 = 2;
pub const PRIM_ALPHA_MODE_EMISSIVE: i32 = 3;

// ── PRIM_PHYSICS_SHAPE sub-constants ────────────────────────────────────

pub const PRIM_PHYSICS_SHAPE_PRIM: i32 = 0;
pub const PRIM_PHYSICS_SHAPE_NONE: i32 = 1;
pub const PRIM_PHYSICS_SHAPE_CONVEX: i32 = 2;

// ── LINK constants ──────────────────────────────────────────────────────

pub const LINK_ROOT: i32 = 1;
pub const LINK_SET: i32 = -1;
pub const LINK_ALL_OTHERS: i32 = -2;
pub const LINK_ALL_CHILDREN: i32 = -3;
pub const LINK_THIS: i32 = -4;

// ── Helpers ─────────────────────────────────────────────────────────────

fn table_get_float(t: &LuaTable, idx: i64) -> f64 {
    t.get::<f64>(idx).unwrap_or(0.0)
}

fn lua_vec3(lua: &Lua, v: [f64; 3]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    t.set("z", v[2])?;
    Ok(t)
}

fn lua_rot4(lua: &Lua, r: [f64; 4]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", r[0])?;
    t.set("y", r[1])?;
    t.set("z", r[2])?;
    t.set("s", r[3])?;
    Ok(t)
}

fn read_vec3(t: &LuaTable) -> [f64; 3] {
    // Try named keys first (x/y/z), fall back to positional (1/2/3)
    let x = t.get::<f64>("x").unwrap_or_else(|_| table_get_float(t, 1));
    let y = t.get::<f64>("y").unwrap_or_else(|_| table_get_float(t, 2));
    let z = t.get::<f64>("z").unwrap_or_else(|_| table_get_float(t, 3));
    [x, y, z]
}

fn read_rot4(t: &LuaTable) -> [f64; 4] {
    let x = t.get::<f64>("x").unwrap_or_else(|_| table_get_float(t, 1));
    let y = t.get::<f64>("y").unwrap_or_else(|_| table_get_float(t, 2));
    let z = t.get::<f64>("z").unwrap_or_else(|_| table_get_float(t, 3));
    let s = t.get::<f64>("s").unwrap_or_else(|_| table_get_float(t, 4));
    [x, y, z, s]
}

/// Number of extra parameters consumed by a PRIM_* rule in set mode.
fn prim_set_param_count(code: i32) -> usize {
    match code {
        PRIM_MATERIAL => 1,
        PRIM_PHYSICS | PRIM_TEMP_ON_REZ | PRIM_PHANTOM => 1,
        PRIM_POSITION | PRIM_POS_LOCAL => 1,
        PRIM_SIZE => 1,
        PRIM_ROTATION | PRIM_ROT_LOCAL => 1,
        PRIM_TYPE => 7, // varies, but consume 7 for box (most common)
        PRIM_TEXTURE => 5,     // face, texture, repeats, offsets, rotation
        PRIM_COLOR => 3,       // face, color, alpha
        PRIM_BUMP_SHINY => 3,  // face, shiny, bump
        PRIM_FULLBRIGHT => 2,  // face, bool
        PRIM_FLEXIBLE => 7,    // bool, softness, gravity, friction, wind, tension, force
        PRIM_TEXGEN => 2,      // face, type
        PRIM_POINT_LIGHT => 5, // bool, color, intensity, radius, falloff
        PRIM_GLOW => 2,        // face, glow
        PRIM_TEXT => 3,         // text, color, alpha
        PRIM_NAME => 1,
        PRIM_DESC => 1,
        PRIM_OMEGA => 3,       // axis, spinrate, gain
        PRIM_PHYSICS_SHAPE_TYPE => 1,
        PRIM_LINK_TARGET => 1, // link number
        PRIM_SLICE => 1,       // vec2 as table
        PRIM_SPECULAR => 7,
        PRIM_NORMAL => 5,
        PRIM_ALPHA_MODE => 3,  // face, mode, cutoff
        PRIM_SIT_TARGET => 3,  // bool, pos, rot
        _ => 0,
    }
}

/// Apply one PRIM_* rule from a list iterator.
fn apply_set_rule(ctx: &mut ScriptContext, code: i32, params: &[LuaValue]) {
    match code {
        PRIM_POSITION | PRIM_POS_LOCAL => {
            if let Some(LuaValue::Table(t)) = params.first() {
                ctx.position = read_vec3(t);
            }
        }
        PRIM_ROTATION | PRIM_ROT_LOCAL => {
            if let Some(LuaValue::Table(t)) = params.first() {
                ctx.rotation = read_rot4(t);
            }
        }
        PRIM_SIZE => {
            if let Some(LuaValue::Table(t)) = params.first() {
                ctx.scale = read_vec3(t);
            }
        }
        PRIM_COLOR => {
            // face, color_vec, alpha
            if params.len() >= 3 {
                if let LuaValue::Table(c) = &params[1] {
                    ctx.color = read_vec3(c);
                }
                match &params[2] {
                    LuaValue::Number(a) => ctx.alpha = *a,
                    LuaValue::Integer(a) => ctx.alpha = *a as f64,
                    _ => {}
                }
            }
        }
        PRIM_TEXTURE => {
            // face, texture_uuid_string, repeats, offsets, rotation
            if params.len() >= 2 {
                if let LuaValue::String(s) = &params[1] {
                    if let Ok(u) = uuid::Uuid::parse_str(&s.to_string_lossy()) {
                        ctx.texture = u;
                    }
                }
            }
        }
        PRIM_TEXT => {
            // text, color, alpha
            if let Some(LuaValue::String(s)) = params.first() {
                ctx.hover_text = s.to_string_lossy().to_string();
            }
            if params.len() >= 2 {
                if let LuaValue::Table(c) = &params[1] {
                    ctx.hover_text_color = read_vec3(c);
                }
            }
        }
        PRIM_NAME => {
            if let Some(LuaValue::String(s)) = params.first() {
                ctx.prim_name = s.to_string_lossy().to_string();
            }
        }
        PRIM_DESC => {
            if let Some(LuaValue::String(s)) = params.first() {
                ctx.prim_desc = s.to_string_lossy().to_string();
            }
        }
        PRIM_MATERIAL => {
            if let Some(LuaValue::Integer(v)) = params.first() {
                ctx.prim_material = *v as i32;
            }
        }
        PRIM_PHYSICS => {
            match params.first() {
                Some(LuaValue::Integer(v)) => ctx.is_physics = *v != 0,
                Some(LuaValue::Boolean(v)) => ctx.is_physics = *v,
                _ => {}
            }
        }
        PRIM_PHANTOM => {
            match params.first() {
                Some(LuaValue::Integer(v)) => ctx.is_phantom = *v != 0,
                Some(LuaValue::Boolean(v)) => ctx.is_phantom = *v,
                _ => {}
            }
        }
        PRIM_TEMP_ON_REZ => {
            match params.first() {
                Some(LuaValue::Integer(v)) => ctx.is_temp_on_rez = *v != 0,
                Some(LuaValue::Boolean(v)) => ctx.is_temp_on_rez = *v,
                _ => {}
            }
        }
        PRIM_FULLBRIGHT => {
            // face, bool
            if params.len() >= 2 {
                match &params[1] {
                    LuaValue::Integer(v) => ctx.fullbright = *v != 0,
                    LuaValue::Boolean(v) => ctx.fullbright = *v,
                    _ => {}
                }
            }
        }
        PRIM_GLOW => {
            // face, glow_value
            if params.len() >= 2 {
                match &params[1] {
                    LuaValue::Number(g) => ctx.glow = *g,
                    LuaValue::Integer(g) => ctx.glow = *g as f64,
                    _ => {}
                }
            }
        }
        PRIM_PHYSICS_SHAPE_TYPE => {
            if let Some(LuaValue::Integer(v)) = params.first() {
                ctx.physics_shape_type = *v as i32;
            }
        }
        PRIM_SIT_TARGET => {
            // bool, pos, rot
            if params.len() >= 3 {
                if let LuaValue::Table(p) = &params[1] {
                    ctx.sit_target_pos = read_vec3(p);
                }
                if let LuaValue::Table(r) = &params[2] {
                    ctx.sit_target_rot = read_rot4(r);
                }
            }
        }
        _ => {
            tracing::warn!("llSetPrimitiveParams: unsupported PRIM code {code}, skipping");
        }
    }
}

/// Gather values for one PRIM_* get rule.
fn gather_get_rule(lua: &Lua, ctx: &ScriptContext, code: i32) -> LuaResult<Vec<LuaValue>> {
    match code {
        PRIM_POSITION | PRIM_POS_LOCAL => {
            Ok(vec![LuaValue::Table(lua_vec3(lua, ctx.position)?)])
        }
        PRIM_ROTATION | PRIM_ROT_LOCAL => {
            Ok(vec![LuaValue::Table(lua_rot4(lua, ctx.rotation)?)])
        }
        PRIM_SIZE => Ok(vec![LuaValue::Table(lua_vec3(lua, ctx.scale)?)]),
        PRIM_COLOR => {
            // returns face=ALL_SIDES → color, alpha
            Ok(vec![
                LuaValue::Table(lua_vec3(lua, ctx.color)?),
                LuaValue::Number(ctx.alpha),
            ])
        }
        PRIM_TEXTURE => {
            Ok(vec![LuaValue::String(
                lua.create_string(ctx.texture.to_string())?,
            )])
        }
        PRIM_TEXT => {
            Ok(vec![
                LuaValue::String(lua.create_string(ctx.hover_text.as_str())?),
                LuaValue::Table(lua_vec3(lua, ctx.hover_text_color)?),
                LuaValue::Number(1.0),
            ])
        }
        PRIM_NAME => Ok(vec![LuaValue::String(
            lua.create_string(ctx.prim_name.as_str())?,
        )]),
        PRIM_DESC => Ok(vec![LuaValue::String(
            lua.create_string(ctx.prim_desc.as_str())?,
        )]),
        PRIM_MATERIAL => Ok(vec![LuaValue::Integer(ctx.prim_material as i64)]),
        PRIM_PHYSICS => Ok(vec![LuaValue::Integer(ctx.is_physics as i64)]),
        PRIM_PHANTOM => Ok(vec![LuaValue::Integer(ctx.is_phantom as i64)]),
        PRIM_TEMP_ON_REZ => Ok(vec![LuaValue::Integer(ctx.is_temp_on_rez as i64)]),
        PRIM_FULLBRIGHT => Ok(vec![LuaValue::Integer(ctx.fullbright as i64)]),
        PRIM_GLOW => Ok(vec![LuaValue::Number(ctx.glow)]),
        PRIM_PHYSICS_SHAPE_TYPE => {
            Ok(vec![LuaValue::Integer(ctx.physics_shape_type as i64)])
        }
        PRIM_SIT_TARGET => Ok(vec![
            LuaValue::Integer(1),
            LuaValue::Table(lua_vec3(lua, ctx.sit_target_pos)?),
            LuaValue::Table(lua_rot4(lua, ctx.sit_target_rot)?),
        ]),
        _ => {
            tracing::warn!("llGetPrimitiveParams: unsupported PRIM code {code}");
            Ok(vec![])
        }
    }
}

/// Process a Lua list (table with integer keys) of set rules.
fn process_set_rules(ctx: &mut ScriptContext, list: &LuaTable) {
    let len = list.raw_len();
    let mut i = 1i64;
    while i <= len as i64 {
        let code: i32 = match list.get::<LuaValue>(i) {
            Ok(LuaValue::Integer(v)) => v as i32,
            Ok(LuaValue::Number(v)) => v as i32,
            _ => break,
        };
        i += 1;

        let count = prim_set_param_count(code);
        let mut params = Vec::with_capacity(count);
        for _ in 0..count {
            if i > len as i64 {
                break;
            }
            if let Ok(v) = list.get::<LuaValue>(i) {
                params.push(v);
            }
            i += 1;
        }
        apply_set_rule(ctx, code, &params);
    }
}

/// Process a Lua list of get rules, returning a flat result list.
fn process_get_rules(lua: &Lua, ctx: &ScriptContext, list: &LuaTable) -> LuaResult<LuaTable> {
    let result = lua.create_table()?;
    let len = list.raw_len();
    let mut out_idx = 1i64;
    let mut i = 1i64;
    while i <= len as i64 {
        let code: i32 = match list.get::<LuaValue>(i) {
            Ok(LuaValue::Integer(v)) => v as i32,
            Ok(LuaValue::Number(v)) => v as i32,
            _ => break,
        };
        i += 1;

        let values = gather_get_rule(lua, ctx, code)?;
        for v in values {
            result.set(out_idx, v)?;
            out_idx += 1;
        }
    }
    Ok(result)
}

/// Register all PRIM_* constants and prim-param functions.
pub fn register_prim_params(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), VmError> {
    let g = lua.globals();

    // ── Register all PRIM_* constants ──
    let consts: &[(&str, i32)] = &[
        ("PRIM_MATERIAL", PRIM_MATERIAL),
        ("PRIM_PHYSICS", PRIM_PHYSICS),
        ("PRIM_TEMP_ON_REZ", PRIM_TEMP_ON_REZ),
        ("PRIM_PHANTOM", PRIM_PHANTOM),
        ("PRIM_POSITION", PRIM_POSITION),
        ("PRIM_SIZE", PRIM_SIZE),
        ("PRIM_ROTATION", PRIM_ROTATION),
        ("PRIM_TYPE", PRIM_TYPE),
        ("PRIM_TEXTURE", PRIM_TEXTURE),
        ("PRIM_COLOR", PRIM_COLOR),
        ("PRIM_BUMP_SHINY", PRIM_BUMP_SHINY),
        ("PRIM_FULLBRIGHT", PRIM_FULLBRIGHT),
        ("PRIM_FLEXIBLE", PRIM_FLEXIBLE),
        ("PRIM_TEXGEN", PRIM_TEXGEN),
        ("PRIM_POINT_LIGHT", PRIM_POINT_LIGHT),
        ("PRIM_CAST_SHADOWS", PRIM_CAST_SHADOWS),
        ("PRIM_GLOW", PRIM_GLOW),
        ("PRIM_TEXT", PRIM_TEXT),
        ("PRIM_NAME", PRIM_NAME),
        ("PRIM_DESC", PRIM_DESC),
        ("PRIM_ROT_LOCAL", PRIM_ROT_LOCAL),
        ("PRIM_PHYSICS_SHAPE_TYPE", PRIM_PHYSICS_SHAPE_TYPE),
        ("PRIM_PHYSICS_MATERIAL", PRIM_PHYSICS_MATERIAL),
        ("PRIM_OMEGA", PRIM_OMEGA),
        ("PRIM_POS_LOCAL", PRIM_POS_LOCAL),
        ("PRIM_LINK_TARGET", PRIM_LINK_TARGET),
        ("PRIM_SLICE", PRIM_SLICE),
        ("PRIM_SPECULAR", PRIM_SPECULAR),
        ("PRIM_NORMAL", PRIM_NORMAL),
        ("PRIM_ALPHA_MODE", PRIM_ALPHA_MODE),
        ("PRIM_ALLOW_UNSIT", PRIM_ALLOW_UNSIT),
        ("PRIM_SCRIPTED_SIT_ONLY", PRIM_SCRIPTED_SIT_ONLY),
        ("PRIM_SIT_TARGET", PRIM_SIT_TARGET),
        ("PRIM_PROJECTOR", PRIM_PROJECTOR),
        ("PRIM_REFLECTION_PROBE", PRIM_REFLECTION_PROBE),
        ("PRIM_GLTF_NORMAL", PRIM_GLTF_NORMAL),
        ("PRIM_GLTF_EMISSIVE", PRIM_GLTF_EMISSIVE),
        ("PRIM_GLTF_METALLIC_ROUGHNESS", PRIM_GLTF_METALLIC_ROUGHNESS),
        ("PRIM_GLTF_BASE_COLOR", PRIM_GLTF_BASE_COLOR),
        ("PRIM_RENDER_MATERIAL", PRIM_RENDER_MATERIAL),
        // Sub-constants
        ("PRIM_TYPE_BOX", PRIM_TYPE_BOX),
        ("PRIM_TYPE_CYLINDER", PRIM_TYPE_CYLINDER),
        ("PRIM_TYPE_PRISM", PRIM_TYPE_PRISM),
        ("PRIM_TYPE_SPHERE", PRIM_TYPE_SPHERE),
        ("PRIM_TYPE_TORUS", PRIM_TYPE_TORUS),
        ("PRIM_TYPE_TUBE", PRIM_TYPE_TUBE),
        ("PRIM_TYPE_RING", PRIM_TYPE_RING),
        ("PRIM_TYPE_SCULPT", PRIM_TYPE_SCULPT),
        ("PRIM_HOLE_DEFAULT", PRIM_HOLE_DEFAULT),
        ("PRIM_HOLE_CIRCLE", PRIM_HOLE_CIRCLE),
        ("PRIM_HOLE_SQUARE", PRIM_HOLE_SQUARE),
        ("PRIM_HOLE_TRIANGLE", PRIM_HOLE_TRIANGLE),
        ("PRIM_MATERIAL_STONE", PRIM_MATERIAL_STONE),
        ("PRIM_MATERIAL_METAL", PRIM_MATERIAL_METAL),
        ("PRIM_MATERIAL_GLASS", PRIM_MATERIAL_GLASS),
        ("PRIM_MATERIAL_WOOD", PRIM_MATERIAL_WOOD),
        ("PRIM_MATERIAL_FLESH", PRIM_MATERIAL_FLESH),
        ("PRIM_MATERIAL_PLASTIC", PRIM_MATERIAL_PLASTIC),
        ("PRIM_MATERIAL_RUBBER", PRIM_MATERIAL_RUBBER),
        ("PRIM_MATERIAL_LIGHT", PRIM_MATERIAL_LIGHT),
        ("PRIM_SHINY_NONE", PRIM_SHINY_NONE),
        ("PRIM_SHINY_LOW", PRIM_SHINY_LOW),
        ("PRIM_SHINY_MEDIUM", PRIM_SHINY_MEDIUM),
        ("PRIM_SHINY_HIGH", PRIM_SHINY_HIGH),
        ("PRIM_BUMP_NONE", PRIM_BUMP_NONE),
        ("PRIM_BUMP_BRIGHT", PRIM_BUMP_BRIGHT),
        ("PRIM_BUMP_DARK", PRIM_BUMP_DARK),
        ("PRIM_BUMP_WOOD", PRIM_BUMP_WOOD),
        ("PRIM_BUMP_BARK", PRIM_BUMP_BARK),
        ("PRIM_BUMP_BRICKS", PRIM_BUMP_BRICKS),
        ("PRIM_BUMP_CHECKER", PRIM_BUMP_CHECKER),
        ("PRIM_BUMP_CONCRETE", PRIM_BUMP_CONCRETE),
        ("PRIM_BUMP_TILE", PRIM_BUMP_TILE),
        ("PRIM_BUMP_STONE", PRIM_BUMP_STONE),
        ("PRIM_BUMP_DISKS", PRIM_BUMP_DISKS),
        ("PRIM_BUMP_GRAVEL", PRIM_BUMP_GRAVEL),
        ("PRIM_BUMP_BLOBS", PRIM_BUMP_BLOBS),
        ("PRIM_BUMP_SIDING", PRIM_BUMP_SIDING),
        ("PRIM_BUMP_LARGETILE", PRIM_BUMP_LARGETILE),
        ("PRIM_BUMP_STUCCO", PRIM_BUMP_STUCCO),
        ("PRIM_BUMP_SUCTION", PRIM_BUMP_SUCTION),
        ("PRIM_BUMP_WEAVE", PRIM_BUMP_WEAVE),
        ("PRIM_TEXGEN_DEFAULT", PRIM_TEXGEN_DEFAULT),
        ("PRIM_TEXGEN_PLANAR", PRIM_TEXGEN_PLANAR),
        ("PRIM_ALPHA_MODE_NONE", PRIM_ALPHA_MODE_NONE),
        ("PRIM_ALPHA_MODE_BLEND", PRIM_ALPHA_MODE_BLEND),
        ("PRIM_ALPHA_MODE_MASK", PRIM_ALPHA_MODE_MASK),
        ("PRIM_ALPHA_MODE_EMISSIVE", PRIM_ALPHA_MODE_EMISSIVE),
        ("PRIM_PHYSICS_SHAPE_PRIM", PRIM_PHYSICS_SHAPE_PRIM),
        ("PRIM_PHYSICS_SHAPE_NONE", PRIM_PHYSICS_SHAPE_NONE),
        ("PRIM_PHYSICS_SHAPE_CONVEX", PRIM_PHYSICS_SHAPE_CONVEX),
        // Link constants
        ("LINK_ROOT", LINK_ROOT),
        ("LINK_SET", LINK_SET),
        ("LINK_ALL_OTHERS", LINK_ALL_OTHERS),
        ("LINK_ALL_CHILDREN", LINK_ALL_CHILDREN),
        ("LINK_THIS", LINK_THIS),
    ];

    for &(name, val) in consts {
        g.set(name, val as i64)?;
    }

    // ── llSetPrimitiveParams(list) ──
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, list: LuaTable| {
            let mut ctx = ctx.write();
            process_set_rules(&mut ctx, &list);
            Ok(())
        })?;
        g.set("llSetPrimitiveParams", f)?;
    }

    // ── llGetPrimitiveParams(list) → list ──
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, list: LuaTable| {
            let ctx = ctx.read();
            process_get_rules(lua, &ctx, &list)
        })?;
        g.set("llGetPrimitiveParams", f)?;
    }

    // ── llSetLinkPrimitiveParams(link, list) ──
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_link, list): (i64, LuaTable)| {
            // Single-prim context: link 0, 1, or LINK_THIS all target self
            let mut ctx = ctx.write();
            process_set_rules(&mut ctx, &list);
            Ok(())
        })?;
        g.set("llSetLinkPrimitiveParams", f)?;
    }

    // ── llSetLinkPrimitiveParamsFast(link, list) ──
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_link, list): (i64, LuaTable)| {
            let mut ctx = ctx.write();
            process_set_rules(&mut ctx, &list);
            Ok(())
        })?;
        g.set("llSetLinkPrimitiveParamsFast", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_vm() -> (Lua, Arc<RwLock<ScriptContext>>) {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::nil(),
            prim_id: Uuid::from_u128(0xabcd),
            prim_name: "TestPrim".into(),
            prim_desc: "A test prim".into(),
            owner_id: Uuid::from_u128(0x1234),
            position: [128.0, 128.0, 25.0],
            scale: [1.0, 2.0, 3.0],
            color: [1.0, 0.5, 0.0],
            alpha: 0.8,
            ..Default::default()
        }));
        register_prim_params(&lua, &ctx).unwrap();
        (lua, ctx)
    }

    #[test]
    fn set_and_get_position() {
        let (lua, ctx) = test_vm();
        lua.load("llSetPrimitiveParams({PRIM_POSITION, {x=10, y=20, z=30}})")
            .exec()
            .unwrap();
        let c = ctx.read();
        assert!((c.position[0] - 10.0).abs() < f64::EPSILON);
        assert!((c.position[1] - 20.0).abs() < f64::EPSILON);
        assert!((c.position[2] - 30.0).abs() < f64::EPSILON);
        drop(c);

        let result: LuaTable = lua
            .load("return llGetPrimitiveParams({PRIM_POSITION})")
            .eval()
            .unwrap();
        let pos: LuaTable = result.get(1).unwrap();
        let x: f64 = pos.get("x").unwrap();
        assert!((x - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_and_get_color() {
        let (lua, ctx) = test_vm();
        lua.load(
            "llSetPrimitiveParams({PRIM_COLOR, 0, {x=0.2, y=0.4, z=0.6}, 0.5})",
        )
        .exec()
        .unwrap();
        let c = ctx.read();
        assert!((c.color[0] - 0.2).abs() < 0.001);
        assert!((c.color[1] - 0.4).abs() < 0.001);
        assert!((c.color[2] - 0.6).abs() < 0.001);
        assert!((c.alpha - 0.5).abs() < 0.001);
    }

    #[test]
    fn set_and_get_name_desc() {
        let (lua, ctx) = test_vm();
        lua.load(r#"llSetPrimitiveParams({PRIM_NAME, "New Name", PRIM_DESC, "New Desc"})"#)
            .exec()
            .unwrap();
        let c = ctx.read();
        assert_eq!(c.prim_name, "New Name");
        assert_eq!(c.prim_desc, "New Desc");
        drop(c);

        let result: LuaTable = lua
            .load("return llGetPrimitiveParams({PRIM_NAME, PRIM_DESC})")
            .eval()
            .unwrap();
        let name: String = result.get(1).unwrap();
        let desc: String = result.get(2).unwrap();
        assert_eq!(name, "New Name");
        assert_eq!(desc, "New Desc");
    }

    #[test]
    fn multi_rule_set() {
        let (lua, ctx) = test_vm();
        lua.load(
            r#"llSetPrimitiveParams({
                PRIM_POSITION, {x=1, y=2, z=3},
                PRIM_SIZE, {x=4, y=5, z=6},
                PRIM_MATERIAL, 3
            })"#,
        )
        .exec()
        .unwrap();
        let c = ctx.read();
        assert!((c.position[0] - 1.0).abs() < f64::EPSILON);
        assert!((c.scale[1] - 5.0).abs() < f64::EPSILON);
        assert_eq!(c.prim_material, PRIM_MATERIAL_WOOD);
    }

    #[test]
    fn link_prim_params_applies_to_self() {
        let (lua, ctx) = test_vm();
        lua.load(
            "llSetLinkPrimitiveParams(LINK_THIS, {PRIM_POSITION, {x=50, y=60, z=70}})",
        )
        .exec()
        .unwrap();
        let c = ctx.read();
        assert!((c.position[0] - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn link_prim_params_fast_identical() {
        let (lua, ctx) = test_vm();
        lua.load(
            "llSetLinkPrimitiveParamsFast(0, {PRIM_PHANTOM, 1})",
        )
        .exec()
        .unwrap();
        assert!(ctx.read().is_phantom);
    }

    #[test]
    fn set_and_get_text() {
        let (lua, ctx) = test_vm();
        lua.load(
            r#"llSetPrimitiveParams({PRIM_TEXT, "Hello!", {x=1, y=0, z=0}, 1.0})"#,
        )
        .exec()
        .unwrap();
        let c = ctx.read();
        assert_eq!(c.hover_text, "Hello!");
        assert!((c.hover_text_color[0] - 1.0).abs() < f64::EPSILON);
        drop(c);

        let result: LuaTable = lua
            .load("return llGetPrimitiveParams({PRIM_TEXT})")
            .eval()
            .unwrap();
        let text: String = result.get(1).unwrap();
        assert_eq!(text, "Hello!");
    }

    #[test]
    fn set_physics_flags() {
        let (lua, ctx) = test_vm();
        lua.load(
            "llSetPrimitiveParams({PRIM_PHYSICS, 1, PRIM_TEMP_ON_REZ, 1})",
        )
        .exec()
        .unwrap();
        let c = ctx.read();
        assert!(c.is_physics);
        assert!(c.is_temp_on_rez);
    }

    #[test]
    fn constants_registered_as_globals() {
        let (lua, _) = test_vm();
        let val: i64 = lua.load("return PRIM_POSITION").eval().unwrap();
        assert_eq!(val, PRIM_POSITION as i64);
        let val: i64 = lua.load("return PRIM_TYPE_SCULPT").eval().unwrap();
        assert_eq!(val, PRIM_TYPE_SCULPT as i64);
        let val: i64 = lua.load("return LINK_THIS").eval().unwrap();
        assert_eq!(val, LINK_THIS as i64);
    }
}
