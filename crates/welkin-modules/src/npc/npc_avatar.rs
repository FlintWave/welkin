// SPDX-License-Identifier: LGPL-3.0-or-later

//! NPCAvatar — a fake "viewer" for non-player characters.
//!
//! In C# this implements `IClientAPI` with ~300 stub methods and ~20 real
//! ones.  The Rust port exposes only the methods that [`super::NPCModule`]
//! actually calls, plus minimal getters.

use crate::egress::EgressEvent;
use uuid::Uuid;

use crate::chat::ChatType;
use crate::events::{ChatMessageEvent, PresenceRemovedEvent};
use crate::messaging::GridInstantMessage;
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// NPCAvatar
// ---------------------------------------------------------------------------

/// A simulated client representing a non-player character in the scene.
///
/// NPCAvatar does not connect to a real viewer; it synthesises the events
/// that the scene and other modules expect from a client presence.
#[derive(Debug, Clone)]
pub struct NPCAvatar {
    first_name: String,
    last_name: String,
    agent_id: Uuid,
    owner_id: Uuid,
    start_pos: [f32; 3],
    position: [f32; 3],
    rotation: [f32; 4],
    sense_as_agent: bool,
    scene: Option<SceneRef>,

    // Active group tracking.
    pub active_group_id: Uuid,
    pub active_group_name: String,
    pub active_group_powers: u64,

    /// Whether this avatar has been disconnected.
    disconnected: bool,

    /// Whether the NPC is currently sitting.
    sitting: bool,
    /// The object UUID the NPC is sitting on, if any.
    sit_target: Option<Uuid>,
}

impl NPCAvatar {
    /// Create a new NPC avatar.
    pub fn new(
        first_name: &str,
        last_name: &str,
        position: [f32; 3],
        uuid: Uuid,
        owner_id: Uuid,
        sense_as_agent: bool,
        scene: Option<SceneRef>,
    ) -> Self {
        Self {
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            agent_id: uuid,
            owner_id,
            start_pos: position,
            position,
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent,
            scene,
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        }
    }

    // -- Getters ------------------------------------------------------------

    /// The NPC's agent UUID.
    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    /// The NPC's first name.
    pub fn first_name(&self) -> &str {
        &self.first_name
    }

    /// The NPC's last name.
    pub fn last_name(&self) -> &str {
        &self.last_name
    }

    /// Full "First Last" display name.
    pub fn name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    /// The owning agent's UUID.
    pub fn owner_id(&self) -> Uuid {
        self.owner_id
    }

    /// The position where the NPC was spawned.
    pub fn start_pos(&self) -> [f32; 3] {
        self.start_pos
    }

    /// Current position in the region.
    pub fn position(&self) -> [f32; 3] {
        self.position
    }

    /// Current rotation quaternion.
    pub fn rotation(&self) -> [f32; 4] {
        self.rotation
    }

    /// Whether this NPC should be sensed as a real agent by LSL sensors.
    pub fn sense_as_agent(&self) -> bool {
        self.sense_as_agent
    }

    /// Whether the NPC has been disconnected.
    pub fn is_disconnected(&self) -> bool {
        self.disconnected
    }

    /// Whether the NPC is sitting.
    pub fn is_sitting(&self) -> bool {
        self.sitting
    }

    /// The scene this NPC belongs to, if any.
    pub fn scene(&self) -> Option<&SceneRef> {
        self.scene.as_ref()
    }

    // -- Setters / actions --------------------------------------------------

    /// Update the NPC's position.
    pub fn set_position(&mut self, pos: [f32; 3]) {
        self.position = pos;
    }

    /// Update the NPC's rotation.
    pub fn set_rotation(&mut self, rot: [f32; 4]) {
        self.rotation = rot;
    }

    /// Make the NPC sit on a target object.
    pub fn sit(&mut self, target: Uuid) {
        self.sitting = true;
        self.sit_target = Some(target);
    }

    /// Make the NPC stand up.
    pub fn stand(&mut self) {
        self.sitting = false;
        self.sit_target = None;
    }

