// SPDX-License-Identifier: LGPL-3.0-or-later
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::DisconnectReason;

/// An event produced by the RealmWalker client.
#[derive(Debug, Clone)]
pub enum Event {
    /// A nearby chat message was received.
    ChatReceived(ChatMessage),
    /// An instant message was received.
    ImReceived(InstantMessage),
    /// The connection was lost or closed.
    Disconnected(DisconnectReason),
    /// The avatar has been moved to a different simulator.
    SimChanged(RegionInfo),
    /// A teleport offer (lure) was received from another avatar.
    TeleportOffer(TeleportOfferData),
    /// The server has begun processing a teleport request.
    TeleportStarted(TeleportStartData),
    /// Server progress message during an in-flight teleport.
    TeleportProgress(TeleportProgressData),
    /// A teleport failed. The reason string describes why.
    TeleportFailed(TeleportFailedData),
    /// The agent's health value changed.
    HealthChanged { health: f32 },
    /// The agent's L$ balance changed.
    MoneyBalance { balance: i32, description: String },
    /// An object was created or updated in the scene.
    ObjectUpdated(SimObject),
    /// One or more objects were removed from the scene.
    ObjectKilled { local_ids: Vec<u32> },
    /// The server returned detailed properties for an object.
    ObjectPropertiesReceived(ObjectPropertiesData),
    /// A caps event-queue event that has no dedicated variant yet.
    EventQueueEvent(String, serde_json::Value),
}

/// A chat message received from the simulator.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Display name of the sender.
    pub from_name: String,
    /// UUID of the sending object or avatar.
    pub source_id: Uuid,
    /// UUID of the owner (same as source_id for avatars).
    pub owner_id: Uuid,
    /// The text of the message.
    pub message: String,
    /// How the message was spoken.
    pub chat_type: ChatType,
    /// What kind of entity sent the message.
    pub source_type: SourceType,
    /// World position of the sender.
    pub position: glam::Vec3,
    /// Audible level (0 = fully audible in range).
    pub audible: u8,
}

/// How a chat message was spoken.
///
/// Values match the LLUDP `ChatFromSimulator.ChatData.ChatType` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ChatType {
    Whisper = 0,
    Normal = 1,
    Shout = 2,
    Say = 3,
    StartTyping = 4,
    StopTyping = 5,
    Debug = 6,
    Region = 7,
    OwnerSay = 8,
}

impl ChatType {
    /// Decode from the wire byte. Unknown values map to `Normal`.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Whisper,
            1 => Self::Normal,
            2 => Self::Shout,
            3 => Self::Say,
            4 => Self::StartTyping,
            5 => Self::StopTyping,
            6 => Self::Debug,
            7 => Self::Region,
            8 => Self::OwnerSay,
            _ => Self::Normal,
        }
    }

    /// Encode to the wire byte.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// What kind of entity originated a chat message.
///
/// Values match the LLUDP `ChatFromSimulator.ChatData.SourceType` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SourceType {
    System = 0,
    Agent = 1,
    Object = 2,
}

impl SourceType {
    /// Decode from the wire byte. Unknown values map to `System`.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::System,
            1 => Self::Agent,
            2 => Self::Object,
            _ => Self::System,
        }
    }

    /// Encode to the wire byte.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// An instant message received from another avatar or an object.
#[derive(Debug, Clone)]
pub struct InstantMessage {
    /// Display name of the sender.
    pub from_name: String,
    /// UUID of the sender.
    pub from_id: Uuid,
    /// UUID of the intended recipient.
    pub to_id: Uuid,
    /// The text of the message.
    pub message: String,
    /// IM dialog type byte (0 = normal message from agent).
    pub dialog: u8,
    /// Session ID for the IM conversation (used for group chat, conference).
    pub session_id: Uuid,
}

