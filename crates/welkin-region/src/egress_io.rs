// SPDX-License-Identifier: LGPL-3.0-or-later

use std::sync::Arc;
use uuid::Uuid;

use welkin_modules::egress::{ClientEgress, EgressEvent};
use welkin_net::server::ClientSession;
use welkin_scene::scene::Scene;

pub type ClientMap = std::sync::Arc<
    tokio::sync::RwLock<
        std::collections::HashMap<
            std::net::SocketAddr,
            std::sync::Arc<tokio::sync::Mutex<welkin_net::server::ClientSession>>,
        >,
    >,
>;

pub struct RegionEgress {
    scene: Arc<Scene>,
    tx: tokio::sync::mpsc::UnboundedSender<EgressEvent>,
}

impl RegionEgress {
    pub fn new(scene: Arc<Scene>) -> (Self, tokio::sync::mpsc::UnboundedReceiver<EgressEvent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Self { scene, tx }, rx)
    }
}

impl ClientEgress for RegionEgress {
    fn send(&self, event: EgressEvent) -> bool {
        let target = event_target(&event);
        if self.scene.get_presence(&target).is_none() {
            return false;
        }
        self.tx.send(event).is_ok()
    }

    fn is_connected(&self, agent: Uuid) -> bool {
        self.scene.get_presence(&agent).is_some()
    }
}

pub fn event_target(event: &EgressEvent) -> Uuid {
    match event {
        EgressEvent::Alert { agent, .. } => *agent,
        EgressEvent::Kick { agent, .. } => *agent,
        EgressEvent::LoadUrl { agent, .. } => *agent,
        EgressEvent::TextBox { agent, .. } => *agent,
        EgressEvent::AppearanceUpdate { agent } => *agent,
        EgressEvent::RebakeRequest { agent } => *agent,
        EgressEvent::FriendOnline { agent, .. } => *agent,
        EgressEvent::Im { to, .. } => *to,
    }
}

fn cstr(s: &str) -> Vec<u8> {
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(b'\0');
    bytes
}

pub fn build_packets(
    event: &EgressEvent,
    session: &mut ClientSession,
    region_id: Uuid,
) -> Vec<Vec<u8>> {
    use welkin_msg::msgs;

    match event {
        EgressEvent::Alert {
            agent,
            message,
            modal,
        } => {
            let msg = msgs::AgentAlertMessage {
                agent_data: msgs::agent_alert_message::AgentData { agent_id: *agent },
                alert_data: msgs::agent_alert_message::AlertData {
                    modal: *modal,
                    message: cstr(message),
                },
            };
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            vec![bytes]
        }
        EgressEvent::Kick { agent, reason } => {
            let kick = msgs::KickUser {
                target_block: msgs::kick_user::TargetBlock {
                    target_ip: [0, 0, 0, 0],
                    target_port: 0,
                },
                user_info: msgs::kick_user::UserInfo {
                    agent_id: *agent,
                    session_id: session.session_id,
                    reason: reason.clone().into_bytes(),
                },
            };
            let (kick_bytes, _) = session.circuit.send_msg(&kick, true);
            let (disable_bytes, _) = session.circuit.send_msg(&msgs::DisableSimulator {}, true);
            vec![kick_bytes, disable_bytes]
        }
        EgressEvent::LoadUrl {
            agent: _,
            object_name,
            object_id,
            owner_id,
            group_owned,
            message,
            url,
        } => {
            let msg = msgs::LoadURL {
                data: msgs::load_url::Data {
                    object_name: cstr(object_name),
                    object_id: *object_id,
                    owner_id: *owner_id,
                    owner_is_group: *group_owned,
                    message: cstr(message),
                    url: cstr(url),
                },
            };
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            vec![bytes]
        }
        EgressEvent::TextBox {
            agent: _,
            message,
            chat_channel,
            object_name,
            object_id,
            owner_id,
        } => {
            let msg = msgs::ScriptDialog {
                data: msgs::script_dialog::Data {
                    object_id: *object_id,
                    first_name: cstr(""),
                    last_name: cstr(""),
                    object_name: cstr(object_name),
                    message: cstr(message),
                    chat_channel: *chat_channel,
                    image_id: Uuid::nil(),
                },
                buttons: vec![msgs::script_dialog::Buttons {
                    button_label: cstr("!!llTextBox!!"),
                }],
                owner_data: vec![msgs::script_dialog::OwnerData {
                    owner_id: *owner_id,
                }],
            };
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            vec![bytes]
        }
        EgressEvent::AppearanceUpdate { .. } => Vec::new(),
        EgressEvent::RebakeRequest { .. } => {
            let msg = msgs::RebakeAvatarTextures {
                texture_data: msgs::rebake_avatar_textures::TextureData {
                    texture_id: Uuid::nil(),
                },
            };
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            vec![bytes]
        }
        EgressEvent::FriendOnline {
            agent: _,
            friend,
            online,
        } => {
            let bytes = if *online {
                let msg = msgs::OnlineNotification {
                    agent_block: vec![msgs::online_notification::AgentBlock { agent_id: *friend }],
                };
                session.circuit.send_msg(&msg, true).0
            } else {
                let msg = msgs::OfflineNotification {
                    agent_block: vec![msgs::offline_notification::AgentBlock { agent_id: *friend }],
                };
                session.circuit.send_msg(&msg, true).0
            };
            vec![bytes]
        }
        EgressEvent::Im {
            to,
            from,
            from_name,
            dialog,
            session_id,
            message,
            offline,
        } => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs() as u32)
                .unwrap_or(0);

            let msg = msgs::ImprovedInstantMessage {
                agent_data: msgs::improved_instant_message::AgentData {
                    agent_id: *from,
                    session_id: Uuid::nil(),
                },
                message_block: msgs::improved_instant_message::MessageBlock {
                    from_group: false,
                    to_agent_id: *to,
                    parent_estate_id: 0,
                    region_id,
                    position: glam::Vec3::ZERO,
                    offline: u8::from(*offline),
                    dialog: *dialog,
                    id: *session_id,
                    timestamp,
                    from_agent_name: cstr(from_name),
                    message: cstr(message),
                    binary_bucket: Vec::new(),
                },
                estate_block: Default::default(),
                meta_data: Vec::new(),
            };
            let (bytes, _) = session.circuit.send_msg(&msg, true);
            vec![bytes]
        }
    }
}

