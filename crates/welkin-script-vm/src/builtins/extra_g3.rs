// SPDX-License-Identifier: LGPL-3.0-or-later

use super::ScriptContext;
use parking_lot::RwLock;
use std::sync::Arc;

fn notify_unsupported(ctx: &Arc<RwLock<ScriptContext>>, name: &str) {
    let (prim_id, prim_name, owner_id, source_pos, sender) = {
        let ctx = ctx.read();
        (
            ctx.prim_id,
            ctx.prim_name.clone(),
            ctx.owner_id,
            ctx.position,
            ctx.chat_sender.clone(),
        )
    };

    if let Some(sender) = sender {
        let _ = sender.send(crate::ChatMessage {
            source_id: prim_id,
            source_name: prim_name,
            source_pos,
            owner_id,
            channel: 0,
            message: format!("{} is not supported on this region", name),
            chat_type: crate::ChatType::Owner,
            target_id: Some(owner_id),
        });
    }
}

pub fn register_extra_g3(
    lua: &mlua::Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSleep",
            lua.create_function(move |_lua, seconds: f64| {
                let mut ctx = ctx.write();
                ctx.sleep_requested += seconds.max(0.0);
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llResetScript",
            lua.create_function(move |_lua, ()| {
                let mut ctx = ctx.write();
                ctx.reset_requested = true;
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSameGroup",
            lua.create_function(move |_lua, agent: String| {
                let uuid = match uuid::Uuid::parse_str(&agent) {
                    Ok(uuid) => uuid,
                    Err(_) => return Ok(0),
                };
                if uuid.is_nil() {
                    return Ok(0);
                }
                let ctx = ctx.read();
                Ok(if uuid == ctx.owner_id { 1 } else { 0 })
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llRotateTexture",
            lua.create_function(move |_lua, (_rotation, _face): (f64, i32)| {
                notify_unsupported(&ctx, "llRotateTexture");
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llLinkParticleSystem",
            lua.create_function(move |_lua, (_linknum, _rules): (i32, Option<mlua::Table>)| {
                notify_unsupported(&ctx, "llLinkParticleSystem");
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetLinkMedia",
            lua.create_function(move |_lua, (_link, _face, _rules): (i32, i32, Option<mlua::Table>)| {
                notify_unsupported(&ctx, "llSetLinkMedia");
                Ok(0)
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llMapDestination",
            lua.create_function(move |_lua, (_simname, _pos, _look_at): (String, mlua::Table, mlua::Table)| {
                notify_unsupported(&ctx, "llMapDestination");
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llRequestSimulatorData",
            lua.create_function(move |_lua, (_simulator, _data): (String, i32)| {
                notify_unsupported(&ctx, "llRequestSimulatorData");
                Ok(uuid::Uuid::nil().to_string())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetAnimation",
            lua.create_function(move |_lua, id: String| {
                let uuid = match uuid::Uuid::parse_str(&id) {
                    Ok(uuid) => uuid,
                    Err(_) => return Ok(String::new()),
                };
                if uuid.is_nil() {
                    return Ok(String::new());
                }
                notify_unsupported(&ctx, "llGetAnimation");
                Ok(String::new())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetAnimationList",
            lua.create_function(move |lua, id: String| {
                let uuid = match uuid::Uuid::parse_str(&id) {
                    Ok(uuid) => uuid,
                    Err(_) => return Ok(lua.create_table()?),
                };
                if uuid.is_nil() {
                    return Ok(lua.create_table()?);
                }
                notify_unsupported(&ctx, "llGetAnimationList");
                lua.create_table()
            })?,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::ScriptContext;
    use crate::{ChatMessage, ChatType};
    use mlua::Lua;
    use parking_lot::RwLock;
    use std::sync::mpsc;
    use std::sync::Arc;
    use uuid::Uuid;

    fn plain_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext::default()))
    }

    fn ctx_with_sender() -> (Arc<RwLock<ScriptContext>>, mpsc::Receiver<ChatMessage>) {
        let (sender, receiver) = mpsc::channel();
        let mut ctx = ScriptContext::default();
        ctx.chat_sender = Some(sender);
        (Arc::new(RwLock::new(ctx)), receiver)
    }

    #[test]
    fn sleep_records_and_clamps() {
        let lua = Lua::new();
        let ctx = plain_ctx();
        register_extra_g3(&lua, &ctx).unwrap();

        lua.load("llSleep(2.25)").exec().unwrap();
        assert_eq!(ctx.read().sleep_requested, 2.25);

        lua.load("llSleep(-3.0)").exec().unwrap();
        assert_eq!(ctx.read().sleep_requested, 2.25);
    }

    #[test]
    fn reset_script_sets_flag() {
        let lua = Lua::new();
        let ctx = plain_ctx();
        register_extra_g3(&lua, &ctx).unwrap();

        lua.load("llResetScript()").exec().unwrap();
        assert!(ctx.read().reset_requested);
    }

    #[test]
    fn same_group_owner_matches() {
        let owner = Uuid::new_v4();
        let mut ctx = ScriptContext::default();
        ctx.owner_id = owner;
        let ctx = Arc::new(RwLock::new(ctx));
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        let result: i32 = lua.load(&format!(r#"llSameGroup("{}")"#, owner)).eval().unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn same_group_other_or_zero_does_not_match() {
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mut ctx = ScriptContext::default();
        ctx.owner_id = owner;
        let ctx = Arc::new(RwLock::new(ctx));
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        let other_result: i32 = lua.load(&format!(r#"llSameGroup("{}")"#, other)).eval().unwrap();
        assert_eq!(other_result, 0);

        let zero_result: i32 = lua.load(r#"llSameGroup("00000000-0000-0000-0000-000000000000")"#).eval().unwrap();
        assert_eq!(zero_result, 0);
    }

    #[test]
    fn rotate_texture_notifies_owner() {
        let (ctx, rx) = ctx_with_sender();
        let owner = Uuid::new_v4();
        ctx.write().owner_id = owner;
        ctx.write().prim_id = Uuid::new_v4();
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        lua.load("llRotateTexture(0.5, 1)").exec().unwrap();
        let message = rx.recv().unwrap();
        assert_eq!(message.message, "llRotateTexture is not supported on this region");
        assert_eq!(message.channel, 0);
        assert!(matches!(message.chat_type, ChatType::Owner));
        assert_eq!(message.target_id, Some(owner));
    }

    #[test]
    fn rotate_texture_without_sender_does_not_panic() {
        let lua = Lua::new();
        let ctx = plain_ctx();
        register_extra_g3(&lua, &ctx).unwrap();

        lua.load("llRotateTexture(0.5, 1)").exec().unwrap();
    }

    #[test]
    fn link_particle_system_notifies_owner() {
        let (ctx, rx) = ctx_with_sender();
        let owner = Uuid::new_v4();
        ctx.write().owner_id = owner;
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        lua.load("llLinkParticleSystem(2, {})").exec().unwrap();
        let message = rx.recv().unwrap();
        assert_eq!(message.message, "llLinkParticleSystem is not supported on this region");
    }

    #[test]
    fn set_link_media_returns_default_and_notifies() {
        let (ctx, rx) = ctx_with_sender();
        let owner = Uuid::new_v4();
        ctx.write().owner_id = owner;
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        let result: i32 = lua.load("llSetLinkMedia(1, 0, {})").eval().unwrap();
        assert_eq!(result, 0);

        let message = rx.recv().unwrap();
        assert_eq!(message.message, "llSetLinkMedia is not supported on this region");
    }

    #[test]
    fn map_destination_notifies_owner() {
        let (ctx, rx) = ctx_with_sender();
        let owner = Uuid::new_v4();
        ctx.write().owner_id = owner;
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        lua.load(r#"llMapDestination("Sim", {x=1,y=2,z=3}, {x=0,y=0,z=0})"#)
            .exec()
            .unwrap();
        let message = rx.recv().unwrap();
        assert_eq!(message.message, "llMapDestination is not supported on this region");
    }

    #[test]
    fn request_simulator_data_returns_nil_key_and_notifies() {
        let (ctx, rx) = ctx_with_sender();
        let owner = Uuid::new_v4();
        ctx.write().owner_id = owner;
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        let result: String = lua
            .load(r#"llRequestSimulatorData("Sim", 1)"#)
            .eval()
            .unwrap();
        assert_eq!(result, Uuid::nil().to_string());

        let message = rx.recv().unwrap();
        assert_eq!(message.message, "llRequestSimulatorData is not supported on this region");
    }

    #[test]
    fn get_animation_invalid_returns_empty_without_notification() {
        let (ctx, rx) = ctx_with_sender();
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        let result: String = lua
            .load(r#"llGetAnimation("00000000-0000-0000-0000-000000000000")"#)
            .eval()
            .unwrap();
        assert!(result.is_empty());

        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn get_animation_valid_notifies_owner() {
        let (ctx, rx) = ctx_with_sender();
        let owner = Uuid::new_v4();
        ctx.write().owner_id = owner;
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        let result: String = lua.load(&format!(r#"llGetAnimation("{}")"#, owner)).eval().unwrap();
        assert!(result.is_empty());

        let message = rx.recv().unwrap();
        assert_eq!(message.message, "llGetAnimation is not supported on this region");
    }

    #[test]
    fn get_animation_list_empty_and_notifies() {
        let (ctx, rx) = ctx_with_sender();
        let owner = Uuid::new_v4();
        ctx.write().owner_id = owner;
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        let length: i64 = lua
            .load(&format!(r#"#llGetAnimationList("{}")"#, owner))
            .eval()
            .unwrap();
        assert_eq!(length, 0);

        let message = rx.recv().unwrap();
        assert_eq!(message.message, "llGetAnimationList is not supported on this region");
    }

    #[test]
    fn get_animation_list_invalid_returns_empty_without_notification() {
        let (ctx, rx) = ctx_with_sender();
        let lua = Lua::new();
        register_extra_g3(&lua, &ctx).unwrap();

        let length: i64 = lua
            .load(r#"#llGetAnimationList("00000000-0000-0000-0000-000000000000")"#)
            .eval()
            .unwrap();
        assert_eq!(length, 0);

        assert!(rx.try_recv().is_err());
    }
}
