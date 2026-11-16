// SPDX-License-Identifier: LGPL-3.0-or-later

use std::io::IsTerminal;
use std::sync::Arc;

use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json as AxumJson, Router};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::console::{ConsoleAction, ConsoleContext};

#[derive(Clone)]
struct AdminState {
    ctx: Arc<ConsoleContext>,
    token: String,
}

pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

pub fn spawn_repl(
    ctx: Arc<ConsoleContext>,
    cancel: CancellationToken,
) -> Option<tokio::task::JoinHandle<()>> {
    if !std::io::stdin().is_terminal() {
        return None;
    }

    let handle = tokio::runtime::Handle::current();
    let ctx = ctx.clone();
    let cancel = cancel.clone();

    Some(tokio::task::spawn_blocking(move || {
        let mut editor = match rustyline::DefaultEditor::new() {
            Ok(editor) => editor,
            Err(err) => {
                eprintln!("console: {err}");
                return;
            }
        };

        loop {
            if cancel.is_cancelled() {
                break;
            }

            match editor.readline("welkin> ") {
                Ok(line) => {
                    let l = line.trim();
                    if l.is_empty() {
                        continue;
                    }
                    // Never keep password-bearing commands in the line history.
                    if !is_password_command(l) {
                        let _ = editor.add_history_entry(l);
                    }

                    if is_inline_password_form(l) {
                        eprintln!(
                            "console: refusing a password on the command line (it would be visible on screen). \
                             Type `create-user <first> <last>` and you will be prompted."
                        );
                        continue;
                    }

                    if let Some((first, last)) = needs_password_prompt(l) {
                        let password = match rpassword::prompt_password("Password: ") {
                            Ok(password) => password,
                            Err(err) => {
                                eprintln!("console: {err}");
                                continue;
                            }
                        };
                        let confirm = match rpassword::prompt_password("Confirm password: ") {
                            Ok(confirm) => confirm,
                            Err(err) => {
                                eprintln!("console: {err}");
                                continue;
                            }
                        };
                        if password != confirm {
                            eprintln!("console: passwords did not match");
                            continue;
                        }

                        if password.contains('"') {
                            eprintln!("console: passwords containing a double quote are not supported");
                            continue;
                        }
                        let command = format!(
                            "create-user \"{}\" \"{}\" \"{}\"",
                            first, last, password
                        );
                        let out = handle.block_on(crate::console::execute(&ctx, &command));
                        println!("{out}");
                        if out == "quit" {
                            break;
                        }
                        continue;
                    }

                    let out = handle.block_on(crate::console::execute(&ctx, l));
                    println!("{out}");
                    if out == "quit" {
                        break;
                    }
                }
                Err(rustyline::error::ReadlineError::Interrupted)
                | Err(rustyline::error::ReadlineError::Eof) => break,
                Err(err) => {
                    eprintln!("console: {err}");
                    break;
                }
            }
        }
    }))
}

/// True for lines that may carry a password; these never enter the line history.
/// Case-insensitive and tolerant of leading whitespace.
fn is_password_command(line: &str) -> bool {
    line.trim_start().to_ascii_lowercase().starts_with("create-user")
}

/// `create-user First Last Password [email]`: a password typed inline in the REPL.
fn is_inline_password_form(line: &str) -> bool {
    let words = line.split_whitespace().collect::<Vec<_>>();
    words.len() >= 4 && words[0].eq_ignore_ascii_case("create-user")
}

fn words(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

fn needs_password_prompt(line: &str) -> Option<(String, String)> {
    let words = words(line);
    if words.len() == 3 && words[0] == "create-user" {
        Some((words[1].to_string(), words[2].to_string()))
    } else {
        None
    }
}

async fn admin_console(
    State(state): State<AdminState>,
    headers: HeaderMap,
    body: Result<Json<serde_json::Value>, axum::extract::rejection::JsonRejection>,
) -> (StatusCode, AxumJson<serde_json::Value>) {
    let expected = format!("Bearer {}", state.token);
    let actual = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !ct_eq(actual.as_bytes(), expected.as_bytes()) {
        return (
            StatusCode::UNAUTHORIZED,
            AxumJson(json!({"error":"unauthorized"})),
        );
    }

    let Json(body) = match body {
        Ok(value) => value,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error":"bad_request"})),
            )
        }
    };

    let command = match body.get("command").and_then(|value| value.as_str()) {
        Some(command) => command,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error":"bad_request"})),
            )
        }
    };

    let output = crate::console::execute(&state.ctx, command).await;
    (StatusCode::OK, AxumJson(json!({"output": output})))
}

