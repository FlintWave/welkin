// SPDX-License-Identifier: GPL-3.0-or-later

//! Offline IM region module.
//!
//! Mirrors the C# `OfflineIMRegionModule`.  Hooks into the message transfer
//! module's undelivered callback to store messages for offline agents, and
//! replays them when the agent logs in.

use std::collections::HashMap;

use uuid::Uuid;

use crate::messaging::{GridInstantMessage, IMDialog};
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

/// Dialog types that should be stored for offline delivery, matching C#.
const STORABLE_DIALOGS: &[IMDialog] = &[
    IMDialog::MessageFromObject,
    IMDialog::MessageFromAgent,
    IMDialog::GroupNotice,
    IMDialog::GroupInvitation,
    IMDialog::InventoryOffered,
];

/// Region module that stores instant messages for offline agents and delivers
/// them on login.
///
/// This is the region-side counterpart of the `OfflineIMService`.  In a full
/// deployment the module forwards to the service over HTTP; here it uses an
/// in-memory store matching the existing `OfflineMessageModule` pattern with
/// extended offline-IM semantics from the C# `OfflineIMRegionModule`.
pub struct OfflineIMModule {
    enabled: bool,
    scenes: Vec<String>,
    forward_offline_group_messages: bool,
    max_offline_ims: usize,
    /// In-memory pending messages keyed by recipient UUID.
    pending: HashMap<Uuid, Vec<GridInstantMessage>>,
}

impl OfflineIMModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scenes: Vec::new(),
            forward_offline_group_messages: true,
            max_offline_ims: 25,
            pending: HashMap::new(),
        }
    }

    /// Whether the module is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Whether to forward offline group messages.
    pub fn forwards_group_messages(&self) -> bool {
        self.forward_offline_group_messages
    }

    /// Maximum offline IMs per recipient.
    pub fn max_offline_ims(&self) -> usize {
        self.max_offline_ims
    }

    /// Check whether a dialog type should be stored for offline delivery.
    fn is_storable_dialog(&self, dialog: IMDialog) -> bool {
        if !STORABLE_DIALOGS.contains(&dialog) {
            return false;
        }
        // If group message forwarding is disabled, reject group dialogs.
        if !self.forward_offline_group_messages
            && (dialog == IMDialog::GroupNotice || dialog == IMDialog::GroupInvitation)
        {
            return false;
        }
        true
    }

    /// Handle an undeliverable message.  Stores it for later delivery if the
    /// dialog type qualifies, returning `Ok(true)` on success, `Ok(false)` if
    /// the queue is full, or an error message.
    pub fn handle_undelivered(&mut self, im: &GridInstantMessage) -> Result<bool, String> {
        if !self.enabled {
            return Err("module not enabled".to_string());
        }
        if !self.is_storable_dialog(im.dialog) {
            return Ok(false);
        }

        let queue = self.pending.entry(im.to_agent_id).or_default();
        if queue.len() >= self.max_offline_ims {
            return Err("Number of offline IMs has maxed out".to_string());
        }

        tracing::debug!(
            from = %im.from_agent_id,
            to = %im.to_agent_id,
            dialog = ?im.dialog,
            "storing offline message"
        );
        queue.push(im.clone());
        Ok(true)
    }

    /// Retrieve and drain all pending offline messages for `agent_id`,
    /// simulating the login-retrieval flow from C#.
    pub fn retrieve_messages(&mut self, agent_id: Uuid) -> Vec<GridInstantMessage> {
        self.pending.remove(&agent_id).unwrap_or_default()
    }

    /// Number of pending messages for a specific agent.
    pub fn pending_count(&self, agent_id: Uuid) -> usize {
        self.pending.get(&agent_id).map_or(0, Vec::len)
    }

    /// Total number of pending messages across all agents.
    pub fn total_pending(&self) -> usize {
        self.pending.values().map(Vec::len).sum()
    }

    /// Delete all messages for a user (both as recipient and sender).
    pub fn delete_messages(&mut self, user_id: Uuid) {
        self.pending.remove(&user_id);
        // Also remove messages *from* this user.
        for queue in self.pending.values_mut() {
            queue.retain(|im| im.from_agent_id != user_id);
        }
        // Clean up empty queues.
        self.pending.retain(|_, v| !v.is_empty());
    }
}

impl Default for OfflineIMModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for OfflineIMModule {
    fn name(&self) -> &str {
        "OfflineIMModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name = config.get_string("Messaging", "OfflineMessageModule", "");
        self.enabled =
            module_name == self.name() || config.get_bool("Messaging", "OfflineIMEnabled", false);

        self.forward_offline_group_messages =
            config.get_bool("Messaging", "ForwardOfflineGroupMessages", true);

        self.max_offline_ims = config.get_int("Messaging", "MaxOfflineIMs", 25) as usize;

        tracing::info!(
            enabled = self.enabled,
            forward_group = self.forward_offline_group_messages,
            max = self.max_offline_ims,
            "OfflineIMModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        if !self.enabled {
            return Ok(());
        }
        self.scenes.push(scene.region_name.clone());
        tracing::info!(region = %scene.region_name, "OfflineIMModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(region = %scene.region_name, "OfflineIMModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
        }

        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }

