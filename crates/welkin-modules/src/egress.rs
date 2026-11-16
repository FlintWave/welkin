// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashSet;

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum EgressEvent {
    Alert { agent: Uuid, message: String, modal: bool },
    Kick { agent: Uuid, reason: String },
    LoadUrl { agent: Uuid, object_name: String, object_id: Uuid, owner_id: Uuid, group_owned: bool, message: String, url: String },
    TextBox { agent: Uuid, message: String, chat_channel: i32, object_name: String, object_id: Uuid, owner_id: Uuid },
    AppearanceUpdate { agent: Uuid },
    RebakeRequest { agent: Uuid },
    FriendOnline { agent: Uuid, friend: Uuid, online: bool },
    Im { to: Uuid, from: Uuid, from_name: String, dialog: u8, session_id: Uuid, message: String, offline: bool },
}

pub trait ClientEgress: Send + Sync {
    /// Queue an event for delivery. Returns true if the agent is connected to this region and the event was accepted.
    fn send(&self, event: EgressEvent) -> bool;
    /// True when `agent` currently has a viewer connected to this region.
    fn is_connected(&self, agent: Uuid) -> bool;
}

#[derive(Debug, Default, Clone)]
pub struct NullEgress;

impl ClientEgress for NullEgress {
    fn send(&self, event: EgressEvent) -> bool {
        tracing::debug!("NullEgress dropped event: {:?}", event);
        false
    }

    fn is_connected(&self, _agent: Uuid) -> bool {
        false
    }
}

pub struct RecordingEgress {
    connected: parking_lot::Mutex<HashSet<Uuid>>,
    events: parking_lot::Mutex<Vec<EgressEvent>>,
}

impl RecordingEgress {
    pub fn new() -> Self {
        Self {
            connected: parking_lot::Mutex::new(HashSet::new()),
            events: parking_lot::Mutex::new(Vec::new()),
        }
    }

    pub fn connect(&self, agent: Uuid) {
        self.connected.lock().insert(agent);
    }

    pub fn disconnect(&self, agent: Uuid) {
        self.connected.lock().remove(&agent);
    }

    pub fn events(&self) -> Vec<EgressEvent> {
        self.events.lock().clone()
    }

    pub fn take(&self) -> Vec<EgressEvent> {
        self.events.lock().drain(..).collect()
    }
}

impl Default for RecordingEgress {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientEgress for RecordingEgress {
    fn send(&self, event: EgressEvent) -> bool {
        let target = match &event {
            EgressEvent::Alert { agent, .. }
            | EgressEvent::Kick { agent, .. }
            | EgressEvent::LoadUrl { agent, .. }
            | EgressEvent::TextBox { agent, .. }
            | EgressEvent::AppearanceUpdate { agent }
            | EgressEvent::RebakeRequest { agent }
            | EgressEvent::FriendOnline { agent, .. } => *agent,
            EgressEvent::Im { to, .. } => *to,
        };

        if self.is_connected(target) {
            self.events.lock().push(event);
            true
        } else {
            false
        }
    }

    fn is_connected(&self, agent: Uuid) -> bool {
        self.connected.lock().contains(&agent)
    }
}

pub type SharedEgress = std::sync::Arc<dyn ClientEgress>;

pub fn null_egress() -> SharedEgress {
    std::sync::Arc::new(NullEgress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_records_and_returns_true_for_connected_agent() {
        let egress = RecordingEgress::new();
        let agent = Uuid::new_v4();
        egress.connect(agent);

        let event = EgressEvent::Alert {
            agent,
            message: "hello".to_string(),
            modal: true,
        };
        assert!(egress.send(event.clone()));
        assert_eq!(egress.events(), vec![event]);
    }

    #[test]
    fn recording_returns_false_and_records_nothing_for_disconnected_agent() {
        let egress = RecordingEgress::new();
        let agent = Uuid::new_v4();

        let event = EgressEvent::Kick {
            agent,
            reason: "gone".to_string(),
        };
        assert!(!egress.send(event.clone()));
        assert!(egress.events().is_empty());
    }

    #[test]
    fn disconnect_stops_delivery() {
        let egress = RecordingEgress::new();
        let agent = Uuid::new_v4();
        egress.connect(agent);

        let first = EgressEvent::RebakeRequest { agent };
        assert!(egress.send(first.clone()));
        assert_eq!(egress.events(), vec![first.clone()]);

        egress.disconnect(agent);

        let second = EgressEvent::AppearanceUpdate { agent };
        assert!(!egress.send(second));
        assert_eq!(egress.events(), vec![first]);
    }

    #[test]
    fn take_drains_events() {
        let egress = RecordingEgress::new();
        let agent = Uuid::new_v4();
        egress.connect(agent);

        let event = EgressEvent::TextBox {
            agent,
            message: "box".to_string(),
            chat_channel: 7,
            object_name: "object".to_string(),
            object_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
        };
        assert!(egress.send(event.clone()));

        assert_eq!(egress.take(), vec![event]);
        assert!(egress.events().is_empty());
    }

    #[test]
    fn im_routes_by_to() {
        let egress = RecordingEgress::new();
        let to = Uuid::new_v4();
        let from = Uuid::new_v4();
        egress.connect(to);

        let event = EgressEvent::Im {
            to,
            from,
            from_name: "sender".to_string(),
            dialog: 0,
            session_id: Uuid::new_v4(),
            message: "message".to_string(),
            offline: false,
        };
        assert!(egress.send(event.clone()));
        assert_eq!(egress.events(), vec![event]);
    }

    #[test]
    fn null_egress_send_is_false_and_not_connected() {
        let egress = null_egress();
        let agent = Uuid::new_v4();

        let event = EgressEvent::FriendOnline {
            agent,
            friend: Uuid::new_v4(),
            online: true,
        };
        assert!(!egress.send(event));
        assert!(!egress.is_connected(agent));
    }
}