pub async fn run(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<EgressEvent>,
    clients: ClientMap,
    socket: Arc<tokio::net::UdpSocket>,
    region_id: Uuid,
) {
    while let Some(event) = rx.recv().await {
        let target = event_target(&event);

        let snapshot: Vec<(
            std::net::SocketAddr,
            Arc<tokio::sync::Mutex<ClientSession>>,
        )> = {
            let guard = clients.read().await;
            guard.iter().map(|(addr, client)| (*addr, client.clone())).collect()
        };

        let mut found_addr = None;
        let mut found_packets: Option<Vec<Vec<u8>>> = None;

        for (addr, client) in snapshot {
            let mut session = client.lock().await;
            if session.agent_id == target {
                found_packets = Some(build_packets(&event, &mut session, region_id));
                found_addr = Some(addr);
                break;
            }
        }

        match (found_addr, found_packets) {
            (Some(addr), Some(packets)) => {
                for packet in packets {
                    if let Err(error) = socket.send_to(&packet, addr).await {
                        tracing::debug!("egress send to {} failed: {}", addr, error);
                    }
                }
            }
            _ => {
                tracing::debug!("egress target {} is not connected to this region", target);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_net::server::ClientSession;

    #[test]
    fn event_target_covers_every_variant() {
        let agent = Uuid::new_v4();
        let to = Uuid::new_v4();
        let friend = Uuid::new_v4();

        assert_eq!(
            event_target(&EgressEvent::Alert {
                agent,
                message: String::new(),
                modal: false,
            }),
            agent
        );
        assert_eq!(
            event_target(&EgressEvent::Kick {
                agent,
                reason: String::new(),
            }),
            agent
        );
        assert_eq!(
            event_target(&EgressEvent::LoadUrl {
                agent,
                object_name: String::new(),
                object_id: Uuid::nil(),
                owner_id: Uuid::nil(),
                group_owned: false,
                message: String::new(),
                url: String::new(),
            }),
            agent
        );
        assert_eq!(
            event_target(&EgressEvent::TextBox {
                agent,
                message: String::new(),
                chat_channel: 0,
                object_name: String::new(),
                object_id: Uuid::nil(),
                owner_id: Uuid::nil(),
            }),
            agent
        );
        assert_eq!(
            event_target(&EgressEvent::AppearanceUpdate { agent }),
            agent
        );
        assert_eq!(
            event_target(&EgressEvent::RebakeRequest { agent }),
            agent
        );
        assert_eq!(
            event_target(&EgressEvent::FriendOnline {
                agent,
                friend,
                online: true,
            }),
            agent
        );
        assert_eq!(
            event_target(&EgressEvent::Im {
                to,
                from: agent,
                from_name: String::new(),
                dialog: 0,
                session_id: Uuid::nil(),
                message: String::new(),
                offline: false,
            }),
            to
        );
        let _ = friend;
    }

    #[test]
    fn build_packets_alert_parses_as_agent_alert() {
        let addr = "127.0.0.1:8000".parse().unwrap();
        let agent = Uuid::new_v4();
        let mut session = ClientSession::new(addr, 7, agent, Uuid::new_v4());
        let packets = build_packets(
            &EgressEvent::Alert {
                agent,
                message: "hello".to_string(),
                modal: true,
            },
            &mut session,
            Uuid::new_v4(),
        );
        assert_eq!(packets.len(), 1);
        let parsed = welkin_msg::parse_packet(&packets[0]).unwrap();
        assert_eq!(parsed.id, <welkin_msg::msgs::AgentAlertMessage as welkin_msg::Msg>::ID);
    }

    #[test]
    fn build_packets_kick_produces_two_datagrams() {
        let addr = "127.0.0.1:8001".parse().unwrap();
        let agent = Uuid::new_v4();
        let mut session = ClientSession::new(addr, 8, agent, Uuid::new_v4());
        let packets = build_packets(
            &EgressEvent::Kick {
                agent,
                reason: "goodbye".to_string(),
            },
            &mut session,
            Uuid::new_v4(),
        );
        assert_eq!(packets.len(), 2);
    }

    #[test]
    fn build_packets_appearance_update_produces_nothing() {
        let addr = "127.0.0.1:8002".parse().unwrap();
        let agent = Uuid::new_v4();
        let mut session = ClientSession::new(addr, 9, agent, Uuid::new_v4());
        let packets = build_packets(
            &EgressEvent::AppearanceUpdate { agent },
            &mut session,
            Uuid::new_v4(),
        );
        assert!(packets.is_empty());
    }

    #[test]
    fn build_packets_other_variants_do_not_panic() {
        let addr = "127.0.0.1:8003".parse().unwrap();
        let agent = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut session = ClientSession::new(addr, 10, agent, Uuid::new_v4());
        let region = Uuid::new_v4();

        assert!(!build_packets(
            &EgressEvent::LoadUrl {
                agent,
                object_name: "object".to_string(),
                object_id: Uuid::nil(),
                owner_id: Uuid::nil(),
                group_owned: false,
                message: "go".to_string(),
                url: "https://example.com".to_string(),
            },
            &mut session,
            region,
        )
        .is_empty());

        let mut session = ClientSession::new(addr, 11, agent, Uuid::new_v4());
        assert!(!build_packets(
            &EgressEvent::TextBox {
                agent,
                message: "text".to_string(),
                chat_channel: 1,
                object_name: "box".to_string(),
                object_id: Uuid::nil(),
                owner_id: Uuid::nil(),
            },
            &mut session,
            region,
        )
        .is_empty());

        let mut session = ClientSession::new(addr, 12, agent, Uuid::new_v4());
        assert!(!build_packets(
            &EgressEvent::RebakeRequest { agent },
            &mut session,
            region,
        )
        .is_empty());

        let mut session = ClientSession::new(addr, 13, agent, Uuid::new_v4());
        assert!(!build_packets(
            &EgressEvent::FriendOnline {
                agent,
                friend: target,
                online: false,
            },
            &mut session,
            region,
        )
        .is_empty());

        let mut session = ClientSession::new(addr, 14, agent, Uuid::new_v4());
        assert!(!build_packets(
            &EgressEvent::Im {
                to: target,
                from: agent,
                from_name: "Tester".to_string(),
                dialog: 0,
                session_id: Uuid::new_v4(),
                message: "hi".to_string(),
                offline: true,
            },
            &mut session,
            region,
        )
        .is_empty());
    }
}
