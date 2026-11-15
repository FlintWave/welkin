//! Chat and IM helpers.
//!
//! Provides functions for sending and receiving chat messages and instant
//! messages over the LLUDP protocol. Incoming packets are decoded and
//! dispatched as events through the EventBus.

use uuid::Uuid;
use welkin_msg::msgs::*;
use welkin_msg::{IncomingPacket, Msg};

use crate::error::RealmWalkerError;
use crate::events::{ChatMessage, ChatType, Event, EventBus, InstantMessage, SourceType};
use crate::network::Network;

/// Send a chat message on the given channel.
pub async fn send_chat(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    message: &str,
    chat_type: ChatType,
    channel: i32,
) -> Result<(), RealmWalkerError> {
    let msg = ChatFromViewer {
        agent_data: chat_from_viewer::AgentData {
            agent_id,
            session_id,
        },
        chat_data: chat_from_viewer::ChatData {
            message: message.as_bytes().to_vec(),
            type_: chat_type.as_u8(),
            channel,
        },
    };
    net.send_msg(&msg, true).await
}

/// Send an instant message to another avatar.
pub async fn send_im(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    to_agent_id: Uuid,
    message: &str,
) -> Result<(), RealmWalkerError> {
    let msg = ImprovedInstantMessage {
        agent_data: improved_instant_message::AgentData {
            agent_id,
            session_id,
        },
        message_block: improved_instant_message::MessageBlock {
            from_group: false,
            to_agent_id,
            parent_estate_id: 0,
            region_id: Uuid::nil(),
            position: glam::Vec3::ZERO,
            offline: 0,
            dialog: 0, // IM from agent
            id: Uuid::new_v4(),
            timestamp: 0,
            from_agent_name: Vec::new(), // server fills this
            message: message.as_bytes().to_vec(),
            binary_bucket: Vec::new(),
        },
        estate_block: improved_instant_message::EstateBlock { estate_id: 0 },
        meta_data: Vec::new(),
    };
    net.send_msg(&msg, true).await
}

/// Try to decode an incoming packet as a ChatFromSimulator and emit an event.
/// Returns true if the packet was a chat message.
pub fn try_handle_chat(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != ChatFromSimulator::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<ChatFromSimulator>() else {
        return false;
    };
    let chat = &msg.chat_data;
    let event = Event::ChatReceived(ChatMessage {
        from_name: String::from_utf8_lossy(&chat.from_name).into_owned(),
        source_id: chat.source_id,
        owner_id: chat.owner_id,
        message: String::from_utf8_lossy(&chat.message).into_owned(),
        chat_type: ChatType::from_u8(chat.chat_type),
        source_type: SourceType::from_u8(chat.source_type),
        position: chat.position,
        audible: chat.audible,
    });
    event_bus.send(event);
    true
}

