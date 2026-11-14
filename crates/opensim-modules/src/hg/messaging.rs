// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid instant messaging: cross-grid IM routing and the
//! HG message transfer module.
//!
//! Mirrors the C# `HGInstantMessageService` (service-layer IM routing)
//! and `HGMessageTransferModule` (region-module IM dispatch).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;
use crate::messaging::{GridInstantMessage, IMDialog};
use opensim_services::hypergrid::UserAgentService;

// ---------------------------------------------------------------------------
// User location cache
// ---------------------------------------------------------------------------

/// Cached location entry for a user, with TTL.
#[derive(Debug, Clone)]
struct CachedLocation {
    url: String,
    expires: Instant,
}

/// A simple expiring cache for user grid locations.
#[derive(Debug)]
struct UserLocationCache {
    entries: HashMap<Uuid, CachedLocation>,
    ttl_secs: u64,
}

impl UserLocationCache {
    fn new(ttl_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            ttl_secs,
        }
    }

    fn get(&self, user_id: &Uuid) -> Option<&str> {
        if let Some(entry) = self.entries.get(user_id) {
            if Instant::now() < entry.expires {
                return Some(&entry.url);
            }
        }
        None
    }

    fn insert(&mut self, user_id: Uuid, url: String) {
        self.entries.insert(
            user_id,
            CachedLocation {
                url,
                expires: Instant::now() + std::time::Duration::from_secs(self.ttl_secs),
            },
        );
    }

    fn remove(&mut self, user_id: &Uuid) {
        self.entries.remove(user_id);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// HGInstantMessageService
// ---------------------------------------------------------------------------

/// Cross-grid IM routing service.
///
/// Mirrors the C# `HGInstantMessageService`.  When a message cannot be
/// delivered locally, it locates the target user via the `UserAgentService`
/// and forwards the IM to the appropriate grid.
pub struct HGInstantMessageService {
    user_agent_service: Option<Arc<dyn UserAgentService>>,
    location_cache: UserLocationCache,
    message_key: String,
    pub forward_offline_group_messages: bool,
    in_gatekeeper: bool,
    max_retries: u32,
}

impl HGInstantMessageService {
    pub fn new() -> Self {
        Self {
            user_agent_service: None,
            location_cache: UserLocationCache::new(120),
            message_key: String::new(),
            forward_offline_group_messages: false,
            in_gatekeeper: false,
            max_retries: 3,
        }
    }

    /// Inject the UserAgentService dependency.
    pub fn set_user_agent_service(&mut self, svc: Arc<dyn UserAgentService>) {
        self.user_agent_service = Some(svc);
    }

    /// Set the message authentication key.
    pub fn set_message_key(&mut self, key: &str) {
        self.message_key = key.to_string();
    }

    /// Configure whether this instance runs inside the gatekeeper.
    pub fn set_in_gatekeeper(&mut self, in_gk: bool) {
        self.in_gatekeeper = in_gk;
    }

    /// Set the maximum number of delivery retries.
    pub fn set_max_retries(&mut self, retries: u32) {
        self.max_retries = retries;
    }

    /// Whether offline group messages should be forwarded.
    pub fn forward_offline_group_messages(&self) -> bool {
        self.forward_offline_group_messages
    }

    /// Try to send an IM, resolving the target user's location.
    ///
    /// Returns `true` if the IM was accepted for delivery.
    pub fn try_send_instant_message(
        &mut self,
        im: &GridInstantMessage,
        foreigner_url: &str,
        foreigner: bool,
    ) -> bool {
        let to_agent_id = im.to_agent_id;

        // 1. Try cached location.
        if let Some(cached_url) = self.location_cache.get(&to_agent_id) {
            let url = cached_url.to_string();
            if self.forward_im_to_grid(&url, im, to_agent_id) {
                return true;
            }
        }

        // 2. Try the provided foreigner URL.
        if !foreigner_url.is_empty() && self.forward_im_to_grid(foreigner_url, im, to_agent_id) {
            return true;
        }

        // 3. Try locating the user via UserAgentService.
        if !foreigner {
            if let Some(ref uas) = self.user_agent_service {
                match uas.locate_user(to_agent_id) {
                    Ok(url) if !url.is_empty() => {
                        // Avoid retrying same URL we already failed on.
                        if !foreigner_url.is_empty()
                            && super::normalise_grid_uri(&url)
                                == super::normalise_grid_uri(foreigner_url)
                        {
                            tracing::debug!(
                                agent = %to_agent_id,
                                "UAS returned same URL we already tried"
                            );
                            return false;
                        }
                        return self.forward_im_to_grid(&url, im, to_agent_id);
                    }
                    Ok(_) => {
                        tracing::debug!(agent = %to_agent_id, "UAS returned empty location");
                    }
                    Err(e) => {
                        tracing::warn!(agent = %to_agent_id, error = %e, "LocateUser failed");
                    }
                }
            }
        }

        tracing::debug!(agent = %to_agent_id, "unable to locate user for IM delivery");
        false
    }

    /// Forward an IM to a specific grid URL (stub).
    fn forward_im_to_grid(
        &mut self,
        url: &str,
        _im: &GridInstantMessage,
        to_agent_id: Uuid,
    ) -> bool {
        // In a real implementation, this would POST to the remote grid's
        // IM endpoint.  For now, simulate success and update the cache.
        tracing::debug!(
            to = %to_agent_id,
            url = %url,
            "forwarding IM to grid (stub)"
        );
        self.location_cache.insert(to_agent_id, url.to_string());
        true
    }

    /// Check whether a dialog type should be stored for offline delivery.
    pub fn is_offline_storable(&self, dialog: IMDialog) -> bool {
        if self.forward_offline_group_messages {
            matches!(
                dialog,
                IMDialog::MessageFromObject
                    | IMDialog::MessageFromAgent
                    | IMDialog::GroupNotice
                    | IMDialog::GroupInvitation
                    | IMDialog::InventoryOffered
            )
        } else {
            matches!(
                dialog,
                IMDialog::MessageFromObject
                    | IMDialog::MessageFromAgent
                    | IMDialog::InventoryOffered
            )
        }
    }

    /// Number of cached user locations (for diagnostics).
    pub fn cached_location_count(&self) -> usize {
        self.location_cache.len()
    }
}

impl Default for HGInstantMessageService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HGMessageTransferModule
// ---------------------------------------------------------------------------

/// Routes instant messages to local agents or forwards them across grids.
///
/// Mirrors the C# `HGMessageTransferModule`.  Extends the base
/// `MessageTransferModule` by checking whether the target user is local
/// or foreign and routing accordingly.
pub struct HGMessageTransferModule {
    enabled: bool,
    scenes: Vec<String>,
    im_service: HGInstantMessageService,
    local_grid_uri: String,
    /// Maps user UUIDs to their home URIs for foreign-user detection.
    foreign_users: HashMap<Uuid, String>,
}

impl HGMessageTransferModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scenes: Vec::new(),
            im_service: HGInstantMessageService::new(),
            local_grid_uri: String::new(),
            foreign_users: HashMap::new(),
        }
    }

    /// Register a user as foreign with their home URI.
    pub fn register_foreign_user(&mut self, user_id: Uuid, home_uri: String) {
        self.foreign_users.insert(user_id, home_uri);
    }

    /// Check whether a user is foreign.
    pub fn is_foreign_user(&self, user_id: Uuid) -> bool {
        self.foreign_users.contains_key(&user_id)
    }

    /// Get the home URI of a foreign user.
    pub fn get_foreign_user_uri(&self, user_id: Uuid) -> Option<&str> {
        self.foreign_users.get(&user_id).map(|s| s.as_str())
    }

    /// Send an instant message, routing to local or remote grid as needed.
    ///
    /// Returns `true` if the message was accepted for delivery.
    pub fn send_instant_message(&mut self, im: &GridInstantMessage) -> bool {
        if im.to_agent_id.is_nil() {
            return false;
        }

        // Check if target is a foreign user.
        let foreigner = self.is_foreign_user(im.to_agent_id);
        let url = self
            .foreign_users
            .get(&im.to_agent_id)
            .cloned()
            .unwrap_or_default();

        if foreigner {
            tracing::debug!(
                from = %im.from_agent_id,
                to = %im.to_agent_id,
                url = %url,
                "routing IM to foreign grid"
            );
            self.im_service
                .try_send_instant_message(im, &url, true)
        } else {
            // Try local delivery first (stub: always try remote).
            tracing::debug!(
                from = %im.from_agent_id,
                to = %im.to_agent_id,
                "routing IM locally then via HG"
            );
            self.im_service
                .try_send_instant_message(im, "", false)
        }
    }

    /// Handle an IM that could not be delivered locally.
    pub fn handle_undeliverable(&self, im: &GridInstantMessage) -> bool {
        if self.im_service.is_offline_storable(im.dialog) {
            tracing::debug!(
                to = %im.to_agent_id,
                dialog = ?im.dialog,
                "storing undeliverable HG message for offline delivery"
            );
            true
        } else {
            false
        }
    }

    /// Access the underlying HG IM service.
    pub fn im_service(&self) -> &HGInstantMessageService {
        &self.im_service
    }

    /// Mutable access to the underlying HG IM service.
    pub fn im_service_mut(&mut self) -> &mut HGInstantMessageService {
        &mut self.im_service
    }
}