    /// Deliver an instant message from this NPC.
    ///
    /// Reports the message through the scene egress as an `Im` event directed at
    /// `im.to_agent_id`. Returns `true` when the NPC is connected to a scene and
    /// the egress accepts the message; NPC and scene state is unchanged otherwise.
    pub fn send_instant_message(&self, im: &GridInstantMessage) -> bool {
        if self.disconnected {
            return false;
        }

        let Some(scene) = self.scene.as_ref() else {
            return false;
        };

        scene.egress.send(EgressEvent::Im {
            to: im.to_agent_id,
            from: self.agent_id,
            from_name: format!("{} {}", self.first_name, self.last_name),
            dialog: im.dialog as u8,
            session_id: im.im_session_id,
            message: im.message.clone(),
            offline: im.offline != 0,
        })
    }

    /// Make the NPC say a chat message, broadcasting it on the scene event bus.
    ///
    /// Returns `true` when the event was successfully broadcast.
    pub fn send_chat_message(&self, message: &str, _chat_type: ChatType, channel: i32) -> bool {
        if let Some(ref scene) = self.scene {
            let evt = ChatMessageEvent {
                region_id: scene.region_id,
                sender_id: self.agent_id,
                message: message.to_string(),
                channel,
            };
            scene.events.on_chat_message.send(evt).is_ok()
        } else {
            false
        }
    }

    /// Fire the chat-from-client event on the scene, as though a real viewer
    /// had sent a chat message.
    pub fn handle_chat_from_client(
        &self,
        message: &str,
        chat_type: ChatType,
        channel: i32,
    ) -> bool {
        self.send_chat_message(message, chat_type, channel)
    }

    /// Disconnect this NPC, firing a presence-removed event.
    pub fn disconnect(&mut self) {
        if self.disconnected {
            return;
        }
        self.disconnected = true;
        self.sitting = false;
        self.sit_target = None;

        if let Some(ref scene) = self.scene {
            let evt = PresenceRemovedEvent {
                region_id: scene.region_id,
                agent_id: self.agent_id,
            };
            let _ = scene.events.on_presence_removed.send(evt);
        }
        tracing::info!(npc = %self.agent_id, name = %self.name(), "NPCAvatar disconnected");
    }

    // -- Stubs for unused IClientAPI methods ---------------------------------

    /// Notify viewers that the supplied local object IDs are being removed.
    ///
    /// An NPC has no viewer packet stream in this crate, so the validated request
    /// is traced locally. No egress event is emitted and no NPC state changes.
    pub fn send_kill_object(&self, local_ids: &[u32]) {
        if self.disconnected || self.scene.is_none() {
            return;
        }

        tracing::debug!(
            agent = %self.agent_id,
            count = local_ids.len(),
            "NPCAvatar: sending kill object"
        );
    }
    /// Request an immediate avatar appearance/data update for this NPC.
    ///
    /// Reports the request through the scene egress as an `AppearanceUpdate` for
    /// this NPC. The method does nothing when the NPC is disconnected or has no
    /// scene.
    pub fn send_avatar_data_immediate(&self) {
        if self.disconnected {
            return;
        }

        if let Some(scene) = self.scene.as_ref() {
            let _ = scene.egress.send(EgressEvent::AppearanceUpdate {
                agent: self.agent_id,
            });
        }
    }
        /// Send an entity update for `entity_id` to the connected client.
        ///
        /// The simplified transport used by this module has no separate entity-update
        /// queue, so there is no additional local state or client effect to produce here.
        pub fn send_entity_update(&self, _entity_id: Uuid) {
            if self.disconnected {
                return;
            }

            // No client-visible egress event corresponds to a generic entity update.
        }
        /// Send a non-modal alert message to the connected client.
        pub fn send_alert_message(&self, message: &str) {
            if self.disconnected {
                return;
            }

            if let Some(scene) = &self.scene {
                let _ = scene.egress.send(crate::egress::EgressEvent::Alert {
                    agent: self.agent_id,
                    message: message.to_string(),
                    modal: false,
                });
            }
        }
        /// Send an agent alert message to the connected client.
        ///
        /// A non-modal message receives the OpenSim slash prefix unless it uses one
        /// of the recognised special message forms.
        pub fn send_agent_alert_message(&self, message: &str, modal: bool) {
            if self.disconnected {
                return;
            }

            let mut message = message.to_string();
            if !modal
                && !message.starts_with("ALERT: ")
                && !message.starts_with("NOTIFY: ")
                && message != "Home position set."
                && message != "You died and have been teleported to your home location"
            {
                message = format!("/{}", message);
            }

            if let Some(scene) = &self.scene {
                let _ = scene.egress.send(crate::egress::EgressEvent::Alert {
                    agent: self.agent_id,
                    message,
                    modal,
                });
            }
        }
    /// Stub: no-op.
    pub fn send_animation(&self, _anim_id: Uuid, _sequence: i32) {}
    /// Stub: no-op.
    pub fn stop_animation(&self, _anim_id: Uuid) {}
    /// Stub: no-op.
    pub fn send_coarse_location_update(&self, _users: &[Uuid], _positions: &[[f32; 3]]) {}
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_c_avatar_rs_npcavatar_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn send_instant_message_success_reports_event() {
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let agent = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        rec.connect(recipient);

        let npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id: agent,
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: false,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        };