/// Try to decode an incoming packet as an ImprovedInstantMessage and emit an event.
/// Returns true if the packet was an IM.
pub fn try_handle_im(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != ImprovedInstantMessage::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<ImprovedInstantMessage>() else {
        return false;
    };
    let mb = &msg.message_block;
    let event = Event::ImReceived(InstantMessage {
        from_name: String::from_utf8_lossy(&mb.from_agent_name).into_owned(),
        from_id: msg.agent_data.agent_id,
        to_id: mb.to_agent_id,
        message: String::from_utf8_lossy(&mb.message).into_owned(),
        dialog: mb.dialog,
        session_id: mb.id,
    });
    event_bus.send(event);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_msg::{build_packet, parse_packet};

    /// Build a ChatFromSimulator packet and verify the handler decodes it
    /// correctly and produces the right Event.
    #[test]
    fn handle_chat_from_simulator() {
        let source_id = Uuid::from_u128(0x1111);
        let owner_id = Uuid::from_u128(0x2222);
        let position = glam::Vec3::new(128.0, 64.0, 32.0);

        let msg = ChatFromSimulator {
            chat_data: chat_from_simulator::ChatData {
                from_name: b"TestAvatar".to_vec(),
                source_id,
                owner_id,
                source_type: 1, // Agent
                chat_type: 1,   // Normal
                audible: 0,
                position,
                message: b"Hello, world!".to_vec(),
            },
        };

        // Build a packet, then parse it back as an IncomingPacket.
        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");

        // Set up an event bus and handle the packet.
        let event_bus = EventBus::new(16);
        let mut rx = event_bus.subscribe();

        assert!(try_handle_chat(&pkt, &event_bus));

        // Verify the event.
        let event = rx.try_recv().expect("should have event");
        match event {
            Event::ChatReceived(chat) => {
                assert_eq!(chat.from_name, "TestAvatar");
                assert_eq!(chat.source_id, source_id);
                assert_eq!(chat.owner_id, owner_id);
                assert_eq!(chat.message, "Hello, world!");
                assert_eq!(chat.chat_type, ChatType::Normal);
                assert_eq!(chat.source_type, SourceType::Agent);
                assert_eq!(chat.position, position);
                assert_eq!(chat.audible, 0);
            }
            other => panic!("expected ChatReceived, got {:?}", other),
        }
    }

    /// Build an ImprovedInstantMessage packet and verify the handler decodes
    /// it and produces the right Event.
    #[test]
    fn handle_improved_instant_message() {
        let agent_id = Uuid::from_u128(0xaaaa);
        let session_id = Uuid::from_u128(0xbbbb);
        let to_id = Uuid::from_u128(0xcccc);
        let im_session_id = Uuid::from_u128(0xdddd);

        let msg = ImprovedInstantMessage {
            agent_data: improved_instant_message::AgentData {
                agent_id,
                session_id,
            },
            message_block: improved_instant_message::MessageBlock {
                from_group: false,
                to_agent_id: to_id,
                parent_estate_id: 1,
                region_id: Uuid::nil(),
                position: glam::Vec3::ZERO,
                offline: 0,
                dialog: 0,
                id: im_session_id,
                timestamp: 12345,
                from_agent_name: b"Sender Name".to_vec(),
                message: b"Private message".to_vec(),
                binary_bucket: Vec::new(),
            },
            estate_block: improved_instant_message::EstateBlock { estate_id: 1 },
            meta_data: Vec::new(),
        };

        let mut buf = Vec::new();
        build_packet(&msg, 2, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");

        let event_bus = EventBus::new(16);
        let mut rx = event_bus.subscribe();

        assert!(try_handle_im(&pkt, &event_bus));

        let event = rx.try_recv().expect("should have event");
        match event {
            Event::ImReceived(im) => {
                assert_eq!(im.from_name, "Sender Name");
                assert_eq!(im.from_id, agent_id);
                assert_eq!(im.to_id, to_id);
                assert_eq!(im.message, "Private message");
                assert_eq!(im.dialog, 0);
                assert_eq!(im.session_id, im_session_id);
            }
            other => panic!("expected ImReceived, got {:?}", other),
        }
    }

    /// A non-chat packet should be ignored by try_handle_chat.
    #[test]
    fn handle_chat_ignores_other_packets() {
        let msg = CompletePingCheck {
            ping_id: complete_ping_check::PingID { ping_id: 1 },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 3, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");

        let event_bus = EventBus::new(16);
        assert!(!try_handle_chat(&pkt, &event_bus));
    }

    /// A non-IM packet should be ignored by try_handle_im.
    #[test]
    fn handle_im_ignores_other_packets() {
        let msg = CompletePingCheck {
            ping_id: complete_ping_check::PingID { ping_id: 1 },
        };
        let mut buf = Vec::new();
        build_packet(&msg, 4, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");

        let event_bus = EventBus::new(16);
        assert!(!try_handle_im(&pkt, &event_bus));
    }

    /// Verify that non-UTF8 bytes in from_name and message are handled
    /// gracefully via lossy conversion.
    #[test]
    fn handle_chat_with_non_utf8() {
        let msg = ChatFromSimulator {
            chat_data: chat_from_simulator::ChatData {
                from_name: vec![0xFF, 0xFE, 0x41], // invalid UTF-8 followed by 'A'
                source_id: Uuid::nil(),
                owner_id: Uuid::nil(),
                source_type: 0,
                chat_type: 1,
                audible: 0,
                position: glam::Vec3::ZERO,
                message: vec![0x48, 0x69, 0xFF], // "Hi" + invalid byte
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 5, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");

        let event_bus = EventBus::new(16);
        let mut rx = event_bus.subscribe();

        assert!(try_handle_chat(&pkt, &event_bus));

        let event = rx.try_recv().expect("should have event");
        match event {
            Event::ChatReceived(chat) => {
                // The lossy conversion replaces invalid bytes with the
                // Unicode replacement character.
                assert!(chat.from_name.contains('A'));
                assert!(chat.message.starts_with("Hi"));
            }
            other => panic!("expected ChatReceived, got {:?}", other),
        }
    }

    /// Verify ChatFromViewer message builds with correct fields.
    #[test]
    fn chat_from_viewer_roundtrip() {
        let agent_id = Uuid::from_u128(0x1234);
        let session_id = Uuid::from_u128(0x5678);

        let msg = ChatFromViewer {
            agent_data: chat_from_viewer::AgentData {
                agent_id,
                session_id,
            },
            chat_data: chat_from_viewer::ChatData {
                message: b"test message".to_vec(),
                type_: ChatType::Shout.as_u8(),
                channel: 42,
            },
        };

        let mut buf = Vec::new();
        build_packet(&msg, 10, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        let decoded: ChatFromViewer = pkt.decode().expect("should decode");

        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.agent_data.session_id, session_id);
        assert_eq!(decoded.chat_data.message, b"test message");
        assert_eq!(decoded.chat_data.type_, ChatType::Shout.as_u8());
        assert_eq!(decoded.chat_data.channel, 42);
    }

    /// Verify ImprovedInstantMessage builds with correct fields.
    #[test]
    fn im_message_roundtrip() {
        let agent_id = Uuid::from_u128(0xaa);
        let session_id = Uuid::from_u128(0xbb);
        let to_id = Uuid::from_u128(0xcc);
        let im_id = Uuid::from_u128(0xdd);

        let msg = ImprovedInstantMessage {
            agent_data: improved_instant_message::AgentData {
                agent_id,
                session_id,
            },
            message_block: improved_instant_message::MessageBlock {
                from_group: false,
                to_agent_id: to_id,
                parent_estate_id: 0,
                region_id: Uuid::nil(),
                position: glam::Vec3::new(1.0, 2.0, 3.0),
                offline: 0,
                dialog: 0,
                id: im_id,
                timestamp: 99,
                from_agent_name: b"Bot".to_vec(),
                message: b"hello".to_vec(),
                binary_bucket: Vec::new(),
            },
            estate_block: improved_instant_message::EstateBlock { estate_id: 0 },
            meta_data: Vec::new(),
        };

        let mut buf = Vec::new();
        build_packet(&msg, 20, true, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("should parse");
        let decoded: ImprovedInstantMessage = pkt.decode().expect("should decode");

        assert_eq!(decoded.agent_data.agent_id, agent_id);
        assert_eq!(decoded.message_block.to_agent_id, to_id);
        assert_eq!(decoded.message_block.message, b"hello");
        assert_eq!(decoded.message_block.from_agent_name, b"Bot");
        assert_eq!(decoded.message_block.id, im_id);
        assert_eq!(decoded.message_block.timestamp, 99);
    }
}
