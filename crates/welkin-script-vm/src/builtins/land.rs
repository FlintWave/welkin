// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL land/parcel built-in functions.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

const PARCEL_DETAILS_NAME: i64 = 0;
const PARCEL_DETAILS_DESC: i64 = 1;
const PARCEL_DETAILS_OWNER: i64 = 2;
const PARCEL_DETAILS_GROUP: i64 = 3;
const PARCEL_DETAILS_AREA: i64 = 4;
const PARCEL_DETAILS_ID: i64 = 5;
const PARCEL_DETAILS_SEE_AVATARS: i64 = 6;

pub fn register_land(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    let g = lua.globals();

    g.set("PARCEL_DETAILS_NAME", PARCEL_DETAILS_NAME)?;
    g.set("PARCEL_DETAILS_DESC", PARCEL_DETAILS_DESC)?;
    g.set("PARCEL_DETAILS_OWNER", PARCEL_DETAILS_OWNER)?;
    g.set("PARCEL_DETAILS_GROUP", PARCEL_DETAILS_GROUP)?;
    g.set("PARCEL_DETAILS_AREA", PARCEL_DETAILS_AREA)?;
    g.set("PARCEL_DETAILS_ID", PARCEL_DETAILS_ID)?;
    g.set("PARCEL_DETAILS_SEE_AVATARS", PARCEL_DETAILS_SEE_AVATARS)?;

    // llGetParcelDetails(pos, params) -> list
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, (_pos, params): (LuaValue, LuaTable)| {
            let ctx = ctx.read();
            let result = lua.create_table()?;
            let mut idx = 1i64;
            for pair in params.sequence_values::<i64>() {
                let param = pair?;
                let val: LuaValue = match param {
                    0 => LuaValue::String(lua.create_string(&ctx.parcel_name)?),
                    1 => LuaValue::String(lua.create_string(&ctx.parcel_description)?),
                    2 => LuaValue::String(lua.create_string(&ctx.parcel_owner.to_string())?),
                    3 => LuaValue::String(lua.create_string(&uuid::Uuid::nil().to_string())?),
                    4 => LuaValue::Integer(ctx.parcel_area as i64),
                    5 => LuaValue::String(lua.create_string(&uuid::Uuid::nil().to_string())?),
                    6 => LuaValue::Integer(1),
                    _ => LuaValue::String(lua.create_string("")?),
                };
                result.set(idx, val)?;
                idx += 1;
            }
            Ok(result)
        })?;
        g.set("llGetParcelDetails", f)?;
    }

    // llGetParcelFlags(pos) -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _pos: LuaValue| {
            let ctx = ctx.read();
            Ok(ctx.parcel_flags as i64)
        })?;
        g.set("llGetParcelFlags", f)?;
    }

    // llGetParcelMaxPrims(pos, sim_wide) -> integer
    {
        let f = lua.create_function(|_, (_pos, _sim_wide): (LuaValue, i64)| Ok(15000i64))?;
        g.set("llGetParcelMaxPrims", f)?;
    }

    // llGetParcelPrimCount(pos, category, sim_wide) -> integer
    {
        let f = lua.create_function(|_, (_pos, _cat, _sw): (LuaValue, i64, i64)| Ok(0i64))?;
        g.set("llGetParcelPrimCount", f)?;
    }

    // llOverMyLand(id) -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, id: String| {
            let ctx = ctx.read();
            let matches = id == ctx.parcel_owner.to_string();
            Ok(if matches { 1i64 } else { 0i64 })
        })?;
        g.set("llOverMyLand", f)?;
    }

    // llGetLandOwnerAt(pos) -> key
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _pos: LuaValue| {
            let ctx = ctx.read();
            Ok(ctx.parcel_owner.to_string())
        })?;
        g.set("llGetLandOwnerAt", f)?;
    }

    // llSetParcelMusicURL(url)
    {
        let f = lua.create_function(|_, url: String| {
            tracing::debug!(%url, "llSetParcelMusicURL");
            Ok(())
        })?;
        g.set("llSetParcelMusicURL", f)?;
    }

    // llSetParcelMediaURL(url)
    {
        let f = lua.create_function(|_, url: String| {
            tracing::debug!(%url, "llSetParcelMediaURL");
            Ok(())
        })?;
        g.set("llSetParcelMediaURL", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        let owner = Uuid::from_u128(0xABCD);
        Arc::new(RwLock::new(ScriptContext {
            parcel_name: "Test Parcel".into(),
            parcel_description: "A parcel for testing".into(),
            parcel_owner: owner,
            parcel_area: 512,
            parcel_flags: 0xFF,
            ..Default::default()
        }))
    }

    #[test]
    fn get_parcel_flags() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_land(&lua, &ctx).unwrap();
        let r: i64 = lua
            .load("return llGetParcelFlags({x=0,y=0,z=0})")
            .eval()
            .unwrap();
        assert_eq!(r, 0xFF);
    }

    #[test]
    fn over_my_land_match() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_land(&lua, &ctx).unwrap();
        let owner_str = ctx.read().parcel_owner.to_string();
        let script = format!("return llOverMyLand(\"{}\")", owner_str);
        let r: i64 = lua.load(&script).eval().unwrap();
        assert_eq!(r, 1);
    }

    #[test]
    fn over_my_land_no_match() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_land(&lua, &ctx).unwrap();
        let r: i64 = lua
            .load(&format!(
                "return llOverMyLand(\"{}\")",
                Uuid::nil()
            ))
            .eval()
            .unwrap();
        assert_eq!(r, 0);
    }

    #[test]
    fn get_land_owner_at() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_land(&lua, &ctx).unwrap();
        let owner: String = lua
            .load("return llGetLandOwnerAt({x=0,y=0,z=0})")
            .eval()
            .unwrap();
        assert_eq!(owner, ctx.read().parcel_owner.to_string());
    }

    #[test]
    fn get_parcel_details() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_land(&lua, &ctx).unwrap();
        crate::builtins::math::register_math(&lua).unwrap();
        let script = r#"
            local d = llGetParcelDetails(
                {x=128,y=128,z=0},
                {PARCEL_DETAILS_NAME, PARCEL_DETAILS_AREA}
            )
            return d[1], d[2]
        "#;
        let (name, area): (String, i64) = lua.load(script).eval().unwrap();
        assert_eq!(name, "Test Parcel");
        assert_eq!(area, 512);
    }
}