async fn admin_health() -> (StatusCode, AxumJson<serde_json::Value>) {
    (StatusCode::OK, AxumJson(json!({"status":"ok"})))
}

pub fn admin_router(ctx: Arc<ConsoleContext>, token: String) -> Router {
    let state = AdminState { ctx, token };
    Router::new()
        .route("/admin/console", post(admin_console))
        .route("/admin/health", get(admin_health))
        .with_state(state)
}

pub async fn run_actions(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<ConsoleAction>,
    socket: Arc<tokio::net::UdpSocket>,
    clients: Arc<
        tokio::sync::RwLock<
            std::collections::HashMap<
                std::net::SocketAddr,
                Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
            >,
        >,
    >,
    shutdown: Arc<tokio::sync::Notify>,
) {
    use welkin_msg::msgs::*;

    while let Some(action) = rx.recv().await {
        match action {
            ConsoleAction::Alert(text) => {
                tracing::info!("console action: alert");
                let chat = ChatFromSimulator {
                    chat_data: chat_from_simulator::ChatData {
                        from_name: b"SYSTEM\0".to_vec(),
                        source_id: uuid::Uuid::nil(),
                        owner_id: uuid::Uuid::nil(),
                        source_type: 1,
                        chat_type: 1,
                        audible: 1,
                        position: glam::Vec3::ZERO,
                        message: text.into_bytes(),
                    },
                };

                let snapshot: Vec<(
                    std::net::SocketAddr,
                    Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
                )> = {
                    let guard = clients.read().await;
                    guard.iter().map(|(addr, client)| (*addr, client.clone())).collect()
                };

                for (addr, client) in snapshot {
                    let pkt = {
                        let mut session = client.lock().await;
                        session.circuit.send_msg(&chat, true).0
                    };
                    let _ = socket.send_to(&pkt, addr).await;
                }
            }
            ConsoleAction::Kick { agent_id, reason } => {
                tracing::info!("console action: kick {}", agent_id);
                let snapshot: Vec<(
                    std::net::SocketAddr,
                    Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
                )> = {
                    let guard = clients.read().await;
                    guard.iter().map(|(addr, client)| (*addr, client.clone())).collect()
                };

                for (addr, client) in snapshot {
                    let packets = {
                        let mut session = client.lock().await;
                        if session.agent_id != agent_id {
                            None
                        } else {
                            let kick = KickUser {
                                target_block: kick_user::TargetBlock {
                                    target_ip: [0, 0, 0, 0],
                                    target_port: 0,
                                },
                                user_info: kick_user::UserInfo {
                                    agent_id: session.agent_id,
                                    session_id: session.session_id,
                                    reason: reason.clone().into_bytes(),
                                },
                            };
                            let disable = DisableSimulator {};
                            let kick_packet = session.circuit.send_msg(&kick, true).0;
                            let disable_packet = session.circuit.send_msg(&disable, true).0;
                            Some((kick_packet, disable_packet))
                        }
                    };

                    if let Some((kick_packet, disable_packet)) = packets {
                        let _ = socket.send_to(&kick_packet, addr).await;
                        let _ = socket.send_to(&disable_packet, addr).await;
                    }
                }
            }
            ConsoleAction::Shutdown => {
                tracing::info!("console action: shutdown");
                shutdown.notify_one();
                continue;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_works() {
        assert!(ct_eq(b"region", b"region"));
        assert!(!ct_eq(b"region", b"wrong!"));
        assert!(!ct_eq(b"short", b"slightly-longer"));
    }

    #[test]
    fn password_commands_are_detected_case_and_space_insensitively() {
        assert!(is_password_command("create-user a b pw"));
        assert!(is_password_command("  create-user a b pw"));
        assert!(is_password_command("Create-User a b pw"));
        assert!(!is_password_command("users"));
    }

    #[test]
    fn inline_password_form_is_detected() {
        assert!(is_inline_password_form("create-user Ann Lee hunter2hunter2"));
        assert!(is_inline_password_form("  Create-User Ann Lee pw a@b.c"));
        assert!(!is_inline_password_form("create-user Ann Lee"));
        assert!(!is_inline_password_form("create-user Ann"));
        assert!(!is_inline_password_form("users a b c d"));
    }

    #[test]
    fn needs_password_prompt_works() {
        assert_eq!(
            needs_password_prompt("create-user alice smith"),
            Some(("alice".to_string(), "smith".to_string()))
        );
        assert_eq!(needs_password_prompt("create-user alice"), None);
        assert_eq!(needs_password_prompt("create-user alice smith extra"), None);
        assert_eq!(needs_password_prompt("list-users alice smith"), None);
        assert_eq!(needs_password_prompt(""), None);
    }
}
