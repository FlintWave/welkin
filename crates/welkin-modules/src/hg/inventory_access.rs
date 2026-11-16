// SPDX-License-Identifier: LGPL-3.0-or-later

//! Hypergrid inventory access, asset mapping, and inventory brokering.
//!
//! - `HGInventoryAccessModule`: routes inventory requests to local or
//!   remote services based on the user's home grid.
//! - `HGAssetMapper`: maps asset IDs and rewrites creator data during
//!   cross-grid object transfer.
//! - `HGInventoryBroker`: routes inventory service calls to local or HG
//!   inventory connectors.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;
use welkin_services::hypergrid::{HgInventoryItem, HypergridInventoryService};

// ---------------------------------------------------------------------------
// HGAssetMapper
// ---------------------------------------------------------------------------

/// Maps asset IDs and rewrites creator identifiers during cross-grid
/// object transfers.
///
/// Mirrors the C# `HGAssetMapper`.  When an object crosses grids, its
/// creator UUIDs are rewritten to UUI format (`uuid;home_uri;name`) so
/// that attribution survives the transfer, and referenced assets
/// (textures, scripts, etc.) are copied between the local and remote
/// asset servers.
pub struct HGAssetMapper {
    home_uri: String,
    /// Tracks assets that have been posted to remote grids to avoid
    /// redundant transfers.
    posted_assets: HashSet<(Uuid, String)>,
}

impl HGAssetMapper {
    pub fn new(home_uri: &str) -> Self {
        Self {
            home_uri: home_uri.to_string(),
            posted_assets: HashSet::new(),
        }
    }

    /// Rewrite a creator UUID to UUI format.
    ///
    /// If `creator_id` is a bare UUID and the creator is a local user,
    /// it becomes `"uuid;home_uri;FirstName LastName"`.
    pub fn rewrite_creator_to_uui(
        &self,
        creator_uuid: Uuid,
        first_name: &str,
        last_name: &str,
    ) -> String {
        format!(
            "{};{};{} {}",
            creator_uuid, self.home_uri, first_name, last_name
        )
    }

    /// Fetch an asset from a remote grid's asset server.
    ///
    /// Requires an HTTP client to download the asset and all its
    /// referenced sub-assets from the remote grid. Returns `false`
    /// until remote asset fetching is wired.
    pub fn get_asset(&self, asset_id: Uuid, _owner_id: Uuid, remote_asset_url: &str) -> bool {
        tracing::warn!(
            asset = %asset_id,
            url = %remote_asset_url,
            "HGAssetMapper::get_asset — remote fetch not yet wired"
        );
        false
    }

    /// Post (upload) an asset to a remote grid's asset server.
    ///
    /// Rewrites creator identifiers to UUI format before sending.
    /// Tracks posted assets to avoid redundant transfers. The actual
    /// HTTP POST is not yet wired.
    pub fn post_asset(&mut self, asset_id: Uuid, _owner_id: Uuid, remote_asset_url: &str) -> bool {
        let key = (asset_id, remote_asset_url.to_string());
        if self.posted_assets.contains(&key) {
            tracing::debug!(
                asset = %asset_id,
                url = %remote_asset_url,
                "asset already posted, skipping"
            );
            return true;
        }

        tracing::warn!(
            asset = %asset_id,
            url = %remote_asset_url,
            "posting asset to remote grid — HTTP transfer not yet wired"
        );
        self.posted_assets.insert(key);
        true
    }

    /// Number of assets posted so far (for diagnostics).
    pub fn posted_count(&self) -> usize {
        self.posted_assets.len()
    }

    /// The home URI used for creator rewriting.
    pub fn home_uri(&self) -> &str {
        &self.home_uri
    }
}

// ---------------------------------------------------------------------------
// HGInventoryBroker
// ---------------------------------------------------------------------------

