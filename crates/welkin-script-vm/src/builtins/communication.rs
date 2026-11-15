// SPDX-License-Identifier: GPL-3.0-or-later
//! Communication ll* function stubs (llSay, llShout, llWhisper, etc.).
//!
//! These are placeholder implementations that log calls via `tracing::debug!`.
//! Real implementations will route messages through the region message system.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

/// Register all communication ll* functions as Lua globals.
pub fn register_communication(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llSay(channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (channel, message): (i64, String)| {
            let _ctx = ctx.read();
            tracing::debug!(channel, %message, "llSay");
            Ok(())
        })?;
        lua.globals().set("llSay", f)?;
    }

    // llShout(channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (channel, message): (i64, String)| {
            let _ctx = ctx.read();
            tracing::debug!(channel, %message, "llShout");
            Ok(())
        })?;
        lua.globals().set("llShout", f)?;
    }

    // llWhisper(channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (channel, message): (i64, String)| {
            let _ctx = ctx.read();
            tracing::debug!(channel, %message, "llWhisper");
            Ok(())
        })?;
        lua.globals().set("llWhisper", f)?;
    }

    // llRegionSay(channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (channel, message): (i64, String)| {
            let _ctx = ctx.read();
            tracing::debug!(channel, %message, "llRegionSay");
            Ok(())
        })?;
        lua.globals().set("llRegionSay", f)?;
    }

    // llRegionSayTo(target, channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (target, channel, message): (String, i64, String)| {
                let _ctx = ctx.read();
                tracing::debug!(%target, channel, %message, "llRegionSayTo");
                Ok(())
            },
        )?;
        lua.globals().set("llRegionSayTo", f)?;
    }

    // llOwnerSay(message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, message: String| {
            let ctx = ctx.read();
            let owner_id = ctx.owner_id;
            tracing::debug!(%owner_id, %message, "llOwnerSay");
            Ok(())
        })?;
        lua.globals().set("llOwnerSay", f)?;
    }

    // llInstantMessage(user, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (user, message): (String, String)| {
            let _ctx = ctx.read();
            tracing::debug!(%user, %message, "llInstantMessage");
            Ok(())
        })?;
        lua.globals().set("llInstantMessage", f)?;
    }

    // llListen(channel, name, id, msg) -> integer handle
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (channel, name, id, msg): (i64, String, String, String)| {
                let _ctx = ctx.read();
                tracing::debug!(channel, %name, %id, %msg, "llListen");
                Ok(0i64) // stub handle
            },
        )?;
        lua.globals().set("llListen", f)?;
    }

    // llListenRemove(handle)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, handle: i64| {
            let _ctx = ctx.read();
            tracing::debug!(handle, "llListenRemove");
            Ok(())
        })?;
        lua.globals().set("llListenRemove", f)?;
    }

    // llListenControl(handle, active)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (handle, active): (i64, i64)| {
            let _ctx = ctx.read();
            tracing::debug!(handle, active, "llListenControl");
            Ok(())
        })?;
        lua.globals().set("llListenControl", f)?;
    }

    // llMessageLinked(link_num, num, str, id)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (_link_num, num, str_val, id): (i32, i32, String, String)| {
                let ctx = ctx.read();
                if let Some(ref tx) = ctx.event_sender {
                    let event = crate::ScriptEvent::LinkMessage {
                        sender_num: ctx.link_num,
                        num,
                        str_val,
                        id,
                    };
                    let _ = tx.send((ctx.prim_id, event));
                }
                Ok(())
            },
        )?;
        lua.globals().set("llMessageLinked", f)?;
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
            prim_id: Uuid::nil(),
            prim_name: "Test Prim".into(),
            prim_desc: String::new(),
            owner_id: Uuid::nil(),
            position: [128.0, 128.0, 25.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            link_num: 0,
            event_sender: None,
        }))
    }

    #[test]
    fn ll_say_does_not_panic() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llSay(0, "hello world")"#).exec().unwrap();
    }

    #[test]
    fn ll_shout_does_not_panic() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llShout(0, "shout!")"#).exec().unwrap();
    }

    #[test]
    fn ll_whisper_does_not_panic() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llWhisper(0, "psst")"#).exec().unwrap();
    }

    #[test]
    fn ll_listen_returns_handle() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_communication(&lua, &ctx).unwrap();
        let handle: i64 = lua
            .load(r#"return llListen(0, "", "", "")"#)
            .eval()
            .unwrap();
        assert_eq!(handle, 0);
    }

    #[test]
    fn ll_owner_say_does_not_panic() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llOwnerSay("owner message")"#).exec().unwrap();
    }
}