        fn set_int(&mut self, section: &str, key: &str, value: i64) {
            self.ints
                .insert((section.to_string(), key.to_string()), value);
        }
    }

    impl crate::module::ModuleConfig for TestConfig {
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

    fn scene_ref() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    fn sample_im(to: Uuid, dialog: IMDialog) -> GridInstantMessage {
        GridInstantMessage {
            from_agent_id: Uuid::new_v4(),
            from_agent_name: "Sender".to_string(),
            to_agent_id: to,
            message: "Hello offline".to_string(),
            im_session_id: Uuid::new_v4(),
            dialog,
            from_group: false,
            offline: 0,
            parent_estate_id: 1,
            position: [128.0, 128.0, 25.0],
            region_id: Uuid::new_v4(),
            timestamp: 1700000000,
            binary_bucket: Vec::new(),
        }
    }

    // -- 1. Module initialises with config -----------------------------------

    #[test]
    fn initialise_reads_config() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        config.set_bool("Messaging", "ForwardOfflineGroupMessages", false);
        config.set_int("Messaging", "MaxOfflineIMs", 50);

        module.initialise(&config).unwrap();

        assert!(module.is_enabled());
        assert!(!module.forwards_group_messages());
        assert_eq!(module.max_offline_ims(), 50);
    }

    // -- 2. Module lifecycle: add/remove region ------------------------------

    #[test]
    fn lifecycle_add_remove_region() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scenes.len(), 1);

        module.remove_region(&scene).unwrap();
        assert!(module.scenes.is_empty());
    }

    // -- 3. Store and retrieve offline messages ------------------------------

    #[test]
    fn store_and_retrieve_messages() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        module.initialise(&config).unwrap();

        let recipient = Uuid::new_v4();
        let im = sample_im(recipient, IMDialog::MessageFromAgent);

        let stored = module.handle_undelivered(&im).unwrap();
        assert!(stored);
        assert_eq!(module.pending_count(recipient), 1);
        assert_eq!(module.total_pending(), 1);

        let msgs = module.retrieve_messages(recipient);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message, "Hello offline");
        assert_eq!(module.pending_count(recipient), 0);
    }

    // -- 4. Max offline IMs enforced -----------------------------------------

    #[test]
    fn max_offline_ims_enforced() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        config.set_int("Messaging", "MaxOfflineIMs", 2);
        module.initialise(&config).unwrap();

        let recipient = Uuid::new_v4();
        module
            .handle_undelivered(&sample_im(recipient, IMDialog::MessageFromAgent))
            .unwrap();
        module
            .handle_undelivered(&sample_im(recipient, IMDialog::MessageFromAgent))
            .unwrap();

        let result = module.handle_undelivered(&sample_im(recipient, IMDialog::MessageFromAgent));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maxed out"));
    }

    // -- 5. Non-storable dialog types are ignored ----------------------------

    #[test]
    fn non_storable_dialog_ignored() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        module.initialise(&config).unwrap();

        let recipient = Uuid::new_v4();
        // StartTyping should not be stored.
        let result = module
            .handle_undelivered(&sample_im(recipient, IMDialog::StartTyping))
            .unwrap();
        assert!(!result);
        assert_eq!(module.pending_count(recipient), 0);
    }

    // -- 6. Group messages respect forwarding flag ---------------------------

    #[test]
    fn group_messages_not_forwarded_when_disabled() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        config.set_bool("Messaging", "ForwardOfflineGroupMessages", false);
        module.initialise(&config).unwrap();

        let recipient = Uuid::new_v4();
        let result = module
            .handle_undelivered(&sample_im(recipient, IMDialog::GroupNotice))
            .unwrap();
        assert!(!result);

        let result2 = module
            .handle_undelivered(&sample_im(recipient, IMDialog::GroupInvitation))
            .unwrap();
        assert!(!result2);
    }

    // -- 7. Group messages forwarded when enabled ----------------------------

    #[test]
    fn group_messages_forwarded_when_enabled() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        config.set_bool("Messaging", "ForwardOfflineGroupMessages", true);
        module.initialise(&config).unwrap();

        let recipient = Uuid::new_v4();
        let result = module
            .handle_undelivered(&sample_im(recipient, IMDialog::GroupNotice))
            .unwrap();
        assert!(result);
        assert_eq!(module.pending_count(recipient), 1);
    }

    // -- 8. Delete messages removes both recipient and sender ----------------

    #[test]
    fn delete_messages_both_directions() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        module.initialise(&config).unwrap();

        let user_a = Uuid::new_v4();
        let user_b = Uuid::new_v4();

        // user_a sends to user_b
        let mut im = sample_im(user_b, IMDialog::MessageFromAgent);
        im.from_agent_id = user_a;
        module.handle_undelivered(&im).unwrap();

        // some other user sends to user_a
        module
            .handle_undelivered(&sample_im(user_a, IMDialog::MessageFromAgent))
            .unwrap();

        assert_eq!(module.total_pending(), 2);

        module.delete_messages(user_a);

        // Messages TO user_a are gone.
        assert_eq!(module.pending_count(user_a), 0);
        // Messages FROM user_a (to user_b) are also gone.
        assert_eq!(module.pending_count(user_b), 0);
    }

    // -- 9. Close clears all state -------------------------------------------

    #[test]
    fn close_clears_state() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Messaging", "OfflineIMEnabled", true);
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        module
            .handle_undelivered(&sample_im(Uuid::new_v4(), IMDialog::MessageFromAgent))
            .unwrap();

        module.close().unwrap();
        assert!(module.scenes.is_empty());
        assert_eq!(module.total_pending(), 0);
    }

    // -- 10. Module disabled rejects messages --------------------------------

    #[test]
    fn disabled_module_rejects() {
        let mut module = OfflineIMModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.is_enabled());

        let result =
            module.handle_undelivered(&sample_im(Uuid::new_v4(), IMDialog::MessageFromAgent));
        assert!(result.is_err());
    }

    // -- 11. Enable by module name -------------------------------------------

    #[test]
    fn enable_by_module_name() {
        let mut module = OfflineIMModule::new();
        let mut config = TestConfig::new();
        config.set_string("Messaging", "OfflineMessageModule", "OfflineIMModule");
        module.initialise(&config).unwrap();
        assert!(module.is_enabled());
    }
}
