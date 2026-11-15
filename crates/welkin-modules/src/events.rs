// SPDX-License-Identifier: LGPL-3.0-or-later

//! Typed event bus for scene-level notifications.
//!
//! Uses `tokio::sync::broadcast` channels so that multiple modules can
//! subscribe to the same event independently.

use tokio::sync::broadcast;
use uuid::Uuid;

/// Default channel capacity for event buses.
const DEFAULT_CAPACITY: usize = 64;

// ---------------------------------------------------------------------------
// Event payload types
// ---------------------------------------------------------------------------

/// An object (scene-object group) was added to the scene.
#[derive(Debug, Clone)]
pub struct ObjectAddedEvent {
    pub region_id: Uuid,
    pub object_id: Uuid,
    pub owner_id: Uuid,
}

/// An object was removed from the scene.
#[derive(Debug, Clone)]
pub struct ObjectRemovedEvent {
    pub region_id: Uuid,
    pub object_id: Uuid,
}

/// A presence (avatar) was added to the scene.
#[derive(Debug, Clone)]
pub struct PresenceAddedEvent {
    pub region_id: Uuid,
    pub agent_id: Uuid,
    pub first_name: String,
    pub last_name: String,
}

/// A presence was removed from the scene.
#[derive(Debug, Clone)]
pub struct PresenceRemovedEvent {
    pub region_id: Uuid,
    pub agent_id: Uuid,
}

/// A chat message was sent in the region.
#[derive(Debug, Clone)]
pub struct ChatMessageEvent {
    pub region_id: Uuid,
    pub sender_id: Uuid,
    pub message: String,
    pub channel: i32,
}

/// A land parcel changed (ownership, flags, etc.).
#[derive(Debug, Clone)]
pub struct LandChangeEvent {
    pub region_id: Uuid,
    pub parcel_id: Uuid,
}

/// A permission check for an agent on an object.
#[derive(Debug, Clone)]
pub struct PermissionCheckEvent {
    pub region_id: Uuid,
    pub agent_id: Uuid,
    pub object_id: Uuid,
    pub permissions: u32,
}

/// Money was received by an object (triggers LSL money() event).
#[derive(Debug, Clone)]
pub struct MoneyReceivedEvent {
    pub region_id: Uuid,
    pub object_id: Uuid,
    pub agent_id: Uuid,
    pub amount: i32,
}

// ---------------------------------------------------------------------------
// SceneEvents — the event bus
// ---------------------------------------------------------------------------

/// Collection of broadcast channels that modules subscribe to for
/// scene-level notifications.
pub struct SceneEvents {
    pub on_object_added: broadcast::Sender<ObjectAddedEvent>,
    pub on_object_removed: broadcast::Sender<ObjectRemovedEvent>,
    pub on_presence_added: broadcast::Sender<PresenceAddedEvent>,
    pub on_presence_removed: broadcast::Sender<PresenceRemovedEvent>,
    pub on_chat_message: broadcast::Sender<ChatMessageEvent>,
    pub on_land_change: broadcast::Sender<LandChangeEvent>,
    pub on_permission_check: broadcast::Sender<PermissionCheckEvent>,
    pub on_money_received: broadcast::Sender<MoneyReceivedEvent>,
}

impl SceneEvents {
    /// Create a new event bus with default channel capacities.
    pub fn new() -> Self {
        Self {
            on_object_added: broadcast::channel(DEFAULT_CAPACITY).0,
            on_object_removed: broadcast::channel(DEFAULT_CAPACITY).0,
            on_presence_added: broadcast::channel(DEFAULT_CAPACITY).0,
            on_presence_removed: broadcast::channel(DEFAULT_CAPACITY).0,
            on_chat_message: broadcast::channel(DEFAULT_CAPACITY).0,
            on_land_change: broadcast::channel(DEFAULT_CAPACITY).0,
            on_permission_check: broadcast::channel(DEFAULT_CAPACITY).0,
            on_money_received: broadcast::channel(DEFAULT_CAPACITY).0,
        }
    }
}

impl Default for SceneEvents {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SceneEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneEvents")
            .field(
                "on_object_added_receivers",
                &self.on_object_added.receiver_count(),
            )
            .field(
                "on_presence_added_receivers",
                &self.on_presence_added.receiver_count(),
            )
            .finish_non_exhaustive()
    }
}
