// SPDX-License-Identifier: GPL-3.0-or-later
//! Avatar ll* functions (permissions, animations, sit targets, etc.).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

pub fn register_avatar(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), crate::VmError> {
    // PERMISSION_* constants
    lua.globals().set("PERMISSION_DEBIT", 0x2i64)?;
    lua.globals().set("PERMISSION_TRIGGER_ANIMATION", 0x10i64)?;
    lua.globals().set("PERMISSION_ATTACH", 0x20i64)?;
    lua.globals().set("PERMISSION_CHANGE_LINKS", 0x80i64)?;
    lua.globals().set("PERMISSION_TRACK_CAMERA", 0x400i64)?;
    lua.globals().set("PERMISSION_CONTROL_CAMERA", 0x800i64)?;
    lua.globals().set("PERMISSION_TELEPORT", 0x1000i64)?;
    lua.globals().set("PERMISSION_TAKE_CONTROLS", 0x4i64)?;

    // llRequestPermissions(agent_id, permissions)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_agent_id, perms): (String, i64)| {
            let mut ctx = ctx.write();
            ctx.permissions_granted = perms as u32;
            tracing::debug!(perms, "llRequestPermissions: auto-granting");
            Ok(())
        })?;
        lua.globals().set("llRequestPermissions", f)?;
    }

    // llGetPermissions() -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.permissions_granted as i64)
        })?;
        lua.globals().set("llGetPermissions", f)?;
    }

    // llGetPermissionsKey() -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            if ctx.permissions_granted != 0 {
                Ok(ctx.owner_id.to_string())
            } else {
                Ok(uuid::Uuid::nil().to_string())
            }
        })?;
        lua.globals().set("llGetPermissionsKey", f)?;
    }

    // llStartAnimation(anim_name)
    {
        let f = lua.create_function(|_, anim: String| {
            tracing::debug!(%anim, "llStartAnimation");
            Ok(())
        })?;
        lua.globals().set("llStartAnimation", f)?;
    }

    // llStopAnimation(anim_name)
    {
        let f = lua.create_function(|_, anim: String| {
            tracing::debug!(%anim, "llStopAnimation");
            Ok(())
        })?;
        lua.globals().set("llStopAnimation", f)?;
    }

    // llSitTarget(offset, rot)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (offset, rot): (LuaTable, LuaTable)| {
            let pos = [
                offset.get::<f64>("x").or_else(|_| offset.get(1))?,
                offset.get::<f64>("y").or_else(|_| offset.get(2))?,
                offset.get::<f64>("z").or_else(|_| offset.get(3))?,
            ];
            let r = [
                rot.get::<f64>("x").or_else(|_| rot.get(1))?,
                rot.get::<f64>("y").or_else(|_| rot.get(2))?,
                rot.get::<f64>("z").or_else(|_| rot.get(3))?,
                rot.get::<f64>("w").or_else(|_| rot.get(4))?,
            ];
            let mut ctx = ctx.write();
            ctx.sit_target_pos = pos;
            ctx.sit_target_rot = r;
            Ok(())
        })?;
        lua.globals().set("llSitTarget", f)?;
    }

    // llAvatarOnSitTarget() -> string (UUID of sitting avatar, or NULL_KEY)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.sitting_avatar.unwrap_or(uuid::Uuid::nil()).to_string())
        })?;
        lua.globals().set("llAvatarOnSitTarget", f)?;
    }

    // llUnSit(agent_id)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _agent_id: String| {
            let mut ctx = ctx.write();
            ctx.sitting_avatar = None;
            Ok(())
        })?;
        lua.globals().set("llUnSit", f)?;
    }

    // llGetAgentSize(agent_id) -> vector
    {
        let f = lua.create_function(|lua, _agent_id: String| {
            let t = lua.create_table()?;
            t.set("x", 0.45)?;
            t.set("y", 0.6)?;
            t.set("z", 1.8)?;
            Ok(t)
        })?;
        lua.globals().set("llGetAgentSize", f)?;
    }

    // llGetAgentInfo(agent_id) -> integer
    {
        let f = lua.create_function(|_, _agent_id: String| Ok(0i64))?;
        lua.globals().set("llGetAgentInfo", f)?;
    }

    // llTeleportAgent(agent_id, landmark, pos, look_at)
    {
        let f = lua.create_function(
            |_, (_agent_id, _landmark, _pos, _look_at): (String, String, LuaTable, LuaTable)| {
                tracing::debug!("llTeleportAgent");
                Ok(())
            },
        )?;
        lua.globals().set("llTeleportAgent", f)?;
    }

    // llTeleportAgentHome(agent_id)
    {
        let f = lua.create_function(|_, _agent_id: String| {
            tracing::debug!("llTeleportAgentHome");
            Ok(())
        })?;
        lua.globals().set("llTeleportAgentHome", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            owner_id: Uuid::from_u128(0xabcd),
            ..Default::default()
        }))
    }

    #[test]
    fn request_permissions_auto_grants() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        lua.load(r#"llRequestPermissions("00000000-0000-0000-0000-000000000000", PERMISSION_TRIGGER_ANIMATION)"#)
            .exec()
            .unwrap();
        assert_eq!(ctx.read().permissions_granted, 0x10);
    }

    #[test]
    fn get_permissions_returns_granted() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        lua.load(r#"llRequestPermissions("00000000-0000-0000-0000-000000000000", PERMISSION_DEBIT)"#)
            .exec()
            .unwrap();
        let perms: i64 = lua.load("return llGetPermissions()").eval().unwrap();
        assert_eq!(perms, 0x2);
    }

    #[test]
    fn sit_target_updates_context() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        lua.load("llSitTarget({x=0, y=0, z=0.5}, {x=0, y=0, z=0, w=1})")
            .exec()
            .unwrap();
        let pos = ctx.read().sit_target_pos;
        assert!((pos[2] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn avatar_on_sit_target_nil_when_nobody() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        let key: String = lua.load("return llAvatarOnSitTarget()").eval().unwrap();
        assert_eq!(key, Uuid::nil().to_string());
    }

    #[test]
    fn unsit_clears_sitting_avatar() {
        let lua = Lua::new();
        let ctx = test_ctx();
        ctx.write().sitting_avatar = Some(Uuid::from_u128(0x1234));
        register_avatar(&lua, &ctx).unwrap();
        lua.load(r#"llUnSit("00000000-0000-0000-0000-000000001234")"#)
            .exec()
            .unwrap();
        assert!(ctx.read().sitting_avatar.is_none());
    }

    #[test]
    fn get_agent_size_returns_defaults() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        let z: f64 = lua
            .load(r#"return llGetAgentSize("00000000-0000-0000-0000-000000000000").z"#)
            .eval()
            .unwrap();
        assert!((z - 1.8).abs() < f64::EPSILON);
    }
}
