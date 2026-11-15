// SPDX-License-Identifier: GPL-3.0-or-later

//! Instant messaging, offline message queuing, mute lists, and presence
//! tracking.
//!
//! This module implements the Rust equivalents of the C# `MessageTransferModule`,
//! `InstantMessageModule`, `MuteListModule`, and a lightweight `PresenceModule`
//! that tracks which agents are online in a region.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// IMDialog — instant-message dialog types
// ---------------------------------------------------------------------------

/// Dialog type for a [`GridInstantMessage`], matching the C#
/// `InstantMessageDialog` enum values used by the viewer protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IMDialog {
    MessageFromAgent = 0,
    MessageBox = 1,
    GroupInvitation = 3,
    InventoryOffered = 4,
    InventoryAccepted = 5,
    InventoryDeclined = 6,
    TaskInventoryOffered = 7,
    GroupVote = 8,
    TaskInventoryAccepted = 9,
    TaskInventoryDeclined = 10,
    NewUserDefault = 11,
    SessionAdd = 13,
    SessionDrop = 14,
    SessionGroupStart = 17,
    SessionCardlessStart = 18,
    MessageFromObject = 19,
    SessionSend = 20,
    ConsoleAndChatHistory = 21,
    RequestTeleport = 22,
    AcceptTeleport = 23,
    DenyTeleport = 24,
    GodLikeRequestTeleport = 25,
    RequestLure = 26,
    GotoUrl = 28,
    Session911Start = 29,
    Lure911 = 30,
    FromTaskAsAlert = 31,
    GroupNotice = 32,
    GroupNoticeInventoryAccepted = 33,
    GroupNoticeInventoryDeclined = 34,
    GroupInvitationAccept = 35,
    GroupInvitationDecline = 36,
    GroupNoticeRequested = 37,
    FriendshipOffered = 38,
    FriendshipAccepted = 39,
    FriendshipDeclined = 40,
    StartTyping = 41,
    StopTyping = 42,
}

impl IMDialog {
    pub const BUSY_AUTO_RESPONSE: u8 = 20;

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::MessageFromAgent),
            1 => Some(Self::MessageBox),
            3 => Some(Self::GroupInvitation),
            4 => Some(Self::InventoryOffered),
            5 => Some(Self::InventoryAccepted),
            6 => Some(Self::InventoryDeclined),
            7 => Some(Self::TaskInventoryOffered),
            8 => Some(Self::GroupVote),
            9 => Some(Self::TaskInventoryAccepted),
            10 => Some(Self::TaskInventoryDeclined),
            11 => Some(Self::NewUserDefault),
            13 => Some(Self::SessionAdd),
            14 => Some(Self::SessionDrop),
            17 => Some(Self::SessionGroupStart),
            18 => Some(Self::SessionCardlessStart),
            19 => Some(Self::MessageFromObject),
            20 => Some(Self::SessionSend),
            21 => Some(Self::ConsoleAndChatHistory),
            22 => Some(Self::RequestTeleport),
            23 => Some(Self::AcceptTeleport),
            24 => Some(Self::DenyTeleport),
            25 => Some(Self::GodLikeRequestTeleport),
            26 => Some(Self::RequestLure),
            28 => Some(Self::GotoUrl),
            29 => Some(Self::Session911Start),
            30 => Some(Self::Lure911),
            31 => Some(Self::FromTaskAsAlert),
            32 => Some(Self::GroupNotice),
            33 => Some(Self::GroupNoticeInventoryAccepted),
            34 => Some(Self::GroupNoticeInventoryDeclined),
            35 => Some(Self::GroupInvitationAccept),
            36 => Some(Self::GroupInvitationDecline),
            37 => Some(Self::GroupNoticeRequested),
            38 => Some(Self::FriendshipOffered),
            39 => Some(Self::FriendshipAccepted),
            40 => Some(Self::FriendshipDeclined),
            41 => Some(Self::StartTyping),
            42 => Some(Self::StopTyping),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// GridInstantMessage
// ---------------------------------------------------------------------------

/// A grid-level instant message, mirroring the C# `GridInstantMessage` class.
#[derive(Debug, Clone)]
pub struct GridInstantMessage {
    pub from_agent_id: Uuid,
    pub from_agent_name: String,
    pub to_agent_id: Uuid,
    pub message: String,
    pub im_session_id: Uuid,
    pub dialog: IMDialog,
    pub from_group: bool,
    pub offline: u8,
    pub parent_estate_id: u32,
    pub position: [f32; 3],
    pub region_id: Uuid,
    pub timestamp: u32,
    pub binary_bucket: Vec<u8>,
}

