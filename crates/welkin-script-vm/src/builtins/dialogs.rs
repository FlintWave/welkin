// SPDX-License-Identifier: LGPL-3.0-or-later

use std::sync::Arc;

use mlua::Lua;
use parking_lot::RwLock;
use uuid::Uuid;

use super::ScriptContext;

#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    Dialog {
        agent_id: Uuid,
        object_name: String,
        object_id: Uuid,
        owner_id: Uuid,
        message: String,
        buttons: Vec<String>,
        channel: i32,
    },
    LoadUrl {
        agent_id: Uuid,
        object_name: String,
        object_id: Uuid,
        owner_id: Uuid,
        message: String,
        url: String,
    },
    TextBox {
        agent_id: Uuid,
        object_name: String,
        object_id: Uuid,
        owner_id: Uuid,
        message: String,
        channel: i32,
    },
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn parse_agent(avatar: Option<String>) -> Option<Uuid> {
    let avatar = avatar?;
    let agent_id = Uuid::parse_str(&avatar).ok()?;
    if agent_id.is_nil() {
        None
    } else {
        Some(agent_id)
    }
}

pub fn register_dialogs(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), crate::VmError> {
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llDialog",
            lua.create_function(
                move |_lua, (avatar, message, buttons, channel): (
                    Option<String>,
                    String,
                    mlua::Table,
                    i32,
                )| {
                    let Some(agent_id) = parse_agent(avatar) else {
                        return Ok(());
                    };
                    if message.is_empty() {
                        return Ok(());
                    }

                    let mut button_labels: Vec<String> = buttons
                        .sequence_values::<String>()
                        .filter_map(Result::ok)
                        .take(12)
                        .map(|label| {
                            let label = truncate_chars(&label, 24);
                            if label.is_empty() {
                                " ".to_string()
                            } else {
                                label
                            }
                        })
                        .collect();

                    if button_labels.is_empty() {
                        button_labels.push("OK".to_string());
                    }

                    let message = truncate_chars(&message, 511);

                    let ctx = ctx.read();
                    if let Some(sender) = &ctx.ui_sender {
                        let _ = sender.send(UiCommand::Dialog {
                            agent_id,
                            object_name: ctx.prim_name.clone(),
                            object_id: ctx.prim_id,
                            owner_id: ctx.owner_id,
                            message,
                            buttons: button_labels,
                            channel,
                        });
                    }
                    Ok(())
                },
            )?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llLoadURL",
            lua.create_function(
                move |_lua, (avatar, message, url): (Option<String>, String, String)| {
                    let Some(agent_id) = parse_agent(avatar) else {
                        return Ok(());
                    };
                    if url.is_empty() {
                        return Ok(());
                    }

                    let message = truncate_chars(&message, 255);
                    let url = truncate_chars(&url, 1024);

                    let ctx = ctx.read();
                    if let Some(sender) = &ctx.ui_sender {
                        let _ = sender.send(UiCommand::LoadUrl {
                            agent_id,
                            object_name: ctx.prim_name.clone(),
                            object_id: ctx.prim_id,
                            owner_id: ctx.owner_id,
                            message,
                            url,
                        });
                    }
                    Ok(())
                },
            )?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llTextBox",
            lua.create_function(
                move |_lua, (avatar, message, channel): (Option<String>, String, i32)| {
                    let Some(agent_id) = parse_agent(avatar) else {
                        return Ok(());
                    };
                    if message.is_empty() {
                        return Ok(());
                    }

                    let message = truncate_chars(&message, 511);

                    let ctx = ctx.read();
                    if let Some(sender) = &ctx.ui_sender {
                        let _ = sender.send(UiCommand::TextBox {
                            agent_id,
                            object_name: ctx.prim_name.clone(),
                            object_id: ctx.prim_id,
                            owner_id: ctx.owner_id,
                            message,
                            channel,
                        });
                    }
                    Ok(())
                },
            )?,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc;

    use mlua::Lua;
    use uuid::Uuid;

    const AVATAR: &str = "11111111-2222-3333-4444-555555555555";
    const NIL_AVATAR: &str = "00000000-0000-0000-0000-000000000000";

    fn make_ctx(tx: mpsc::Sender<UiCommand>) -> (Arc<RwLock<ScriptContext>>, Uuid, Uuid, String) {
        let prim_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let prim_name = "TestPrim".to_string();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            ui_sender: Some(tx),
            prim_id,
            prim_name: prim_name.clone(),
            owner_id,
            ..Default::default()
        }));
        (ctx, prim_id, owner_id, prim_name)
    }

    #[test]
    fn truncate_chars_truncates_by_characters() {
        assert_eq!(truncate_chars("abcdef", 3), "abc");
        assert_eq!(truncate_chars("a😀b", 2), "a😀");
        assert_eq!(truncate_chars("short", 10), "short");
    }

    #[test]
    fn dialog_sends_buttons_and_channel() {
        let (tx, rx) = mpsc::channel();
        let (ctx, prim_id, owner_id, prim_name) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(
            r#"llDialog("{}", "hello", {{"Yes", "No"}}, 42)"#,
            AVATAR
        ))
        .exec()
        .unwrap();

        match rx.try_recv().unwrap() {
            UiCommand::Dialog {
                agent_id,
                object_name,
                object_id,
                owner_id: received_owner,
                message,
                buttons,
                channel,
            } => {
                assert_eq!(agent_id.to_string(), AVATAR);
                assert_eq!(object_name, prim_name);
                assert_eq!(object_id, prim_id);
                assert_eq!(received_owner, owner_id);
                assert_eq!(message, "hello");
                assert_eq!(buttons, vec!["Yes".to_string(), "No".to_string()]);
                assert_eq!(channel, 42);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn dialog_empty_button_becomes_space() {
        let (tx, rx) = mpsc::channel();
        let (ctx, _, _, _) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(
            r#"llDialog("{}", "m", {{"", "X"}}, 1)"#,
            AVATAR
        ))
        .exec()
        .unwrap();

        match rx.try_recv().unwrap() {
            UiCommand::Dialog { buttons, .. } => {
                assert_eq!(buttons, vec![" ".to_string(), "X".to_string()]);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn dialog_limits_buttons_to_twelve() {
        let (tx, rx) = mpsc::channel();
        let (ctx, _, _, _) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(
            r#"
            local buttons = {{}}
            for i = 1, 15 do
                buttons[i] = tostring(i)
            end
            llDialog("{}", "m", buttons, 1)
            "#,
            AVATAR
        ))
        .exec()
        .unwrap();

        match rx.try_recv().unwrap() {
            UiCommand::Dialog { buttons, .. } => {
                assert_eq!(buttons.len(), 12);
                let expected: Vec<String> = (1..=12).map(|i| i.to_string()).collect();
                assert_eq!(buttons, expected);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn dialog_no_buttons_becomes_ok() {
        let (tx, rx) = mpsc::channel();
        let (ctx, _, _, _) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(r#"llDialog("{}", "m", {{}}, 1)"#, AVATAR))
            .exec()
            .unwrap();

        match rx.try_recv().unwrap() {
            UiCommand::Dialog { buttons, .. } => {
                assert_eq!(buttons, vec!["OK".to_string()]);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn dialog_truncates_long_message() {
        let (tx, rx) = mpsc::channel();
        let (ctx, _, _, _) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(
            r#"llDialog("{}", string.rep("a", 600), {{}}, 1)"#,
            AVATAR
        ))
        .exec()
        .unwrap();

        match rx.try_recv().unwrap() {
            UiCommand::Dialog { message, .. } => {
                assert_eq!(message.chars().count(), 511);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn dialog_nil_and_invalid_avatar_send_nothing() {
        let (tx, rx) = mpsc::channel();
        let (ctx, _, _, _) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(r#"llDialog(nil, "m", {}, 1)"#)
            .exec()
            .unwrap();
        assert!(rx.try_recv().is_err());

        lua.load(format!(r#"llDialog("{}", "m", {{}}, 1)"#, NIL_AVATAR))
            .exec()
            .unwrap();
        assert!(rx.try_recv().is_err());

        lua.load(r#"llDialog("not-a-uuid", "m", {}, 1)"#)
            .exec()
            .unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn dialog_empty_message_sends_nothing() {
        let (tx, rx) = mpsc::channel();
        let (ctx, _, _, _) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(r#"llDialog("{}", "", {{}}, 1)"#, AVATAR))
            .exec()
            .unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn load_url_sends_url() {
        let (tx, rx) = mpsc::channel();
        let (ctx, prim_id, owner_id, prim_name) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(
            r#"llLoadURL("{}", "Please visit", "https://example.com")"#,
            AVATAR
        ))
        .exec()
        .unwrap();

        match rx.try_recv().unwrap() {
            UiCommand::LoadUrl {
                agent_id,
                object_name,
                object_id,
                owner_id: received_owner,
                message,
                url,
            } => {
                assert_eq!(agent_id.to_string(), AVATAR);
                assert_eq!(object_name, prim_name);
                assert_eq!(object_id, prim_id);
                assert_eq!(received_owner, owner_id);
                assert_eq!(message, "Please visit");
                assert_eq!(url, "https://example.com");
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn load_url_empty_url_sends_nothing() {
        let (tx, rx) = mpsc::channel();
        let (ctx, _, _, _) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(
            r#"llLoadURL("{}", "message", "")"#,
            AVATAR
        ))
        .exec()
        .unwrap();
        assert!(rx.try_recv().is_err());

        lua.load(r#"llLoadURL("not-a-uuid", "message", "https://example.com")"#)
        .exec()
        .unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn text_box_sends_channel() {
        let (tx, rx) = mpsc::channel();
        let (ctx, prim_id, owner_id, prim_name) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(
            r#"llTextBox("{}", "Enter value", 7)"#,
            AVATAR
        ))
        .exec()
        .unwrap();

        match rx.try_recv().unwrap() {
            UiCommand::TextBox {
                agent_id,
                object_name,
                object_id,
                owner_id: received_owner,
                message,
                channel,
            } => {
                assert_eq!(agent_id.to_string(), AVATAR);
                assert_eq!(object_name, prim_name);
                assert_eq!(object_id, prim_id);
                assert_eq!(received_owner, owner_id);
                assert_eq!(message, "Enter value");
                assert_eq!(channel, 7);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn text_box_empty_message_sends_nothing() {
        let (tx, rx) = mpsc::channel();
        let (ctx, _, _, _) = make_ctx(tx);
        let lua = Lua::new();
        register_dialogs(&lua, &ctx).unwrap();

        lua.load(format!(r#"llTextBox("{}", "", 1)"#, AVATAR))
            .exec()
            .unwrap();
        assert!(rx.try_recv().is_err());
    }
}