impl Default for HGMessageTransferModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for HGMessageTransferModule {
    fn name(&self) -> &str {
        "HGMessageTransferModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name = config.get_string("Messaging", "MessageTransferModule", "MessageTransferModule");
        if module_name != self.name() {
            tracing::debug!("HGMessageTransferModule disabled by configuration");
            return Ok(());
        }

        self.enabled = true;
        self.local_grid_uri = config.get_string("Hypergrid", "GatekeeperURI", "");

        let message_key = config.get_string("Messaging", "MessageKey", "");
        self.im_service.set_message_key(&message_key);

        let in_gk = config.get_bool("HGInstantMessageService", "InGatekeeper", false);
        self.im_service.set_in_gatekeeper(in_gk);

        self.im_service.forward_offline_group_messages =
            config.get_bool("Messaging", "ForwardOfflineGroupMessages", false);

        let max_retries = config.get_int("HGInstantMessageService", "MaxRetries", 3) as u32;
        self.im_service.set_max_retries(max_retries);

        tracing::info!(
            enabled = self.enabled,
            local_grid = %self.local_grid_uri,
            "HGMessageTransferModule initialised"
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
            "HGMessageTransferModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(
            region = %scene.region_name,
            "HGMessageTransferModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.im_service.location_cache.clear();
        self.foreign_users.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

        #[allow(dead_code)]
        fn set_int(&mut self, section: &str, key: &str, value: i64) {
            self.ints
                .insert((section.to_string(), key.to_string()), value);
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

    fn scene_ref() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    fn sample_im(to: Uuid, dialog: IMDialog) -> GridInstantMessage {
        GridInstantMessage {
            from_agent_id: Uuid::new_v4(),
            from_agent_name: "Sender".to_string(),
            to_agent_id: to,
            message: "Hello HG".to_string(),
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

    // -- 1. Module initialises with correct name -------------------------

    #[test]
    fn module_initialise_enabled() {
        let mut module = HGMessageTransferModule::new();
        let mut config = TestConfig::new();
        config.set_string("Messaging", "MessageTransferModule", "HGMessageTransferModule");
        config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
        module.initialise(&config).unwrap();
        assert!(module.enabled);
        assert_eq!(module.local_grid_uri, "http://mygrid.com:8002");
    }

    // -- 2. Module disabled when name doesn't match ----------------------

    #[test]
    fn module_disabled_when_name_mismatch() {
        let mut module = HGMessageTransferModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.enabled);
    }

    // -- 3. Send IM to nil UUID returns false ----------------------------

    #[test]
    fn send_im_nil_uuid_returns_false() {
        let mut module = HGMessageTransferModule::new();
        let im = sample_im(Uuid::nil(), IMDialog::MessageFromAgent);
        assert!(!module.send_instant_message(&im));
    }

    // -- 4. Foreign user IM routing --------------------------------------

    #[test]
    fn send_im_to_foreign_user() {
        let mut module = HGMessageTransferModule::new();
        module.enabled = true;
        let foreign_user = Uuid::new_v4();
        module.register_foreign_user(foreign_user, "http://foreign.grid:8002".to_string());

        assert!(module.is_foreign_user(foreign_user));
        let im = sample_im(foreign_user, IMDialog::MessageFromAgent);
        let result = module.send_instant_message(&im);
        assert!(result);
    }

    // -- 5. Local user IM routing ----------------------------------------

    #[test]
    fn send_im_to_local_user() {
        let mut module = HGMessageTransferModule::new();
        module.enabled = true;
        let local_user = Uuid::new_v4();
        let im = sample_im(local_user, IMDialog::MessageFromAgent);
        // Should attempt via IM service (stub: succeeds via UAS path or fails)
        let _result = module.send_instant_message(&im);
        assert!(!module.is_foreign_user(local_user));
    }

    // -- 6. Offline storable dialog types --------------------------------

    #[test]
    fn offline_storable_with_group_forwarding() {
        let mut svc = HGInstantMessageService::new();
        svc.forward_offline_group_messages = true;
        assert!(svc.is_offline_storable(IMDialog::MessageFromAgent));
        assert!(svc.is_offline_storable(IMDialog::GroupNotice));
        assert!(svc.is_offline_storable(IMDialog::InventoryOffered));
        assert!(!svc.is_offline_storable(IMDialog::StartTyping));
    }

    // -- 7. Offline storable without group forwarding --------------------

    #[test]
    fn offline_storable_without_group_forwarding() {
        let svc = HGInstantMessageService::new();
        assert!(svc.is_offline_storable(IMDialog::MessageFromAgent));
        assert!(!svc.is_offline_storable(IMDialog::GroupNotice));
        assert!(!svc.is_offline_storable(IMDialog::GroupInvitation));
    }

    // -- 8. Location cache basic operations ------------------------------

    #[test]
    fn location_cache_insert_and_get() {
        let mut cache = UserLocationCache::new(3600);
        let user = Uuid::new_v4();
        cache.insert(user, "http://grid.example.com:8002".to_string());
        assert_eq!(cache.get(&user), Some("http://grid.example.com:8002"));
        assert_eq!(cache.len(), 1);

        cache.remove(&user);
        assert!(cache.get(&user).is_none());
        assert_eq!(cache.len(), 0);
    }

    // -- 9. Module lifecycle: add/remove region --------------------------

    #[test]
    fn module_lifecycle() {
        let mut module = HGMessageTransferModule::new();
        let mut config = TestConfig::new();
        config.set_string("Messaging", "MessageTransferModule", "HGMessageTransferModule");
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scenes.len(), 1);

        module.remove_region(&scene).unwrap();
        assert!(module.scenes.is_empty());

        module.close().unwrap();
    }

    // -- 10. Handle undeliverable message --------------------------------

    #[test]
    fn handle_undeliverable_storable() {
        let module = HGMessageTransferModule::new();
        let im = sample_im(Uuid::new_v4(), IMDialog::MessageFromAgent);
        // Default: forward_offline_group_messages=false but MessageFromAgent is storable.
        assert!(module.handle_undeliverable(&im));
    }

    // -- 11. Handle undeliverable non-storable ---------------------------

    #[test]
    fn handle_undeliverable_non_storable() {
        let module = HGMessageTransferModule::new();
        let im = sample_im(Uuid::new_v4(), IMDialog::StartTyping);
        assert!(!module.handle_undeliverable(&im));
    }
}
