// SPDX-License-Identifier: GPL-3.0-or-later
//! Object ll* function stubs (llGetPos, llSetPos, llGetObjectName, etc.).
//!
//! These read from and write to the [`ScriptContext`] to provide basic
//! object property access.  Position, rotation, and scale are stored on the
//! context; the real implementation will forward changes to the scene graph.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

/// Extract `[x, y, z]` from a Lua table that has named or positional keys.
fn get_vec3_from_table(t: &LuaTable) -> LuaResult<[f64; 3]> {
    let x: f64 = t.get("x").or_else(|_| t.get(1))?;
    let y: f64 = t.get("y").or_else(|_| t.get(2))?;
    let z: f64 = t.get("z").or_else(|_| t.get(3))?;
    Ok([x, y, z])
}

/// Extract `[x, y, z, w]` from a Lua table that has named or positional keys.
fn get_rot4_from_table(t: &LuaTable) -> LuaResult<[f64; 4]> {
    let x: f64 = t.get("x").or_else(|_| t.get(1))?;
    let y: f64 = t.get("y").or_else(|_| t.get(2))?;
    let z: f64 = t.get("z").or_else(|_| t.get(3))?;
    let w: f64 = t.get("w").or_else(|_| t.get(4))?;
    Ok([x, y, z, w])
}

/// Build a Lua table `{x, y, z}` from a 3-component array.
fn make_vec3(lua: &Lua, v: [f64; 3]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    t.set("z", v[2])?;
    Ok(t)
}

/// Build a Lua table `{x, y, z, w}` from a 4-component array.
fn make_rot4(lua: &Lua, r: [f64; 4]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", r[0])?;
    t.set("y", r[1])?;
    t.set("z", r[2])?;
    t.set("w", r[3])?;
    Ok(t)
}

/// Register all object ll* functions as Lua globals.
pub fn register_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), crate::VmError> {
    // llGetPos() -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let ctx = ctx.read();
            tracing::debug!("llGetPos");
            make_vec3(lua, ctx.position)
        })?;
        lua.globals().set("llGetPos", f)?;
    }

    // llSetPos(pos)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, pos: LuaTable| {
            let v = get_vec3_from_table(&pos)?;
            let mut ctx = ctx.write();
            tracing::debug!(x = v[0], y = v[1], z = v[2], "llSetPos");
            ctx.position = v;
            Ok(())
        })?;
        lua.globals().set("llSetPos", f)?;
    }

    // llGetRot() -> rotation
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let ctx = ctx.read();
            tracing::debug!("llGetRot");
            make_rot4(lua, ctx.rotation)
        })?;
        lua.globals().set("llGetRot", f)?;
    }

    // llSetRot(rot)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, rot: LuaTable| {
            let r = get_rot4_from_table(&rot)?;
            let mut ctx = ctx.write();
            tracing::debug!(x = r[0], y = r[1], z = r[2], w = r[3], "llSetRot");
            ctx.rotation = r;
            Ok(())
        })?;
        lua.globals().set("llSetRot", f)?;
    }

    // llGetScale() -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let ctx = ctx.read();
            tracing::debug!("llGetScale");
            make_vec3(lua, ctx.scale)
        })?;
        lua.globals().set("llGetScale", f)?;
    }

    // llSetScale(scale)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, scale: LuaTable| {
            let v = get_vec3_from_table(&scale)?;
            let mut ctx = ctx.write();
            tracing::debug!(x = v[0], y = v[1], z = v[2], "llSetScale");
            ctx.scale = v;
            Ok(())
        })?;
        lua.globals().set("llSetScale", f)?;
    }

    // llGetObjectName() -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            tracing::debug!("llGetObjectName");
            Ok(ctx.prim_name.clone())
        })?;
        lua.globals().set("llGetObjectName", f)?;
    }

    // llSetObjectName(name)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, name: String| {
            let mut ctx = ctx.write();
            tracing::debug!(%name, "llSetObjectName");
            ctx.prim_name = name;
            Ok(())
        })?;
        lua.globals().set("llSetObjectName", f)?;
    }

    // llGetObjectDesc() -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            tracing::debug!("llGetObjectDesc");
            Ok(ctx.prim_desc.clone())
        })?;
        lua.globals().set("llGetObjectDesc", f)?;
    }

    // llSetObjectDesc(desc)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, desc: String| {
            let mut ctx = ctx.write();
            tracing::debug!(%desc, "llSetObjectDesc");
            ctx.prim_desc = desc;
            Ok(())
        })?;
        lua.globals().set("llSetObjectDesc", f)?;
    }

    // llGetKey() -> string (UUID of the prim)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            tracing::debug!("llGetKey");
            Ok(ctx.prim_id.to_string())
        })?;
        lua.globals().set("llGetKey", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::nil(),
            prim_id: Uuid::from_u128(0xdead_beef),
            prim_name: "My Prim".into(),
            prim_desc: "A test prim".into(),
            owner_id: Uuid::nil(),
            position: [10.0, 20.0, 30.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 2.0, 3.0],
        }))
    }

    #[test]
    fn ll_get_object_name() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let name: String = lua.load("return llGetObjectName()").eval().unwrap();
        assert_eq!(name, "My Prim");
    }

    #[test]
    fn ll_set_object_name() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        lua.load(r#"llSetObjectName("New Name")"#).exec().unwrap();
        assert_eq!(ctx.read().prim_name, "New Name");
    }

    #[test]
    fn ll_get_pos_returns_position() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();

        let script = r#"
            local p = llGetPos()
            return p.x, p.y, p.z
        "#;
        let (x, y, z): (f64, f64, f64) = lua.load(script).eval().unwrap();
        assert!((x - 10.0).abs() < f64::EPSILON);
        assert!((y - 20.0).abs() < f64::EPSILON);
        assert!((z - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_set_pos_updates_context() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        lua.load("llSetPos({x=5.0, y=6.0, z=7.0})").exec().unwrap();
        let pos = ctx.read().position;
        assert!((pos[0] - 5.0).abs() < f64::EPSILON);
        assert!((pos[1] - 6.0).abs() < f64::EPSILON);
        assert!((pos[2] - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_get_scale_returns_scale() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();

        let script = r#"
            local s = llGetScale()
            return s.x, s.y, s.z
        "#;
        let (x, y, z): (f64, f64, f64) = lua.load(script).eval().unwrap();
        assert!((x - 1.0).abs() < f64::EPSILON);
        assert!((y - 2.0).abs() < f64::EPSILON);
        assert!((z - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_get_key_returns_prim_uuid() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let key: String = lua.load("return llGetKey()").eval().unwrap();
        assert_eq!(key, Uuid::from_u128(0xdead_beef).to_string());
    }

    #[test]
    fn ll_get_object_desc() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let desc: String = lua.load("return llGetObjectDesc()").eval().unwrap();
        assert_eq!(desc, "A test prim");
    }

    #[test]
    fn ll_set_object_desc_updates() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        lua.load(r#"llSetObjectDesc("new desc")"#).exec().unwrap();
        assert_eq!(ctx.read().prim_desc, "new desc");
    }

    #[test]
    fn ll_get_rot_returns_rotation() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();

        let script = r#"
            local r = llGetRot()
            return r.x, r.y, r.z, r.w
        "#;
        let (x, y, z, w): (f64, f64, f64, f64) = lua.load(script).eval().unwrap();
        assert!((x - 0.0).abs() < f64::EPSILON);
        assert!((y - 0.0).abs() < f64::EPSILON);
        assert!((z - 0.0).abs() < f64::EPSILON);
        assert!((w - 1.0).abs() < f64::EPSILON);
    }
}