// ---------------------------------------------------------------------------
// MuteType / MuteEntry
// ---------------------------------------------------------------------------

/// How an entry on the mute list is keyed, matching the C# `MuteData.MuteType`
/// values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MuteType {
    ByName = 0,
    ByAgent = 1,
    ByGroup = 2,
    ByObject = 3,
}

/// A single entry on an agent's mute (ignore) list.
#[derive(Debug, Clone)]
pub struct MuteEntry {
    pub agent_id: Uuid,
    pub mute_id: Uuid,
    pub name: String,
    pub mute_type: MuteType,
    pub flags: u32,
}

// ---------------------------------------------------------------------------
// MessageTransferModule
// ---------------------------------------------------------------------------

/// Routes instant messages to local agents or marks them for grid delivery.
///
/// Mirrors the C# `MessageTransferModule`.  Currently a stub: local delivery
/// is logged but not wired to a real client connection.
pub struct MessageTransferModule {
    enabled: bool,
    scenes: Vec<String>,
    message_key: String,
    offline_queue: Vec<GridInstantMessage>,
}

impl MessageTransferModule {
    pub fn new() -> Self {
        Self {
            enabled: true,
            scenes: Vec::new(),
            message_key: String::new(),
            offline_queue: Vec::new(),
        }
    }

    /// Attempt to deliver an instant message.
    ///
    /// Returns `true` when the message was accepted for delivery.
    /// Currently accepts all non-nil recipients. Actual per-session
    /// delivery is handled by the region's IM routing.
    pub fn send_instant_message(&self, im: &GridInstantMessage) -> bool {
        if im.to_agent_id.is_nil() {
            return false;
        }
        tracing::debug!(
            from = %im.from_agent_id,
            to = %im.to_agent_id,
            dialog = ?im.dialog,
            "send_instant_message"
        );
        true
    }

    /// Handle a message that could not be delivered to any online agent.
    ///
    /// Queues the message for offline delivery. The queue is drained when
    /// the recipient next logs in.
    pub fn handle_undeliverable(&mut self, im: &GridInstantMessage) {
        tracing::debug!(
            from = %im.from_agent_id,
            to = %im.to_agent_id,
            dialog = ?im.dialog,
            "queuing undeliverable IM for offline delivery"
        );
        self.offline_queue.push(im.clone());
    }

    /// Drain all queued offline messages for the given agent.
    pub fn drain_offline_messages(&mut self, agent_id: Uuid) -> Vec<GridInstantMessage> {
        let mut remaining = Vec::new();
        let mut delivered = Vec::new();
        for im in self.offline_queue.drain(..) {
            if im.to_agent_id == agent_id {
                delivered.push(im);
            } else {
                remaining.push(im);
            }
        }
        self.offline_queue = remaining;
        if !delivered.is_empty() {
            tracing::debug!(
                %agent_id,
                count = delivered.len(),
                "drained offline messages"
            );
        }
        delivered
    }

    /// Number of queued offline messages (for diagnostics).
    pub fn offline_queue_len(&self) -> usize {
        self.offline_queue.len()
    }
}

impl Default for MessageTransferModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for MessageTransferModule {
    fn name(&self) -> &str {
        "MessageTransferModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.message_key = config.get_string("Messaging", "MessageKey", "");
        tracing::info!("MessageTransferModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.push(scene.region_name.clone());
        tracing::info!(region = %scene.region_name, "MessageTransferModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(region = %scene.region_name, "MessageTransferModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// OfflineMessageModule
// ---------------------------------------------------------------------------

/// Stores instant messages for agents who are offline and replays them on
/// login.
///
/// Stub implementation backed by an in-memory `HashMap`; a real deployment
/// would persist to a database.
pub struct OfflineMessageModule {
    enabled: bool,
    messages: HashMap<Uuid, Vec<GridInstantMessage>>,
}

impl OfflineMessageModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            messages: HashMap::new(),
        }
    }

    /// Queue a message for later delivery.
    pub fn store_message(&mut self, im: GridInstantMessage) {
        tracing::debug!(to = %im.to_agent_id, "storing offline message");
        self.messages.entry(im.to_agent_id).or_default().push(im);
    }

    /// Retrieve and drain all pending messages for `agent_id`.
    pub fn retrieve_messages(&mut self, agent_id: Uuid) -> Vec<GridInstantMessage> {
        self.messages.remove(&agent_id).unwrap_or_default()
    }

    /// Number of messages queued for a specific agent.
    pub fn pending_count(&self, agent_id: Uuid) -> usize {
        self.messages.get(&agent_id).map_or(0, |v| v.len())
    }

    /// Total number of messages queued across all agents.
    pub fn total_pending(&self) -> usize {
        self.messages.values().map(|v| v.len()).sum()
    }
}

impl Default for OfflineMessageModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for OfflineMessageModule {
    fn name(&self) -> &str {
        "OfflineMessageModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.enabled = config.get_bool("Messaging", "OfflineMessageModule", false);
        tracing::info!(enabled = self.enabled, "OfflineMessageModule initialised");
        Ok(())
    }