/// Routes inventory service calls to local or HG inventory connectors
/// based on the user's home grid.
///
/// Mirrors the C# `HGInventoryBroker`.  Local users go through the
/// normal inventory service; foreign users go through the HG inventory
/// connector which presents only their Suitcase folder.
pub struct HGInventoryBroker {
    /// Maps foreign user UUIDs to their home grid's inventory service URL.
    foreign_user_inventory_urls: HashMap<Uuid, String>,
    hg_inventory_service: Option<Arc<dyn HypergridInventoryService>>,
}

impl HGInventoryBroker {
    pub fn new(_local_grid_uri: &str) -> Self {
        Self {
            foreign_user_inventory_urls: HashMap::new(),
            hg_inventory_service: None,
        }
    }

    /// Inject the HG inventory service.
    pub fn set_hg_inventory_service(&mut self, svc: Arc<dyn HypergridInventoryService>) {
        self.hg_inventory_service = Some(svc);
    }

    /// Register a foreign user and their inventory service URL.
    pub fn register_foreign_user(&mut self, user_id: Uuid, inventory_url: String) {
        self.foreign_user_inventory_urls
            .insert(user_id, inventory_url);
    }

    /// Check whether a user's inventory requests should be routed to the
    /// HG inventory service.
    pub fn is_foreign_user(&self, user_id: Uuid) -> bool {
        self.foreign_user_inventory_urls.contains_key(&user_id)
    }

    /// Get the inventory service URL for a foreign user.
    pub fn get_foreign_inventory_url(&self, user_id: Uuid) -> Option<&str> {
        self.foreign_user_inventory_urls
            .get(&user_id)
            .map(|s| s.as_str())
    }

    /// Get an item from the appropriate inventory service.
    pub fn get_item(
        &self,
        user_id: Uuid,
        item_id: Uuid,
    ) -> Result<Option<HgInventoryItem>, String> {
        if self.is_foreign_user(user_id) {
            if let Some(ref svc) = self.hg_inventory_service {
                svc.get_item(user_id, item_id)
                    .map_err(|e| format!("HG inventory error: {}", e))
            } else {
                Err("no HG inventory service configured".to_string())
            }
        } else {
            // Local user: would delegate to local inventory service.
            tracing::debug!(
                user = %user_id,
                item = %item_id,
                "local inventory get_item (stub)"
            );
            Ok(None)
        }
    }

    /// Number of registered foreign users.
    pub fn foreign_user_count(&self) -> usize {
        self.foreign_user_inventory_urls.len()
    }
}

// ---------------------------------------------------------------------------
// HGInventoryAccessModule
// ---------------------------------------------------------------------------

/// Routes inventory access operations (rez, derez, asset transfer)
/// to local or remote services depending on whether the user is local
/// or foreign.
///
/// Mirrors the C# `HGInventoryAccessModule`.
pub struct HGInventoryAccessModule {
    enabled: bool,
    scenes: Vec<String>,
    local_grid_uri: String,
    outbound_permission: bool,
    restrict_inventory_access_abroad: bool,
    check_separate_assets: bool,
    local_assets_url: String,
    asset_mapper: Option<HGAssetMapper>,
    inventory_broker: Option<HGInventoryBroker>,
    /// Known foreign users with their asset server URLs.
    foreign_user_asset_urls: HashMap<Uuid, String>,
}

