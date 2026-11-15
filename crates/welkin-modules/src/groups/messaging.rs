// SPDX-License-Identifier: GPL-3.0-or-later

//! Groups messaging module — group chat and notice delivery.
//!
//! Ports the C# `GroupsMessagingModule` from
//! `OpenSim/Addons/Groups/GroupsMessagingModule.cs`.
//!
//! Routes group IMs through [`MessageTransferModule`], manages per-agent
//! chat session tracking (invited / dropped), and delivers group notices
//! with optional inventory-item attachments.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::messaging::{GridInstantMessage, IMDialog};
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Chat session state
// ---------------------------------------------------------------------------

/// Tracks which agents are participating in each group chat session.
///
/// Mirrors the C# dictionaries:
/// - `m_groupsAgentsDroppedFromChatSession`
/// - `m_groupsAgentsInvitedToChatSession`
#[derive(Debug, Clone, Default)]
pub struct ChatSessionState {
    /// Agents currently invited/active in group chat sessions.
    /// Key: group_id, Value: set of agent_ids.
    pub invited: HashMap<Uuid, HashSet<Uuid>>,
    /// Agents who have dropped (left) group chat sessions.
    /// Key: group_id, Value: set of agent_ids.
    pub dropped: HashMap<Uuid, HashSet<Uuid>>,
}

impl ChatSessionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark an agent as invited to a group chat session.
    pub fn invite_agent(&mut self, group_id: Uuid, agent_id: Uuid) {
        self.invited.entry(group_id).or_default().insert(agent_id);
        // Remove from dropped if re-invited.
        if let Some(dropped) = self.dropped.get_mut(&group_id) {
            dropped.remove(&agent_id);
        }
    }

    /// Mark an agent as having dropped from a group chat session.
    pub fn drop_agent(&mut self, group_id: Uuid, agent_id: Uuid) {
        self.dropped.entry(group_id).or_default().insert(agent_id);
        if let Some(invited) = self.invited.get_mut(&group_id) {
            invited.remove(&agent_id);
        }
    }

    /// Check whether an agent is active (invited and not dropped) in a session.
    pub fn is_agent_active(&self, group_id: Uuid, agent_id: Uuid) -> bool {
        let invited = self
            .invited
            .get(&group_id)
            .is_some_and(|s| s.contains(&agent_id));
        let dropped = self
            .dropped
            .get(&group_id)
            .is_some_and(|s| s.contains(&agent_id));
        invited && !dropped
    }

    /// Check whether an agent has been explicitly dropped from a session.
    pub fn has_agent_dropped(&self, group_id: Uuid, agent_id: Uuid) -> bool {
        self.dropped
            .get(&group_id)
            .is_some_and(|s| s.contains(&agent_id))
    }

    /// Get all active agents in a group chat session.
    pub fn active_agents(&self, group_id: Uuid) -> Vec<Uuid> {
        let invited = match self.invited.get(&group_id) {
            Some(s) => s,
            None => return Vec::new(),
        };
        let dropped = self.dropped.get(&group_id);
        invited
            .iter()
            .filter(|a| dropped.is_none_or(|d| !d.contains(a)))
            .copied()
            .collect()
    }

    /// Remove all session state for a group.
    pub fn clear_session(&mut self, group_id: Uuid) {
        self.invited.remove(&group_id);
        self.dropped.remove(&group_id);
    }

    /// Remove all session state.
    pub fn clear_all(&mut self) {
        self.invited.clear();
        self.dropped.clear();
    }

    /// Number of tracked sessions (groups with at least one invited agent).
    pub fn session_count(&self) -> usize {
        self.invited.len()
    }
}

// ---------------------------------------------------------------------------
// Group notice attachment info
// ---------------------------------------------------------------------------

/// Attachment metadata for a group notice.
#[derive(Debug, Clone)]
pub struct NoticeAttachment {
    pub item_id: Uuid,
    pub owner_id: Uuid,
    pub asset_type: u8,
    pub name: String,
}

// ---------------------------------------------------------------------------
// GroupsMessagingModule
// ---------------------------------------------------------------------------

