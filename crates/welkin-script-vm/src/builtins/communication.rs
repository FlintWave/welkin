// SPDX-License-Identifier: GPL-3.0-or-later
//! Communication ll* functions — llSay, llShout, llWhisper, llListen, etc.
//!
//! Sends structured [`ChatMessage`]s through `ScriptContext::chat_sender`.
//! Listen filters are tracked in `ScriptContext::listen_handles`.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::{ListenFilter, ScriptContext};
use crate::{ChatMessage, ChatType};

fn send_chat(ctx: &ScriptContext, channel: i32, message: String, chat_type: ChatType) {
    if let Some(ref tx) = ctx.chat_sender {
        let msg = ChatMessage {
            source_id: ctx.prim_id,
            source_name: ctx.prim_name.clone(),
            source_pos: ctx.position,
            owner_id: ctx.owner_id,
            channel,
            message,
            chat_type,
            target_id: None,
        };
        let _ = tx.send(msg);
    }
}

/// Register all communication ll* functions as Lua globals.
pub fn register_communication(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llSay(channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (channel, message): (i64, String)| {
            let ctx = ctx.read();
            send_chat(&ctx, channel as i32, message, ChatType::Say);
            Ok(())
        })?;
        lua.globals().set("llSay", f)?;
    }

    // llShout(channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (channel, message): (i64, String)| {
            let ctx = ctx.read();
            send_chat(&ctx, channel as i32, message, ChatType::Shout);
            Ok(())
        })?;
        lua.globals().set("llShout", f)?;
    }

    // llWhisper(channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (channel, message): (i64, String)| {
            let ctx = ctx.read();
            send_chat(&ctx, channel as i32, message, ChatType::Whisper);
            Ok(())
        })?;
        lua.globals().set("llWhisper", f)?;
    }

    // llRegionSay(channel, message) — channel 0 is blocked per LSL spec
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (channel, message): (i64, String)| {
            if channel == 0 {
                return Ok(());
            }
            let ctx = ctx.read();
            send_chat(&ctx, channel as i32, message, ChatType::Region);
            Ok(())
        })?;
        lua.globals().set("llRegionSay", f)?;
    }

    // llRegionSayTo(target, channel, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (target, channel, message): (String, i64, String)| {
                let ctx = ctx.read();
                if let Some(ref tx) = ctx.chat_sender {
                    let target_uuid =
                        uuid::Uuid::parse_str(&target).unwrap_or(uuid::Uuid::nil());
                    let msg = ChatMessage {
                        source_id: ctx.prim_id,
                        source_name: ctx.prim_name.clone(),
                        source_pos: ctx.position,
                        owner_id: ctx.owner_id,
                        channel: channel as i32,
                        message,
                        chat_type: ChatType::Direct,
                        target_id: Some(target_uuid),
                    };
                    let _ = tx.send(msg);
                }
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
            send_chat(&ctx, 0, message, ChatType::Owner);
            Ok(())
        })?;
        lua.globals().set("llOwnerSay", f)?;
    }

    // llInstantMessage(user, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (user, message): (String, String)| {
            let ctx = ctx.read();
            if let Some(ref tx) = ctx.chat_sender {
                let target_uuid = uuid::Uuid::parse_str(&user).unwrap_or(uuid::Uuid::nil());
                let msg = ChatMessage {
                    source_id: ctx.prim_id,
                    source_name: ctx.prim_name.clone(),
                    source_pos: ctx.position,
                    owner_id: ctx.owner_id,
                    channel: 0,
                    message,
                    chat_type: ChatType::IM,
                    target_id: Some(target_uuid),
                };
                let _ = tx.send(msg);
            }
            Ok(())
        })?;
        lua.globals().set("llInstantMessage", f)?;
    }

    // llListen(channel, name, id, msg) -> integer handle
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (channel, name, id, msg): (i64, String, String, String)| {
                let mut ctx = ctx.write();
                let handle = ctx.next_listen_handle;
                ctx.next_listen_handle += 1;
                ctx.listen_handles.push(ListenFilter {
                    handle,
                    channel: channel as i32,
                    name,
                    id,
                    msg,
                    active: true,
                });
                Ok(handle as i64)
            },
        )?;
        lua.globals().set("llListen", f)?;
    }

    // llListenRemove(handle)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, handle: i64| {
            let mut ctx = ctx.write();
            ctx.listen_handles.retain(|f| f.handle != handle as i32);
            Ok(())
        })?;
        lua.globals().set("llListenRemove", f)?;
    }

    // llListenControl(handle, active)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (handle, active): (i64, i64)| {
            let mut ctx = ctx.write();
            if let Some(filter) = ctx
                .listen_handles
                .iter_mut()
                .find(|f| f.handle == handle as i32)
            {
                filter.active = active != 0;
            }
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
            prim_id: Uuid::from_u128(0xabcd),
            prim_name: "Test Prim".into(),
            owner_id: Uuid::from_u128(0x1234),
            position: [128.0, 128.0, 25.0],
            ..Default::default()
        }))
    }

    fn test_ctx_with_chat() -> (Arc<RwLock<ScriptContext>>, std::sync::mpsc::Receiver<ChatMessage>)
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::nil(),
            prim_id: Uuid::from_u128(0xabcd),
            prim_name: "Test Prim".into(),
            owner_id: Uuid::from_u128(0x1234),
            position: [128.0, 128.0, 25.0],
            chat_sender: Some(tx),
            ..Default::default()
        }));
        (ctx, rx)
    }

    #[test]
    fn ll_say_sends_chat_message() {
        let (ctx, rx) = test_ctx_with_chat();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llSay(0, "hello world")"#).exec().unwrap();
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.chat_type, ChatType::Say);
        assert_eq!(msg.channel, 0);
        assert_eq!(msg.message, "hello world");
        assert_eq!(msg.source_id, Uuid::from_u128(0xabcd));
    }

    #[test]
    fn ll_shout_sends_chat_message() {
        let (ctx, rx) = test_ctx_with_chat();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llShout(42, "shout!")"#).exec().unwrap();
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.chat_type, ChatType::Shout);
        assert_eq!(msg.channel, 42);
        assert_eq!(msg.message, "shout!");
    }

    #[test]
    fn ll_whisper_sends_chat_message() {
        let (ctx, rx) = test_ctx_with_chat();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llWhisper(0, "psst")"#).exec().unwrap();
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.chat_type, ChatType::Whisper);
        assert_eq!(msg.message, "psst");
    }

    #[test]
    fn ll_region_say_blocks_channel_zero() {
        let (ctx, rx) = test_ctx_with_chat();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llRegionSay(0, "should not send")"#)
            .exec()
            .unwrap();
        assert!(
            rx.try_recv().is_err(),
            "llRegionSay on channel 0 must not send"
        );
    }

    #[test]
    fn ll_region_say_sends_on_nonzero_channel() {
        let (ctx, rx) = test_ctx_with_chat();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llRegionSay(42, "region msg")"#)
            .exec()
            .unwrap();
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.chat_type, ChatType::Region);
        assert_eq!(msg.channel, 42);
    }

    #[test]
    fn ll_owner_say_sends_owner_type() {
        let (ctx, rx) = test_ctx_with_chat();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llOwnerSay("owner message")"#).exec().unwrap();
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.chat_type, ChatType::Owner);
        assert_eq!(msg.channel, 0);
    }

    #[test]
    fn ll_say_no_sender_does_not_panic() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        lua.load(r#"llSay(0, "hello world")"#).exec().unwrap();
    }

    #[test]
    fn ll_listen_returns_incrementing_handles() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        let h1: i64 = lua
            .load(r#"return llListen(0, "", "", "")"#)
            .eval()
            .unwrap();
        let h2: i64 = lua
            .load(r#"return llListen(1, "", "", "")"#)
            .eval()
            .unwrap();
        assert_eq!(h1, 1);
        assert_eq!(h2, 2);
        assert_eq!(ctx.read().listen_handles.len(), 2);
    }

    #[test]
    fn ll_listen_remove_removes_filter() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        let h: i64 = lua
            .load(r#"return llListen(0, "", "", "")"#)
            .eval()
            .unwrap();
        assert_eq!(ctx.read().listen_handles.len(), 1);
        lua.load(&format!("llListenRemove({h})")).exec().unwrap();
        assert_eq!(ctx.read().listen_handles.len(), 0);
    }

    #[test]
    fn ll_listen_control_toggles_active() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_communication(&lua, &ctx).unwrap();
        let h: i64 = lua
            .load(r#"return llListen(0, "", "", "")"#)
            .eval()
            .unwrap();
        assert!(ctx.read().listen_handles[0].active);
        lua.load(&format!("llListenControl({h}, 0)"))
            .exec()
            .unwrap();
        assert!(!ctx.read().listen_handles[0].active);
        lua.load(&format!("llListenControl({h}, 1)"))
            .exec()
            .unwrap();
        assert!(ctx.read().listen_handles[0].active);
    }
}
