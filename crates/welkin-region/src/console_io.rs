// SPDX-License-Identifier: LGPL-3.0-or-later

use std::io::IsTerminal;
use std::sync::Arc;

use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json as AxumJson, Router};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

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
                    let _ = editor.add_history_entry(l);
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
}
