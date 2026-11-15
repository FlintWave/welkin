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