/// Routes group instant messages and notices to online group members.
///
/// Mirrors the C# `GroupsMessagingModule` (ISharedRegionModule +
/// IGroupsMessagingModule).
///
/// In the C# implementation this module:
/// 1. Receives group IMs from the viewer
/// 2. Fans them out to all online group members via scene.ForEachAgent
/// 3. Tracks per-agent chat session state (invited/dropped)
/// 4. Handles InstantMessage dialog types: SessionSend, SessionGroupStart,
///    SessionDrop
/// 5. Delivers group notices with optional inventory attachments
pub struct GroupsMessagingModule {
    enabled: bool,
    message_online_only: bool,
    online_cache_expiry_secs: u64,
    scenes: Vec<String>,
    sessions: ChatSessionState,
}

impl GroupsMessagingModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            message_online_only: false,
            online_cache_expiry_secs: 20,
            scenes: Vec::new(),
            sessions: ChatSessionState::new(),
        }
    }

    /// Whether the module is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Access the chat session tracking state.
    pub fn sessions(&self) -> &ChatSessionState {
        &self.sessions
    }

    /// Mutable access to sessions (for external agents to update).
    pub fn sessions_mut(&mut self) -> &mut ChatSessionState {
        &mut self.sessions
    }

    // -- IM handlers --------------------------------------------------------

    /// Start a group chat session for an agent.
    ///
    /// Corresponds to `IMDialog::SessionGroupStart` / `Session911Start`.
    pub fn handle_session_group_start(&mut self, agent_id: Uuid, group_id: Uuid) {
        if !self.enabled {
            return;
        }
        tracing::debug!(
            agent = %agent_id,
            group = %group_id,
            "SessionGroupStart"
        );
        self.sessions.invite_agent(group_id, agent_id);
    }

    /// Handle an agent sending a message to a group chat session.
    ///
    /// Corresponds to `IMDialog::SessionSend`.  Returns the list of
    /// recipient agent IDs to fan the message out to.
    pub fn handle_session_send(&self, agent_id: Uuid, group_id: Uuid, message: &str) -> Vec<Uuid> {
        if !self.enabled {
            return Vec::new();
        }
        if !self.sessions.is_agent_active(group_id, agent_id) {
            tracing::debug!(
                agent = %agent_id,
                group = %group_id,
                "SessionSend from non-active agent"
            );
            return Vec::new();
        }

        tracing::debug!(
            agent = %agent_id,
            group = %group_id,
            message_len = message.len(),
            "SessionSend"
        );

        // Return all active agents except the sender.
        self.sessions
            .active_agents(group_id)
            .into_iter()
            .filter(|a| *a != agent_id)
            .collect()
    }

    /// Handle an agent dropping from a group chat session.
    ///
    /// Corresponds to `IMDialog::SessionDrop`.
    pub fn handle_session_drop(&mut self, agent_id: Uuid, group_id: Uuid) {
        if !self.enabled {
            return;
        }
        tracing::debug!(
            agent = %agent_id,
            group = %group_id,
            "SessionDrop"
        );
        self.sessions.drop_agent(group_id, agent_id);
    }

    /// Build a `GridInstantMessage` for a group chat message to fan out.
    pub fn build_group_im(
        from_agent_id: Uuid,
        from_agent_name: &str,
        to_agent_id: Uuid,
        group_id: Uuid,
        message: &str,
    ) -> GridInstantMessage {
        GridInstantMessage {
            from_agent_id,
            from_agent_name: from_agent_name.to_string(),
            to_agent_id,
            message: message.to_string(),
            im_session_id: group_id,
            dialog: IMDialog::SessionAdd,
            from_group: true,
            offline: 0,
            parent_estate_id: 0,
            position: [0.0, 0.0, 0.0],
            region_id: Uuid::nil(),
            timestamp: 0,
            binary_bucket: Vec::new(),
        }
    }

    /// Build a `GridInstantMessage` for a group notice.
    pub fn build_group_notice(
        from_agent_id: Uuid,
        from_agent_name: &str,
        to_agent_id: Uuid,
        group_id: Uuid,
        subject: &str,
        message: &str,
        attachment: Option<&NoticeAttachment>,
    ) -> GridInstantMessage {
        let mut bucket = Vec::new();
        if let Some(att) = attachment {
            // Bucket format: [has_att(1), att_type(1), name..., \0,
            //                 item_id(16), owner_id(16)]
            bucket.push(1u8);
            bucket.push(att.asset_type);
            bucket.extend_from_slice(att.name.as_bytes());
            bucket.push(0);
            bucket.extend_from_slice(att.item_id.as_bytes());
            bucket.extend_from_slice(att.owner_id.as_bytes());
        } else {
            bucket.push(0u8);
        }

        let combined = format!("{subject}|{message}");

        GridInstantMessage {
            from_agent_id,
            from_agent_name: from_agent_name.to_string(),
            to_agent_id,
            message: combined,
            im_session_id: group_id,
            dialog: IMDialog::GroupNotice,
            from_group: true,
            offline: 1, // allow offline delivery
            parent_estate_id: 0,
            position: [0.0, 0.0, 0.0],
            region_id: Uuid::nil(),
            timestamp: 0,
            binary_bucket: bucket,
        }
    }

    /// Process an incoming group IM based on its dialog type.
    ///
    /// Returns the list of recipients for fan-out, or empty if the
    /// message was handled internally (e.g., SessionDrop).
    pub fn process_group_im(&mut self, im: &GridInstantMessage) -> Vec<Uuid> {
        if !self.enabled {
            return Vec::new();
        }

        let group_id = im.im_session_id;
        let agent_id = im.from_agent_id;

        match im.dialog {
            IMDialog::Session911Start => {
                self.handle_session_group_start(agent_id, group_id);
                Vec::new()
            }
            IMDialog::SessionAdd => {
                // SessionSend equivalent — fan out to active members.
                self.handle_session_send(agent_id, group_id, &im.message)
            }
            IMDialog::SessionDrop => {
                self.handle_session_drop(agent_id, group_id);
                Vec::new()
            }
            IMDialog::GroupNotice => {
                // Notice — deliver to all group members (via caller).
                tracing::debug!(
                    agent = %agent_id,
                    group = %group_id,
                    "GroupNotice IM"
                );
                Vec::new()
            }
            _ => {
                tracing::debug!(
                    dialog = ?im.dialog,
                    "unhandled group IM dialog type"
                );
                Vec::new()
            }
        }
    }
}