/// Information about a simulator region.
#[derive(Debug, Clone)]
pub struct RegionInfo {
    /// Human-readable region name.
    pub region_name: String,
    /// Packed region handle (high 32 bits = grid Y * 256, low 32 bits = grid X * 256).
    pub region_handle: u64,
    /// Grid X coordinate of the region's south-west corner in meters.
    pub region_x: u32,
    /// Grid Y coordinate of the region's south-west corner in meters.
    pub region_y: u32,
    /// IP address of the simulator.
    pub sim_ip: String,
    /// UDP port of the simulator.
    pub sim_port: u16,
    /// Seed capability URL for this region.
    pub seed_capability: String,
}

impl RegionInfo {
    /// Construct a `RegionInfo` from a region handle, computing x/y automatically.
    pub fn from_handle(
        region_name: String,
        region_handle: u64,
        sim_ip: String,
        sim_port: u16,
        seed_capability: String,
    ) -> Self {
        let region_x = (region_handle & 0xFFFF_FFFF) as u32;
        let region_y = ((region_handle >> 32) & 0xFFFF_FFFF) as u32;
        Self {
            region_name,
            region_handle,
            region_x,
            region_y,
            sim_ip,
            sim_port,
            seed_capability,
        }
    }

    /// Pack grid coordinates back into a region handle.
    pub fn to_handle(region_x: u32, region_y: u32) -> u64 {
        ((region_y as u64) << 32) | (region_x as u64)
    }
}

/// A teleport offer (lure) from another avatar.
#[derive(Debug, Clone)]
pub struct TeleportOfferData {
    /// Display name of the avatar who sent the offer.
    pub from_name: String,
    /// UUID of the avatar who sent the offer.
    pub from_id: Uuid,
    /// Lure session UUID (pass to TeleportLureRequest to accept).
    pub lure_id: Uuid,
    /// Message attached to the offer.
    pub message: String,
}

/// Data emitted when the server begins processing a teleport.
#[derive(Debug, Clone)]
pub struct TeleportStartData {
    /// Teleport flags indicating the type of teleport being started.
    pub flags: u32,
}

/// Server progress update during a teleport.
#[derive(Debug, Clone)]
pub struct TeleportProgressData {
    /// Teleport flags indicating progress stage.
    pub flags: u32,
    /// Human-readable progress message.
    pub message: String,
}

/// Data emitted when a teleport fails.
#[derive(Debug, Clone)]
pub struct TeleportFailedData {
    /// UUID of the agent whose teleport failed.
    pub agent_id: uuid::Uuid,
    /// Server-provided reason for the failure.
    pub reason: String,
}

/// A scene object as decoded from an inbound ObjectUpdate or related packet.
///
/// Contains the subset of object fields useful to bots: identity, position,
/// ownership, and hierarchy. Full shape and texture data are omitted.
#[derive(Debug, Clone)]
pub struct SimObject {
    /// Simulator-local object ID (changes across region crossings).
    pub local_id: u32,
    /// Globally unique object UUID (stable across regions).
    pub full_id: Uuid,
    /// Local ID of the parent object, or 0 if root.
    pub parent_id: u32,
    /// Primitive code (47 = prim, 9 = legacy grass, 5 = legacy tree).
    /// PCode 47 (Prim) is the vast majority. Avatar is 0x2F = 47 as well,
    /// but avatar updates have pcode == 0x2E (LL_PCODE_LEGACY_AVATAR = 46).
    pub pcode: u8,
    /// Object state byte (encodes attachment point).
    pub state: u8,
    /// Material type (0=stone, 1=metal, 2=glass, 3=wood, 4=flesh, 5=plastic, 6=rubber, 7=light).
    pub material: u8,
    /// Click action (0=touch, 1=sit, 2=buy, 3=pay, 4=open, 5=play, 6=open_media, 7=zoom).
    pub click_action: u8,
    /// Scale in meters.
    pub scale: glam::Vec3,
    /// World position in the region.
    pub position: glam::Vec3,
    /// World rotation.
    pub rotation: glam::Quat,
    /// Linear velocity.
    pub velocity: glam::Vec3,
    /// Linear acceleration.
    pub acceleration: glam::Vec3,
    /// Angular velocity.
    pub angular_velocity: glam::Vec3,
    /// Server-side update flags (permissions, physics, etc.).
    pub update_flags: u32,
    /// UUID of the object's owner.
    pub owner_id: Uuid,
    /// NameValue string (avatars encode FirstName/LastName here).
    pub name_value: String,
    /// Hover text above the object.
    pub text: String,
    /// RGBA color of the hover text.
    pub text_color: [u8; 4],
    /// Whether this object represents an avatar (pcode == 46).
    pub is_avatar: bool,
    /// Region handle from the update's RegionData block.
    pub region_handle: u64,
}