    fn add_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MuteListModule
// ---------------------------------------------------------------------------

/// Manages per-agent mute (ignore) lists.
///
/// Mirrors the C# `MuteListModule`.  Mute entries are stored in memory;
/// a real deployment would persist to a database via `IMuteListService`.
pub struct MuteListModule {
    enabled: bool,
    mutes: HashMap<Uuid, Vec<MuteEntry>>,
}

impl MuteListModule {
    pub fn new() -> Self {
        Self {
            enabled: true,
            mutes: HashMap::new(),
        }
    }

    /// Add or replace a mute entry for `owner_id`.
    ///
    /// If an entry with the same `mute_id` and `mute_type` already exists it
    /// is replaced; otherwise the new entry is appended.
    pub fn update_mute(&mut self, owner_id: Uuid, entry: MuteEntry) {
        let list = self.mutes.entry(owner_id).or_default();
        if let Some(pos) = list
            .iter()
            .position(|e| e.mute_id == entry.mute_id && e.mute_type == entry.mute_type)
        {
            list[pos] = entry;
        } else {
            list.push(entry);
        }
    }

    /// Remove a mute entry matching `mute_id` and `name`.
    pub fn remove_mute(&mut self, owner_id: Uuid, mute_id: Uuid, name: &str) {
        if let Some(list) = self.mutes.get_mut(&owner_id) {
            list.retain(|e| !(e.mute_id == mute_id && e.name == name));
        }
    }

    /// Return a clone of the mute list for `owner_id`.
    pub fn get_mutes(&self, owner_id: Uuid) -> Vec<MuteEntry> {
        self.mutes.get(&owner_id).cloned().unwrap_or_default()
    }

    /// Check whether `target_id` is on `owner_id`'s mute list (by `mute_id`).
    pub fn is_muted(&self, owner_id: Uuid, target_id: Uuid) -> bool {
        self.mutes
            .get(&owner_id)
            .is_some_and(|list| list.iter().any(|e| e.mute_id == target_id))
    }
}

impl Default for MuteListModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for MuteListModule {
    fn name(&self) -> &str {
        "MuteListModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("MuteListModule initialised");
        Ok(())
    }

    fn add_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PresenceModule
// ---------------------------------------------------------------------------

/// Tracks which agents are online in this region.
///
/// A lightweight in-memory set; the real grid-wide presence service is
/// separate.
pub struct PresenceModule {
    present: HashSet<Uuid>,
}

impl PresenceModule {
    pub fn new() -> Self {
        Self {
            present: HashSet::new(),
        }
    }

    /// Record an agent as present in the region.
    pub fn register_presence(&mut self, agent_id: Uuid) {
        self.present.insert(agent_id);
        tracing::debug!(%agent_id, "presence registered");
    }

    /// Remove an agent's presence record.
    pub fn deregister_presence(&mut self, agent_id: Uuid) {
        self.present.remove(&agent_id);
        tracing::debug!(%agent_id, "presence deregistered");
    }

    /// Check whether an agent is present in this region.
    pub fn is_present(&self, agent_id: Uuid) -> bool {
        self.present.contains(&agent_id)
    }

    /// Return all currently present agent IDs.
    pub fn get_present_agents(&self) -> Vec<Uuid> {
        self.present.iter().copied().collect()
    }

    /// Number of agents currently present.
    pub fn presence_count(&self) -> usize {
        self.present.len()
    }
}

impl Default for PresenceModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for PresenceModule {
    fn name(&self) -> &str {
        "PresenceModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("PresenceModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "PresenceModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "PresenceModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.present.clear();
        Ok(())
    }
}