impl Default for GroupsMessagingModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for GroupsMessagingModule {
    fn name(&self) -> &str {
        "GroupsMessagingModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let groups_enabled = config.get_bool("Groups", "Enabled", false);
        let messaging_module = config.get_string("Groups", "MessagingModule", "");

        if !groups_enabled || (messaging_module != self.name() && !messaging_module.is_empty()) {
            tracing::info!("GroupsMessagingModule disabled");
            return Ok(());
        }

        self.enabled = true;
        self.message_online_only = config.get_bool("Groups", "MessageOnlineUsersOnly", false);
        self.online_cache_expiry_secs = config.get_int("Groups", "OnlineCacheExpiry", 20) as u64;

        tracing::info!(
            online_only = self.message_online_only,
            cache_expiry = self.online_cache_expiry_secs,
            "GroupsMessagingModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        if !self.enabled {
            return Ok(());
        }
        self.scenes.push(scene.region_name.clone());
        tracing::info!(
            region = %scene.region_name,
            "GroupsMessagingModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(
            region = %scene.region_name,
            "GroupsMessagingModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.sessions.clear_all();
        tracing::info!("GroupsMessagingModule closed");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::ModuleConfig;
    use crate::registry::SceneRef;
    use std::collections::HashMap;

    struct TestConfig {
        strings: HashMap<(String, String), String>,
        ints: HashMap<(String, String), i64>,
        bools: HashMap<(String, String), bool>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: HashMap::new(),
                ints: HashMap::new(),
                bools: HashMap::new(),
            }
        }

        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }

        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
        }
    }

    impl ModuleConfig for TestConfig {
        fn get_string(&self, section: &str, key: &str, default: &str) -> String {
            self.strings
                .get(&(section.to_string(), key.to_string()))
                .cloned()
                .unwrap_or_else(|| default.to_string())
        }
        fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
            self.ints
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }
        fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
            self.bools
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }
        fn get_float(&self, _section: &str, _key: &str, default: f64) -> f64 {
            default
        }
    }

    fn test_scene_ref() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    fn enabled_config() -> TestConfig {
        let mut c = TestConfig::new();
        c.set_bool("Groups", "Enabled", true);
        c.set_string("Groups", "MessagingModule", "GroupsMessagingModule");
        c
    }

    // 1. ChatSessionState invite and check.
    #[test]
    fn session_invite_and_active() {
        let mut state = ChatSessionState::new();
        let group = Uuid::new_v4();
        let agent = Uuid::new_v4();

        assert!(!state.is_agent_active(group, agent));
        state.invite_agent(group, agent);
        assert!(state.is_agent_active(group, agent));
    }

    // 2. ChatSessionState drop agent.
    #[test]
    fn session_drop_agent() {
        let mut state = ChatSessionState::new();
        let group = Uuid::new_v4();
        let agent = Uuid::new_v4();

        state.invite_agent(group, agent);
        assert!(state.is_agent_active(group, agent));

        state.drop_agent(group, agent);
        assert!(!state.is_agent_active(group, agent));
        assert!(state.has_agent_dropped(group, agent));
    }

    // 3. ChatSessionState re-invite after drop.
    #[test]
    fn session_reinvite_after_drop() {
        let mut state = ChatSessionState::new();
        let group = Uuid::new_v4();
        let agent = Uuid::new_v4();

        state.invite_agent(group, agent);
        state.drop_agent(group, agent);
        assert!(!state.is_agent_active(group, agent));

        state.invite_agent(group, agent);
        assert!(state.is_agent_active(group, agent));
        assert!(!state.has_agent_dropped(group, agent));
    }

    // 4. ChatSessionState active_agents filters dropped.
    #[test]
    fn session_active_agents() {
        let mut state = ChatSessionState::new();
        let group = Uuid::new_v4();
        let a1 = Uuid::new_v4();
        let a2 = Uuid::new_v4();
        let a3 = Uuid::new_v4();

        state.invite_agent(group, a1);
        state.invite_agent(group, a2);
        state.invite_agent(group, a3);
        state.drop_agent(group, a2);

        let active = state.active_agents(group);
        assert_eq!(active.len(), 2);
        assert!(active.contains(&a1));
        assert!(active.contains(&a3));
        assert!(!active.contains(&a2));
    }

    // 5. ChatSessionState clear_session.
    #[test]
    fn session_clear() {
        let mut state = ChatSessionState::new();
        let group = Uuid::new_v4();
        state.invite_agent(group, Uuid::new_v4());
        assert_eq!(state.session_count(), 1);

        state.clear_session(group);
        assert_eq!(state.session_count(), 0);
    }

    // 6. GroupsMessagingModule lifecycle.
    #[test]
    fn messaging_module_lifecycle() {
        let mut module = GroupsMessagingModule::new();
        let config = enabled_config();
        let scene = test_scene_ref();

        module.initialise(&config).unwrap();
        assert!(module.is_enabled());
        module.add_region(&scene).unwrap();
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // 7. GroupsMessagingModule disabled by default.
    #[test]
    fn messaging_disabled_by_default() {
        let mut module = GroupsMessagingModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.is_enabled());
    }

    // 8. handle_session_group_start adds agent.
    #[test]
    fn messaging_session_start() {
        let mut module = GroupsMessagingModule::new();
        let config = enabled_config();
        module.initialise(&config).unwrap();

        let group = Uuid::new_v4();
        let agent = Uuid::new_v4();
        module.handle_session_group_start(agent, group);
        assert!(module.sessions().is_agent_active(group, agent));
    }

    // 9. handle_session_send fans out to active agents.
    #[test]
    fn messaging_session_send() {
        let mut module = GroupsMessagingModule::new();
        let config = enabled_config();
        module.initialise(&config).unwrap();

        let group = Uuid::new_v4();
        let sender = Uuid::new_v4();
        let other = Uuid::new_v4();

        module.handle_session_group_start(sender, group);
        module.handle_session_group_start(other, group);

        let recipients = module.handle_session_send(sender, group, "Hello");
        assert_eq!(recipients.len(), 1);
        assert_eq!(recipients[0], other);
    }

    // 10. handle_session_send from non-active agent returns empty.
    #[test]
    fn messaging_session_send_non_active() {
        let mut module = GroupsMessagingModule::new();
        let config = enabled_config();
        module.initialise(&config).unwrap();

        let group = Uuid::new_v4();
        let outsider = Uuid::new_v4();
        let recipients = module.handle_session_send(outsider, group, "Hello");
        assert!(recipients.is_empty());
    }

    // 11. handle_session_drop removes agent.
    #[test]
    fn messaging_session_drop() {
        let mut module = GroupsMessagingModule::new();
        let config = enabled_config();
        module.initialise(&config).unwrap();

        let group = Uuid::new_v4();
        let agent = Uuid::new_v4();
        module.handle_session_group_start(agent, group);
        module.handle_session_drop(agent, group);
        assert!(!module.sessions().is_agent_active(group, agent));
    }

    // 12. build_group_im creates correct IM.
    #[test]
    fn build_group_im_fields() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let group = Uuid::new_v4();
        let im = GroupsMessagingModule::build_group_im(from, "Test User", to, group, "Hello");
        assert_eq!(im.from_agent_id, from);
        assert_eq!(im.to_agent_id, to);
        assert_eq!(im.im_session_id, group);
        assert_eq!(im.message, "Hello");
        assert!(im.from_group);
        assert_eq!(im.dialog, IMDialog::SessionAdd);
    }

    // 13. build_group_notice without attachment.
    #[test]
    fn build_group_notice_no_attachment() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let group = Uuid::new_v4();
        let im = GroupsMessagingModule::build_group_notice(
            from, "Sender", to, group, "Subject", "Body", None,
        );
        assert_eq!(im.dialog, IMDialog::GroupNotice);
        assert!(im.message.contains("Subject"));
        assert!(im.message.contains("Body"));
        assert_eq!(im.binary_bucket, vec![0u8]);
    }

    // 14. build_group_notice with attachment.
    #[test]
    fn build_group_notice_with_attachment() {
        let att = NoticeAttachment {
            item_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            asset_type: 7,
            name: "Notecard".to_string(),
        };
        let im = GroupsMessagingModule::build_group_notice(
            Uuid::new_v4(),
            "Sender",
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Subject",
            "Body",
            Some(&att),
        );
        assert_eq!(im.binary_bucket[0], 1);
        assert_eq!(im.binary_bucket[1], 7);
        // Bucket should contain has_att(1) + type(1) + name + null(1)
        // + item_id(16) + owner_id(16).
        assert_eq!(im.binary_bucket.len(), 2 + att.name.len() + 1 + 16 + 16);
    }

    // 15. Module name and type.
    #[test]
    fn messaging_module_name_and_type() {
        let module = GroupsMessagingModule::new();
        assert_eq!(module.name(), "GroupsMessagingModule");
        assert_eq!(module.module_type(), ModuleType::Shared);
    }

    // 16. process_group_im dispatches session start.
    #[test]
    fn process_im_session_start() {
        let mut module = GroupsMessagingModule::new();
        let config = enabled_config();
        module.initialise(&config).unwrap();

        let group = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let im = GridInstantMessage {
            from_agent_id: agent,
            from_agent_name: "Agent".to_string(),
            to_agent_id: Uuid::nil(),
            message: String::new(),
            im_session_id: group,
            dialog: IMDialog::Session911Start,
            from_group: true,
            offline: 0,
            parent_estate_id: 0,
            position: [0.0; 3],
            region_id: Uuid::nil(),
            timestamp: 0,
            binary_bucket: Vec::new(),
        };

        let result = module.process_group_im(&im);
        assert!(result.is_empty()); // Session start doesn't fan out.
        assert!(module.sessions().is_agent_active(group, agent));
    }
}
