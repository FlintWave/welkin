// SPDX-License-Identifier: LGPL-3.0-or-later
//! Email ll* functions — llEmail and llGetNextEmail.
//!
//! Outbound emails are sent through `ScriptContext::email_sender`.
//! Inbound emails are queued in `ScriptContext::email_queue`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub time: String,
    pub address: String,
    pub subject: String,
    pub message: String,
    pub num_remaining: i32,
}

#[derive(Debug, Clone)]
pub struct EmailOutbound {
    pub from_script: uuid::Uuid,
    pub to_address: String,
    pub subject: String,
    pub body: String,
}

pub fn register_email(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llEmail(to, subject, message)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (to, subject, message): (String, String, String)| {
            let mut ctx = ctx.write();
            let now = Instant::now();
            let allowed = match &ctx.last_email_time {
                None => true,
                Some(last) => now.duration_since(*last) >= Duration::from_secs(20),
            };
            if !allowed {
                return Ok(());
            }
            if let Some(ref tx) = ctx.email_sender {
                let outbound = EmailOutbound {
                    from_script: ctx.script_id,
                    to_address: to,
                    subject,
                    body: message,
                };
                let _ = tx.send(outbound);
            }
            ctx.last_email_time = Some(now);
            Ok(())
        })?;
        lua.globals().set("llEmail", f)?;
    }

    // llGetNextEmail(sender_filter, subject_filter)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (sender_filter, subject_filter): (String, String)| {
                let mut ctx = ctx.write();
                let position = ctx.email_queue.iter().position(|email| {
                    (sender_filter.is_empty() || email.address == sender_filter)
                        && (subject_filter.is_empty() || email.subject == subject_filter)
                });
                if let Some(index) = position {
                    if let Some(email) = ctx.email_queue.remove(index) {
                        let num_remaining = ctx.email_queue.len() as i32;
                        if let Some(ref tx) = ctx.event_sender {
                            let event = crate::ScriptEvent::Email {
                                time: email.time,
                                address: email.address,
                                subject: email.subject,
                                message: email.message,
                                num_remaining,
                            };
                            let _ = tx.send((ctx.script_id, event));
                        }
                    }
                }
                Ok(())
            },
        )?;
        lua.globals().set("llGetNextEmail", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use uuid::Uuid;

    #[test]
    fn ll_email_sends_outbound() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            email_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_email(&lua, &ctx).unwrap();
        lua.load(r#"llEmail("to@example.com", "subject", "body")"#)
            .exec()
            .unwrap();
        let outbound = rx.try_recv().unwrap();
        assert_eq!(outbound.to_address, "to@example.com");
        assert_eq!(outbound.subject, "subject");
        assert_eq!(outbound.body, "body");
    }

    #[test]
    fn ll_email_rate_limit() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            email_sender: Some(tx),
            last_email_time: Some(Instant::now()),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_email(&lua, &ctx).unwrap();
        lua.load(r#"llEmail("to@example.com", "subject", "body")"#)
            .exec()
            .unwrap();
        assert!(
            rx.try_recv().is_err(),
            "should not send within 20s rate limit"
        );
    }

    #[test]
    fn ll_get_next_email_fires_event() {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let mut queue = VecDeque::new();
        queue.push_back(EmailMessage {
            time: "2026-01-01T00:00:00Z".to_string(),
            address: "sender@example.com".to_string(),
            subject: "hello".to_string(),
            message: "world".to_string(),
            num_remaining: 0,
        });
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            event_sender: Some(event_tx),
            email_queue: queue,
            ..Default::default()
        }));
        let lua = Lua::new();
        register_email(&lua, &ctx).unwrap();
        lua.load(r#"llGetNextEmail("", "")"#).exec().unwrap();
        let (_, event) = event_rx.try_recv().unwrap();
        match event {
            crate::ScriptEvent::Email {
                address,
                subject,
                message,
                num_remaining,
                ..
            } => {
                assert_eq!(address, "sender@example.com");
                assert_eq!(subject, "hello");
                assert_eq!(message, "world");
                assert_eq!(num_remaining, 0);
            }
            _ => panic!("expected Email event"),
        }
        assert!(ctx.read().email_queue.is_empty());
    }

    #[test]
    fn ll_get_next_email_filter_match() {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let mut queue = VecDeque::new();
        queue.push_back(EmailMessage {
            time: "t".into(),
            address: "a@b.com".into(),
            subject: "no".into(),
            message: "skip".into(),
            num_remaining: 0,
        });
        queue.push_back(EmailMessage {
            time: "t".into(),
            address: "target@b.com".into(),
            subject: "yes".into(),
            message: "found".into(),
            num_remaining: 0,
        });
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            event_sender: Some(event_tx),
            email_queue: queue,
            ..Default::default()
        }));
        let lua = Lua::new();
        register_email(&lua, &ctx).unwrap();
        lua.load(r#"llGetNextEmail("target@b.com", "")"#)
            .exec()
            .unwrap();
        let (_, event) = event_rx.try_recv().unwrap();
        match event {
            crate::ScriptEvent::Email { subject, .. } => assert_eq!(subject, "yes"),
            _ => panic!("expected Email event"),
        }
        assert_eq!(ctx.read().email_queue.len(), 1);
    }
}
