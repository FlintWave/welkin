// SPDX-License-Identifier: GPL-3.0-or-later

//! Chat module: handles public chat (whisper, say, shout), region-wide
//! broadcasts, debug-channel messages, and owner/direct chat in a region.
//!
//! Mirrors the C# `ChatModule` from OpenSim's CoreModules.  Chat messages
//! are range-filtered based on [`ChatType`] before delivery to listeners.

use uuid::Uuid;

use crate::events::ChatMessageEvent;
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Channel 0 is public chat; the debug channel uses the maximum `i32` value.
pub const DEBUG_CHANNEL: i32 = 2147483647;

/// Messages longer than this are truncated before delivery (matching C#
/// behaviour which clips at 1000 characters).
pub const MAX_MESSAGE_LENGTH: usize = 1000;

// ---------------------------------------------------------------------------
// ChatType
// ---------------------------------------------------------------------------

/// The type of a chat message, which determines its audible range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatType {
    /// 10 m range.
    Whisper,
    /// 20 m range (normal chat).
    Say,
    /// 100 m range.
    Shout,
    /// Region-wide, no distance limit.
    Region,
    /// Heard only by the object's owner.
    Owner,
    /// Point-to-point, no distance limit.
    Direct,
    /// Broadcast to every avatar in the region.
    Broadcast,
    /// Debug channel (`DEBUG_CHANNEL`), no distance limit.
    DebugChannel,
}

impl ChatType {
    /// Returns the squared audible range in metres, or `None` for types
    /// that have no distance limit (Region, Owner, Direct, Broadcast,
    /// DebugChannel).
    pub fn range_squared(&self) -> Option<f32> {
        match self {
            ChatType::Whisper => Some(10.0 * 10.0),
            ChatType::Say => Some(20.0 * 20.0),
            ChatType::Shout => Some(100.0 * 100.0),
            ChatType::Region
            | ChatType::Owner
            | ChatType::Direct
            | ChatType::Broadcast
            | ChatType::DebugChannel => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ChatSourceType
// ---------------------------------------------------------------------------

/// Identifies the origin of a chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatSourceType {
    /// Message sent by an avatar.
    Agent,
    /// Message sent by a scripted object.
    Object,
    /// System-generated message.
    System,
}

// ---------------------------------------------------------------------------
// ChatMessage
// ---------------------------------------------------------------------------

/// A single chat message flowing through the region.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub sender_id: Uuid,
    pub sender_name: String,
    pub message: String,
    pub chat_type: ChatType,
    pub source_type: ChatSourceType,
    pub position: [f32; 3],
    pub channel: i32,
}

// ---------------------------------------------------------------------------
// ChatModule
// ---------------------------------------------------------------------------

/// Non-shared region module that processes and delivers chat messages.
///
/// On receiving a message from a viewer ([`on_chat_from_viewer`]), the module
/// truncates it, adjusts the [`ChatType`] for the debug channel, and
/// broadcasts a [`ChatMessageEvent`] on the scene's event bus.  Delivery to
/// individual clients is range-filtered via [`deliver_chat_to_client`].
pub struct ChatModule {
    pub enabled: bool,
    pub whisper_distance: i32,
    pub say_distance: i32,
    pub shout_distance: i32,
    pub admin_prefix: String,
    scene: Option<SceneRef>,
}

impl ChatModule {
    pub fn new() -> Self {
        Self {
            enabled: true,
            whisper_distance: 10,
            say_distance: 20,
            shout_distance: 100,
            admin_prefix: String::new(),
            scene: None,
        }
    }

    /// Process an incoming chat message from a viewer.
    ///
    /// The message is truncated to [`MAX_MESSAGE_LENGTH`], the chat type is
    /// switched to [`ChatType::DebugChannel`] when the channel matches
    /// [`DEBUG_CHANNEL`], and a [`ChatMessageEvent`] is broadcast on the
    /// scene's event bus.
    ///
    /// Returns `true` when the event was successfully broadcast.
    pub fn on_chat_from_viewer(&self, msg: &mut ChatMessage) -> bool {
        if !self.enabled {
            return false;
        }

        // Truncate oversized messages.
        if msg.message.len() > MAX_MESSAGE_LENGTH {
            // Truncate at a char boundary.
            let end = msg
                .message
                .char_indices()
                .take_while(|&(i, _)| i < MAX_MESSAGE_LENGTH)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);
            msg.message.truncate(end);
        }

        // Debug channel overrides the chat type.
        if msg.channel == DEBUG_CHANNEL {
            msg.chat_type = ChatType::DebugChannel;
        }

        // Broadcast via the scene event bus.
        if let Some(ref scene) = self.scene {
            let evt = ChatMessageEvent {
                region_id: scene.region_id,
                sender_id: msg.sender_id,
                message: msg.message.clone(),
                channel: msg.channel,
            };
            scene.events.on_chat_message.send(evt).is_ok()
        } else {
            false
        }
    }

    /// Determine whether a chat message should be delivered to a listener at
    /// the given position.
    ///
    /// For range-limited chat types the squared distance between the sender
    /// and listener positions is compared against
    /// [`ChatType::range_squared`].  Unlimited-range types always return
    /// `true`.
    pub fn deliver_chat_to_client(&self, msg: &ChatMessage, listener_position: [f32; 3]) -> bool {
        match msg.chat_type.range_squared() {
            None => true,
            Some(max_dist_sq) => {
                let dx = msg.position[0] - listener_position[0];
                let dy = msg.position[1] - listener_position[1];
                let dz = msg.position[2] - listener_position[2];
                let dist_sq = dx * dx + dy * dy + dz * dz;
                dist_sq <= max_dist_sq
            }
        }
    }

    /// Returns `true` if the agent is currently frozen (unable to chat).
    ///
    /// Stub — always returns `false`.
    pub fn is_frozen(&self, _agent_id: Uuid) -> bool {
        false
    }
}

impl Default for ChatModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for ChatModule {
    fn name(&self) -> &str {
        "ChatModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.enabled = config.get_bool("Chat", "enabled", true);
        self.whisper_distance = config.get_int("Chat", "whisper_distance", 10) as i32;
        self.say_distance = config.get_int("Chat", "say_distance", 20) as i32;
        self.shout_distance = config.get_int("Chat", "shout_distance", 100) as i32;
        self.admin_prefix = config.get_string("Chat", "admin_prefix", "");
        tracing::info!(
            whisper = self.whisper_distance,
            say = self.say_distance,
            shout = self.shout_distance,
            "ChatModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        tracing::info!(region = %scene.region_name, "ChatModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = None;
        tracing::info!(region = %scene.region_name, "ChatModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scene = None;
        Ok(())
    }
}