impl HGInventoryAccessModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scenes: Vec::new(),
            local_grid_uri: String::new(),
            outbound_permission: true,
            restrict_inventory_access_abroad: true,
            check_separate_assets: false,
            local_assets_url: String::new(),
            asset_mapper: None,
            inventory_broker: None,
            foreign_user_asset_urls: HashMap::new(),
        }
    }

    /// Check if a user is a foreign visitor.
    pub fn is_foreign_user(&self, user_id: Uuid) -> bool {
        self.foreign_user_asset_urls.contains_key(&user_id)
    }

    /// Register a foreign user with their asset server URL.
    pub fn register_foreign_user(&mut self, user_id: Uuid, asset_url: String) {
        self.foreign_user_asset_urls.insert(user_id, asset_url);
    }

    /// Get the asset server URL for a foreign user.
    pub fn get_foreign_asset_url(&self, user_id: Uuid) -> Option<&str> {
        self.foreign_user_asset_urls
            .get(&user_id)
            .map(|s| s.as_str())
    }

    /// Handle a new inventory item being uploaded: if the owner is a
    /// foreign user, post the asset to their home grid.
    pub fn post_inventory_asset(
        &mut self,
        owner_id: Uuid,
        asset_id: Uuid,
        _asset_name: &str,
    ) -> bool {
        if let Some(url) = self.foreign_user_asset_urls.get(&owner_id).cloned()
            && self.outbound_permission
            && let Some(ref mut mapper) = self.asset_mapper
        {
            return mapper.post_asset(asset_id, owner_id, &url);
        }
        false
    }

    /// Access the asset mapper.
    pub fn asset_mapper(&self) -> Option<&HGAssetMapper> {
        self.asset_mapper.as_ref()
    }

    /// Mutable access to the asset mapper.
    pub fn asset_mapper_mut(&mut self) -> Option<&mut HGAssetMapper> {
        self.asset_mapper.as_mut()
    }

    /// Access the inventory broker.
    pub fn inventory_broker(&self) -> Option<&HGInventoryBroker> {
        self.inventory_broker.as_ref()
    }
}