        let session_id = Uuid::new_v4();
        let im = crate::messaging::GridInstantMessage {
            from_agent_id: Uuid::nil(),
            to_agent_id: recipient,
            im_session_id: session_id,
            timestamp: 0,
            from_agent_name: String::new(),
            message: "hello".to_string(),
            dialog: crate::messaging::IMDialog::MessageFromAgent,
            from_group: false,
            offline: 0,
            parent_estate_id: 0,
            position: [0.0, 0.0, 0.0],
            binary_bucket: Vec::new(),
            region_id: Uuid::nil(),
        };

        assert!(npc.send_instant_message(&im));

        let events = rec.take();
        assert_eq!(events.len(), 1);
        match &events[0] {
            crate::egress::EgressEvent::Im {
                to,
                from,
                from_name,
                dialog,
                session_id: observed_session,
                message,
                offline,
            } => {
                assert_eq!(*to, recipient);
                assert_eq!(*from, agent);
                assert_eq!(from_name, "Test Avatar");
                assert_eq!(*dialog, 0);
                assert_eq!(*observed_session, session_id);
                assert_eq!(message, "hello");
                assert!(!*offline);
            }
            _ => panic!("expected Im egress event"),
        }
    }

    #[test]
    fn send_instant_message_disconnected_returns_false() {
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: false,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: true,
            sitting: false,
            sit_target: None,
        };

        let im = crate::messaging::GridInstantMessage {
            from_agent_id: Uuid::nil(),
            to_agent_id: Uuid::new_v4(),
            im_session_id: Uuid::new_v4(),
            timestamp: 0,
            from_agent_name: String::new(),
            message: "hello".to_string(),
            dialog: crate::messaging::IMDialog::MessageFromAgent,
            from_group: false,
            offline: 0,
            parent_estate_id: 0,
            position: [0.0, 0.0, 0.0],
            binary_bucket: Vec::new(),
            region_id: Uuid::nil(),
        };

        assert!(!npc.send_instant_message(&im));
        assert!(rec.take().is_empty());
    }

    #[test]
    fn send_kill_object_does_not_emit_egress_events() {
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: false,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        };

        npc.send_kill_object(&[1, 2, 3]);
        assert!(rec.take().is_empty());
    }

    #[test]
    fn send_kill_object_disconnected_is_noop() {
        let npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: false,
            scene: None,
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: true,
            sitting: false,
            sit_target: None,
        };

        npc.send_kill_object(&[9]);
    }

    #[test]
    fn send_avatar_data_immediate_success_reports_event() {
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let agent = Uuid::new_v4();
        rec.connect(agent);

        let npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id: agent,
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: false,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        };

        npc.send_avatar_data_immediate();

        let events = rec.take();
        assert_eq!(events.len(), 1);
        match &events[0] {
            crate::egress::EgressEvent::AppearanceUpdate { agent: observed } => {
                assert_eq!(*observed, agent);
            }
            _ => panic!("expected AppearanceUpdate egress event"),
        }
    }

    #[test]
    fn send_avatar_data_immediate_disconnected_does_not_report() {
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let agent = Uuid::new_v4();
        rec.connect(agent);

        let npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id: agent,
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: false,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: true,
            sitting: false,
            sit_target: None,
        };

        npc.send_avatar_data_immediate();
        assert!(rec.take().is_empty());
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_c_avatar_rs_npcavatar_1 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn send_entity_update_emits_no_egress_events() {
        let agent_id = Uuid::new_v4();
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        scene.egress = rec.clone();
        rec.connect(agent_id);

        let mut npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id,
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: true,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        };

        npc.send_entity_update(Uuid::new_v4());
        assert!(rec.take().is_empty());
    }

    #[test]
    fn send_entity_update_without_scene_is_safe() {
        let npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: true,
            scene: None,
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        };

        npc.send_entity_update(Uuid::new_v4());
    }

    #[test]
    fn send_alert_message_emits_non_modal_alert() {
        let agent_id = Uuid::new_v4();
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        scene.egress = rec.clone();
        rec.connect(agent_id);

        let mut npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id,
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: true,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        };

        npc.send_alert_message("hello");
        let events = rec.take();
        assert_eq!(events.len(), 1);
        match &events[0] {
            crate::egress::EgressEvent::Alert { agent, message, modal } => {
                assert_eq!(*agent, agent_id);
                assert_eq!(message, "hello");
                assert!(!*modal);
            }
            _ => panic!("unexpected egress event"),
        }
    }

    #[test]
    fn send_alert_message_skips_disconnected_npc() {
        let agent_id = Uuid::new_v4();
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        scene.egress = rec.clone();
        rec.connect(agent_id);

        let mut npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id,
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: true,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: true,
            sitting: false,
            sit_target: None,
        };

        npc.send_alert_message("hello");
        assert!(rec.take().is_empty());
    }

    #[test]
    fn send_agent_alert_message_prefixes_non_modal_message() {
        let agent_id = Uuid::new_v4();
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        scene.egress = rec.clone();
        rec.connect(agent_id);

        let mut npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id,
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: true,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        };

        npc.send_agent_alert_message("hello", false);
        let events = rec.take();
        assert_eq!(events.len(), 1);
        match &events[0] {
            crate::egress::EgressEvent::Alert { agent, message, modal } => {
                assert_eq!(*agent, agent_id);
                assert_eq!(message, "/hello");
                assert!(!*modal);
            }
            _ => panic!("unexpected egress event"),
        }
    }

    #[test]
    fn send_agent_alert_message_respects_modal_and_special_forms() {
        let agent_id = Uuid::new_v4();
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        scene.egress = rec.clone();
        rec.connect(agent_id);

        let mut npc = NPCAvatar {
            first_name: "Test".to_string(),
            last_name: "Avatar".to_string(),
            agent_id,
            owner_id: Uuid::new_v4(),
            start_pos: [0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            sense_as_agent: true,
            scene: Some(scene),
            active_group_id: Uuid::nil(),
            active_group_name: String::new(),
            active_group_powers: 0,
            disconnected: false,
            sitting: false,
            sit_target: None,
        };

        npc.send_agent_alert_message("hello", true);
        npc.send_agent_alert_message("ALERT: hello", false);

        let events = rec.take();
        assert_eq!(events.len(), 2);
        match &events[0] {
            crate::egress::EgressEvent::Alert { message, modal, .. } => {
                assert_eq!(message, "hello");
                assert!(*modal);
            }
            _ => panic!("unexpected egress event"),
        }
        match &events[1] {
            crate::egress::EgressEvent::Alert { message, modal, .. } => {
                assert_eq!(message, "ALERT: hello");
                assert!(!*modal);
            }
            _ => panic!("unexpected egress event"),
        }
    }
}
