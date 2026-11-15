// SPDX-License-Identifier: GPL-3.0-or-later
//! Object ll* function stubs (llGetPos, llSetPos, llGetObjectName, etc.).
//!
//! These read from and write to the [`ScriptContext`] to provide basic
//! object property access.  Position, rotation, and scale are stored on the
//! context; the real implementation will forward changes to the scene graph.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::{ScriptContext, RezRequest, ObjectCommand, INVENTORY_OBJECT};

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

    // llGetOwner() -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.owner_id.to_string())
        })?;
        lua.globals().set("llGetOwner", f)?;
    }

    // llGetCreator() -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.creator_id.to_string())
        })?;
        lua.globals().set("llGetCreator", f)?;
    }

    // llGetNumberOfPrims() -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.prim_count as i64)
        })?;
        lua.globals().set("llGetNumberOfPrims", f)?;
    }

    // llGetLinkNumber() -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.link_num as i64)
        })?;
        lua.globals().set("llGetLinkNumber", f)?;
    }

    // llGetLinkKey(link_num) -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, link: i64| {
            let ctx = ctx.read();
            if link == 0 || link == 1 {
                Ok(ctx.prim_id.to_string())
            } else {
                Ok(uuid::Uuid::nil().to_string())
            }
        })?;
        lua.globals().set("llGetLinkKey", f)?;
    }

    // llDie() -> void
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            if let Some(ref tx) = ctx.object_command_sender {
                let _ = tx.send(ObjectCommand::Die {
                    object_id: ctx.prim_id,
                });
            }
            tracing::warn!(prim = %ctx.prim_id, "llDie: script requested object deletion");
            Ok(())
        })?;
        lua.globals().set("llDie", f)?;
    }

    // llGetColor(face) -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, _face: i64| {
            let ctx = ctx.read();
            make_vec3(lua, ctx.color)
        })?;
        lua.globals().set("llGetColor", f)?;
    }

    // llSetColor(color, face)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (color, _face): (LuaTable, i64)| {
            let v = get_vec3_from_table(&color)?;
            let mut ctx = ctx.write();
            ctx.color = v;
            Ok(())
        })?;
        lua.globals().set("llSetColor", f)?;
    }

    // llGetAlpha(face) -> float
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _face: i64| {
            let ctx = ctx.read();
            Ok(ctx.alpha)
        })?;
        lua.globals().set("llGetAlpha", f)?;
    }

    // llSetAlpha(alpha, face)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (alpha, _face): (f64, i64)| {
            let mut ctx = ctx.write();
            ctx.alpha = alpha.clamp(0.0, 1.0);
            Ok(())
        })?;
        lua.globals().set("llSetAlpha", f)?;
    }

    // llGetTexture(face) -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _face: i64| {
            let ctx = ctx.read();
            Ok(ctx.texture.to_string())
        })?;
        lua.globals().set("llGetTexture", f)?;
    }

    // llSetTexture(texture, face)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (texture, _face): (String, i64)| {
            let mut ctx = ctx.write();
            ctx.texture = uuid::Uuid::parse_str(&texture).unwrap_or(uuid::Uuid::nil());
            Ok(())
        })?;
        lua.globals().set("llSetTexture", f)?;
    }

    // OBJECT_* constants for llGetObjectDetails
    lua.globals().set("OBJECT_NAME", 1i64)?;
    lua.globals().set("OBJECT_DESC", 2i64)?;
    lua.globals().set("OBJECT_POS", 3i64)?;
    lua.globals().set("OBJECT_ROT", 4i64)?;
    lua.globals().set("OBJECT_VELOCITY", 5i64)?;
    lua.globals().set("OBJECT_OWNER", 6i64)?;
    lua.globals().set("OBJECT_GROUP", 7i64)?;
    lua.globals().set("OBJECT_CREATOR", 8i64)?;

    // llGetObjectDetails(id, params) -> list
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, (id, params): (String, LuaTable)| {
            let ctx = ctx.read();
            let result = lua.create_table()?;
            let prim_id_str = ctx.prim_id.to_string();
            if id != prim_id_str {
                return Ok(result);
            }
            let mut idx = 1i64;
            for pair in params.pairs::<i64, i64>() {
                let (_, param) = pair?;
                match param {
                    1 => { result.set(idx, ctx.prim_name.clone())?; }
                    2 => { result.set(idx, ctx.prim_desc.clone())?; }
                    3 => { result.set(idx, make_vec3(lua, ctx.position)?)?; }
                    4 => { result.set(idx, make_rot4(lua, ctx.rotation)?)?; }
                    5 => { result.set(idx, make_vec3(lua, [0.0, 0.0, 0.0])?)?; }
                    6 => { result.set(idx, ctx.owner_id.to_string())?; }
                    7 => { result.set(idx, uuid::Uuid::nil().to_string())?; }
                    8 => { result.set(idx, ctx.creator_id.to_string())?; }
                    _ => { result.set(idx, mlua::Value::Nil)?; }
                }
                idx += 1;
            }
            Ok(result)
        })?;
        lua.globals().set("llGetObjectDetails", f)?;
    }

    // llRezObject(name, pos, vel, rot, start_param)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (name, pos, vel, rot, param): (String, LuaTable, LuaTable, LuaTable, i64)| {
                let ctx = ctx.read();
                let item = ctx.inventory.iter().find(|i| i.name == name && i.item_type == INVENTORY_OBJECT);
                if item.is_none() {
                    return Err(LuaError::runtime(format!("llRezObject: item '{}' not found in inventory", name)));
                }
                if let Some(ref tx) = ctx.rez_sender {
                    let req = RezRequest {
                        source_script_id: ctx.script_id,
                        source_prim_id: ctx.prim_id,
                        item_name: name,
                        position: get_vec3_from_table(&pos)?,
                        velocity: get_vec3_from_table(&vel)?,
                        rotation: get_rot4_from_table(&rot)?,
                        start_param: param as i32,
                        at_root: false,
                    };
                    let _ = tx.send(req);
                }
                Ok(())
            },
        )?;
        lua.globals().set("llRezObject", f)?;
    }

    // llRezAtRoot(name, pos, vel, rot, start_param)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (name, pos, vel, rot, param): (String, LuaTable, LuaTable, LuaTable, i64)| {
                let ctx = ctx.read();
                let item = ctx.inventory.iter().find(|i| i.name == name && i.item_type == INVENTORY_OBJECT);
                if item.is_none() {
                    return Err(LuaError::runtime(format!("llRezAtRoot: item '{}' not found in inventory", name)));
                }
                if let Some(ref tx) = ctx.rez_sender {
                    let req = RezRequest {
                        source_script_id: ctx.script_id,
                        source_prim_id: ctx.prim_id,
                        item_name: name,
                        position: get_vec3_from_table(&pos)?,
                        velocity: get_vec3_from_table(&vel)?,
                        rotation: get_rot4_from_table(&rot)?,
                        start_param: param as i32,
                        at_root: true,
                    };
                    let _ = tx.send(req);
                }
                Ok(())
            },
        )?;
        lua.globals().set("llRezAtRoot", f)?;
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
            ..Default::default()
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

    #[test]
    fn ll_get_owner() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let owner: String = lua.load("return llGetOwner()").eval().unwrap();
        assert_eq!(owner, Uuid::nil().to_string());
    }

    #[test]
    fn ll_get_creator() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let creator: String = lua.load("return llGetCreator()").eval().unwrap();
        assert_eq!(creator, Uuid::nil().to_string());
    }

    #[test]
    fn ll_get_number_of_prims() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let n: i64 = lua.load("return llGetNumberOfPrims()").eval().unwrap();
        // test_ctx uses Default which sets prim_count = 1
        assert_eq!(n, 1);
    }

    #[test]
    fn ll_get_number_of_prims_from_context() {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_count: 5,
            ..Default::default()
        }));
        register_object(&lua, &ctx).unwrap();
        let n: i64 = lua.load("return llGetNumberOfPrims()").eval().unwrap();
        assert_eq!(n, 5);
    }

    #[test]
    fn ll_get_link_number() {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            link_num: 3,
            ..Default::default()
        }));
        register_object(&lua, &ctx).unwrap();
        let n: i64 = lua.load("return llGetLinkNumber()").eval().unwrap();
        assert_eq!(n, 3);
    }

    #[test]
    fn ll_get_link_key_root() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let key: String = lua.load("return llGetLinkKey(0)").eval().unwrap();
        assert_eq!(key, Uuid::from_u128(0xdead_beef).to_string());
    }

    #[test]
    fn ll_get_link_key_other() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let key: String = lua.load("return llGetLinkKey(5)").eval().unwrap();
        assert_eq!(key, Uuid::nil().to_string());
    }

    #[test]
    fn ll_die_does_not_panic() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        lua.load("llDie()").exec().unwrap();
    }

    #[test]
    fn ll_die_sends_object_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let prim_id = Uuid::from_u128(0xdead_beef);
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id,
            object_command_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_object(&lua, &ctx).unwrap();
        lua.load("llDie()").exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            super::super::ObjectCommand::Die { object_id } => {
                assert_eq!(object_id, prim_id);
            }
            _ => panic!("expected Die command"),
        }
    }

    #[test]
    fn ll_get_color() {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            color: [0.5, 0.6, 0.7],
            ..Default::default()
        }));
        register_object(&lua, &ctx).unwrap();
        let (r, g, b): (f64, f64, f64) = lua
            .load("local c = llGetColor(0); return c.x, c.y, c.z")
            .eval()
            .unwrap();
        assert!((r - 0.5).abs() < f64::EPSILON);
        assert!((g - 0.6).abs() < f64::EPSILON);
        assert!((b - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_set_color_updates_context() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        lua.load("llSetColor({x=0.1, y=0.2, z=0.3}, 0)")
            .exec()
            .unwrap();
        let c = ctx.read().color;
        assert!((c[0] - 0.1).abs() < f64::EPSILON);
        assert!((c[1] - 0.2).abs() < f64::EPSILON);
        assert!((c[2] - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_get_alpha() {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            alpha: 0.75,
            ..Default::default()
        }));
        register_object(&lua, &ctx).unwrap();
        let a: f64 = lua.load("return llGetAlpha(0)").eval().unwrap();
        assert!((a - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_set_alpha_clamps() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        lua.load("llSetAlpha(1.5, 0)").exec().unwrap();
        assert!((ctx.read().alpha - 1.0).abs() < f64::EPSILON);
        lua.load("llSetAlpha(-0.5, 0)").exec().unwrap();
        assert!((ctx.read().alpha - 0.0).abs() < f64::EPSILON);
        lua.load("llSetAlpha(0.5, 0)").exec().unwrap();
        assert!((ctx.read().alpha - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_get_texture() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let tex: String = lua.load("return llGetTexture(0)").eval().unwrap();
        assert_eq!(tex, Uuid::nil().to_string());
    }

    #[test]
    fn ll_set_texture_updates() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let test_uuid = "12345678-1234-1234-1234-123456789abc";
        lua.load(&format!(r#"llSetTexture("{test_uuid}", 0)"#))
            .exec()
            .unwrap();
        assert_eq!(ctx.read().texture.to_string(), test_uuid);
    }

    #[test]
    fn ll_get_object_details_known_prim() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let prim_id = ctx.read().prim_id.to_string();
        let script = format!(
            r#"
            local d = llGetObjectDetails("{prim_id}", {{OBJECT_NAME, OBJECT_DESC}})
            return d[1], d[2]
        "#
        );
        let (name, desc): (String, String) = lua.load(&script).eval().unwrap();
        assert_eq!(name, "My Prim");
        assert_eq!(desc, "A test prim");
    }

    #[test]
    fn ll_get_object_details_unknown_returns_empty() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_object(&lua, &ctx).unwrap();
        let script = r#"
            local d = llGetObjectDetails("00000000-0000-0000-0000-ffffffffffff", {OBJECT_NAME})
            return #d
        "#;
        let count: i64 = lua.load(script).eval().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn ll_rez_object_sends_request() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::from_u128(0x1),
            prim_id: Uuid::from_u128(0x2),
            inventory: vec![super::super::InventoryItem {
                name: "MyBox".into(),
                item_type: super::super::INVENTORY_OBJECT,
                asset_id: Uuid::from_u128(0x99),
                ..Default::default()
            }],
            rez_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_object(&lua, &ctx).unwrap();
        lua.load(r#"llRezObject("MyBox", {x=10,y=20,z=30}, {x=0,y=0,z=0}, {x=0,y=0,z=0,w=1}, 42)"#)
            .exec()
            .unwrap();
        let req = rx.try_recv().unwrap();
        assert_eq!(req.item_name, "MyBox");
        assert!((req.position[0] - 10.0).abs() < f64::EPSILON);
        assert_eq!(req.start_param, 42);
        assert!(!req.at_root);
    }

    #[test]
    fn ll_rez_object_errors_on_missing_item() {
        let ctx = Arc::new(RwLock::new(ScriptContext {
            inventory: Vec::new(),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_object(&lua, &ctx).unwrap();
        let result = lua.load(r#"llRezObject("Missing", {x=0,y=0,z=0}, {x=0,y=0,z=0}, {x=0,y=0,z=0,w=1}, 0)"#)
            .exec();
        assert!(result.is_err());
    }

    #[test]
    fn ll_rez_at_root_sets_at_root_flag() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            inventory: vec![super::super::InventoryItem {
                name: "MyBox".into(),
                item_type: super::super::INVENTORY_OBJECT,
                asset_id: Uuid::from_u128(0x99),
                ..Default::default()
            }],
            rez_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_object(&lua, &ctx).unwrap();
        lua.load(r#"llRezAtRoot("MyBox", {x=1,y=2,z=3}, {x=0,y=0,z=0}, {x=0,y=0,z=0,w=1}, 7)"#)
            .exec()
            .unwrap();
        let req = rx.try_recv().unwrap();
        assert!(req.at_root);
        assert_eq!(req.start_param, 7);
    }
}