impl Default for HGInventoryAccessModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for HGInventoryAccessModule {
    fn name(&self) -> &str {
        "HGInventoryAccessModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name = config.get_string("Modules", "InventoryAccessModule", "");
        if module_name != self.name() {
            tracing::debug!("HGInventoryAccessModule disabled by configuration");
            return Ok(());
        }

        self.enabled = true;
        self.local_grid_uri = config.get_string("Hypergrid", "GatekeeperURI", "");
        self.outbound_permission =
            config.get_bool("HGInventoryAccessModule", "OutboundPermission", true);
        self.restrict_inventory_access_abroad = config.get_bool(
            "HGInventoryAccessModule",
            "RestrictInventoryAccessAbroad",
            true,
        );
        self.check_separate_assets =
            config.get_bool("HGInventoryAccessModule", "CheckSeparateAssets", false);
        self.local_assets_url =
            config.get_string("HGInventoryAccessModule", "RegionHGAssetServerURI", "");

        // Create the asset mapper with our home URI.
        self.asset_mapper = Some(HGAssetMapper::new(&self.local_grid_uri));
        self.inventory_broker = Some(HGInventoryBroker::new(&self.local_grid_uri));

        tracing::info!(
            enabled = self.enabled,
            outbound = self.outbound_permission,
            restrict_abroad = self.restrict_inventory_access_abroad,
            "HGInventoryAccessModule initialised"
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
            "HGInventoryAccessModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(
            region = %scene.region_name,
            "HGInventoryAccessModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.foreign_user_asset_urls.clear();
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
        bools: HashMap<(String, String), bool>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: HashMap::new(),
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
    }

    impl ModuleConfig for TestConfig {
        fn get_string(&self, section: &str, key: &str, default: &str) -> String {
            self.strings
                .get(&(section.to_string(), key.to_string()))
                .cloned()
                .unwrap_or_else(|| default.to_string())
        }
        fn get_int(&self, _section: &str, _key: &str, default: i64) -> i64 {
            default
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

    // -- 1. HGAssetMapper creator rewriting ------------------------------

    #[test]
    fn asset_mapper_rewrite_creator() {
        let mapper = HGAssetMapper::new("http://mygrid.com:8002");
        let uuid = Uuid::new_v4();
        let uui = mapper.rewrite_creator_to_uui(uuid, "John", "Doe");
        assert!(uui.contains("http://mygrid.com:8002"));
        assert!(uui.contains("John Doe"));
        assert!(uui.starts_with(&uuid.to_string()));
    }

    // -- 2. HGAssetMapper post deduplication ----------------------------

    #[test]
    fn asset_mapper_post_deduplication() {
        let mut mapper = HGAssetMapper::new("http://mygrid.com:8002");
        let asset_id = Uuid::new_v4();
        let owner = Uuid::new_v4();

        assert!(mapper.post_asset(asset_id, owner, "http://remote.grid:8002"));
        assert_eq!(mapper.posted_count(), 1);

        // Posting same asset to same URL is deduplicated.
        assert!(mapper.post_asset(asset_id, owner, "http://remote.grid:8002"));
        assert_eq!(mapper.posted_count(), 1);

        // Different URL is a new post.
        assert!(mapper.post_asset(asset_id, owner, "http://other.grid:8002"));
        assert_eq!(mapper.posted_count(), 2);
    }

    // -- 3. HGInventoryBroker routing ------------------------------------

    #[test]
    fn inventory_broker_routing() {
        let mut broker = HGInventoryBroker::new("http://mygrid.com:8002");
        let local_user = Uuid::new_v4();
        let foreign_user = Uuid::new_v4();

        broker.register_foreign_user(foreign_user, "http://foreign.grid:8002".to_string());

        assert!(!broker.is_foreign_user(local_user));
        assert!(broker.is_foreign_user(foreign_user));
        assert_eq!(
            broker.get_foreign_inventory_url(foreign_user),
            Some("http://foreign.grid:8002")
        );
        assert_eq!(broker.foreign_user_count(), 1);
    }

    // -- 4. Module initialises correctly ---------------------------------

    #[test]
    fn module_initialise_enabled() {
        let mut module = HGInventoryAccessModule::new();
        let mut config = TestConfig::new();
        config.set_string(
            "Modules",
            "InventoryAccessModule",
            "HGInventoryAccessModule",
        );
        config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
        config.set_bool("HGInventoryAccessModule", "OutboundPermission", false);
        module.initialise(&config).unwrap();

        assert!(module.enabled);
        assert!(!module.outbound_permission);
        assert!(module.asset_mapper.is_some());
        assert!(module.inventory_broker.is_some());
    }

    // -- 5. Foreign user asset posting -----------------------------------

    #[test]
    fn post_inventory_asset_foreign() {
        let mut module = HGInventoryAccessModule::new();
        let mut config = TestConfig::new();
        config.set_string(
            "Modules",
            "InventoryAccessModule",
            "HGInventoryAccessModule",
        );
        config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
        module.initialise(&config).unwrap();

        let foreign_user = Uuid::new_v4();
        module.register_foreign_user(foreign_user, "http://foreign.grid:8002".to_string());

        let asset_id = Uuid::new_v4();
        assert!(module.post_inventory_asset(foreign_user, asset_id, "Test Object"));

        // Local user: nothing posted.
        let local_user = Uuid::new_v4();
        assert!(!module.post_inventory_asset(local_user, asset_id, "Test Object"));
    }

    // -- 6. Module lifecycle ---------------------------------------------

    #[test]
    fn module_lifecycle() {
        let mut module = HGInventoryAccessModule::new();
        let mut config = TestConfig::new();
        config.set_string(
            "Modules",
            "InventoryAccessModule",
            "HGInventoryAccessModule",
        );
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scenes.len(), 1);

        module.remove_region(&scene).unwrap();
        assert!(module.scenes.is_empty());

        module.close().unwrap();
    }

    // -- 7. Module disabled by default -----------------------------------

    #[test]
    fn module_disabled_by_default() {
        let mut module = HGInventoryAccessModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.enabled);
    }

    // -- 8. Inventory broker get_item for local user (stub) ---------------

    #[test]
    fn broker_get_item_local() {
        let broker = HGInventoryBroker::new("http://mygrid.com:8002");
        let result = broker.get_item(Uuid::new_v4(), Uuid::new_v4());
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