/// Detailed object properties returned by the server.
#[derive(Debug, Clone)]
pub struct ObjectPropertiesData {
    /// UUID of the object.
    pub object_id: Uuid,
    /// UUID of the creator.
    pub creator_id: Uuid,
    /// UUID of the current owner.
    pub owner_id: Uuid,
    /// UUID of the associated group.
    pub group_id: Uuid,
    /// Unix timestamp of creation.
    pub creation_date: u64,
    /// Permission masks.
    pub base_mask: u32,
    pub owner_mask: u32,
    pub group_mask: u32,
    pub everyone_mask: u32,
    pub next_owner_mask: u32,
    /// Sale info.
    pub sale_type: u8,
    pub sale_price: i32,
    /// Object name.
    pub name: String,
    /// Object description.
    pub description: String,
    /// Touch-action text.
    pub touch_name: String,
    /// Sit-action text.
    pub sit_name: String,
}

/// Broadcast bus for distributing events to subscribers.
///
/// Uses `tokio::sync::broadcast` internally, so subscribers that fall behind
/// will receive a `Lagged` error and skip to the newest event.
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    /// Create a new event bus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to events. Each subscriber gets its own receiver that
    /// independently tracks position in the stream.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// Publish an event to all current subscribers. If no subscribers exist
    /// or all are lagged, the event is silently dropped.
    pub fn send(&self, event: Event) {
        // Ignore the error (no active receivers) -- the event is simply lost.
        let _ = self.sender.send(event);
    }

    /// Return the number of active subscribers.
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_type_roundtrip() {
        for v in 0..=8u8 {
            let ct = ChatType::from_u8(v);
            assert_eq!(ct.as_u8(), v);
        }
        // Unknown value falls back to Normal (1)
        assert_eq!(ChatType::from_u8(99).as_u8(), 1);
    }

    #[test]
    fn source_type_roundtrip() {
        for v in 0..=2u8 {
            let st = SourceType::from_u8(v);
            assert_eq!(st.as_u8(), v);
        }
        assert_eq!(SourceType::from_u8(42).as_u8(), 0);
    }

    #[test]
    fn region_handle_roundtrip() {
        let x: u32 = 256000;
        let y: u32 = 257000;
        let handle = RegionInfo::to_handle(x, y);
        let info = RegionInfo::from_handle(
            "TestRegion".into(),
            handle,
            "127.0.0.1".into(),
            9000,
            String::new(),
        );
        assert_eq!(info.region_x, x);
        assert_eq!(info.region_y, y);
        assert_eq!(info.region_handle, handle);
    }

    #[tokio::test]
    async fn event_bus_delivers_to_subscriber() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        bus.send(Event::Disconnected(DisconnectReason::ClientRequested));

        let event = rx.recv().await.expect("should receive event");
        assert!(matches!(event, Event::Disconnected(DisconnectReason::ClientRequested)));
    }

    #[tokio::test]
    async fn event_bus_no_subscribers_does_not_panic() {
        let bus = EventBus::new(16);
        // No subscribers -- should not panic or error.
        bus.send(Event::Disconnected(DisconnectReason::NetworkTimeout));
        assert_eq!(bus.receiver_count(), 0);
    }
}
