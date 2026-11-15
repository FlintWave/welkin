// SPDX-License-Identifier: GPL-3.0-or-later

//! NPCAvatar — a fake "viewer" for non-player characters.
//!
//! In C# this implements `IClientAPI` with ~300 stub methods and ~20 real
//! ones.  The Rust port exposes only the methods that [`super::NPCModule`]
//! actually calls, plus minimal getters.

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
    /// Stub: in a full implementation this would route through the
    /// `MessageTransferModule`.  Returns `true` on success.
    pub fn send_instant_message(&self, im: &GridInstantMessage) -> bool {
        tracing::debug!(
            from = %self.agent_id,
            to = %im.to_agent_id,
            dialog = ?im.dialog,
            "NPCAvatar: sending instant message"
        );
        // Stub — no real message transport.
        true
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

    /// Stub: no-op.
    pub fn send_kill_object(&self, _local_ids: &[u32]) {}
    /// Stub: no-op.
    pub fn send_avatar_data_immediate(&self) {}
    /// Stub: no-op.
    pub fn send_entity_update(&self, _entity_id: Uuid) {}
    /// Stub: no-op.
    pub fn send_alert_message(&self, _message: &str) {}
    /// Stub: no-op.
    pub fn send_agent_alert_message(&self, _message: &str, _modal: bool) {}
    /// Stub: no-op.
    pub fn send_animation(&self, _anim_id: Uuid, _sequence: i32) {}
    /// Stub: no-op.
    pub fn stop_animation(&self, _anim_id: Uuid) {}
    /// Stub: no-op.
    pub fn send_coarse_location_update(&self, _users: &[Uuid], _positions: &[[f32; 3]]) {}
}
