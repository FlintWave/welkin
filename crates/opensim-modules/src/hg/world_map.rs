// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid world-map module: map blocks for hyperlinked regions and
//! map-server-URL injection for foreign visitors.
//!
//! Mirrors the C# `HGWorldMapModule` which extends `WorldMapModule` with
//! Hypergrid-specific behaviour: tracking which map blocks each client has
//! seen, resetting them on disconnect, and injecting the map-image server
//! URL for foreign users via SimulatorFeatures.

use std::collections::HashMap;

use uuid::Uuid;

use super::*;
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Map block types
// ---------------------------------------------------------------------------

/// A map block representing a hyperlinked region visible on the world map.
#[derive(Debug, Clone, PartialEq)]
pub struct HGMapBlock {
    pub x: u32,
    pub y: u32,
    pub name: String,
    pub access: u8,
    pub size_x: u32,
    pub size_y: u32,
    pub agents: u32,
}

impl HGMapBlock {
    pub fn new(x: u32, y: u32, name: &str) -> Self {
        Self {
            x,
            y,
            name: name.to_string(),
            access: 13, // PG
            size_x: 256,
            size_y: 256,
            agents: 0,
        }
    }

    /// Create a "down" block used to reset map tiles on disconnect.
    pub fn as_down(&self) -> Self {
        Self {
            x: self.x,
            y: self.y,
            name: String::new(),
            access: 254, // Down
            size_x: self.size_x,
            size_y: self.size_y,
            agents: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// HGWorldMapModule
// ---------------------------------------------------------------------------

/// Hypergrid-aware world map module.
///
/// Mirrors the C# `HGWorldMapModule`.  Extends the base world map module
/// by:
/// - Tracking which hyperlinked map blocks each connected client has seen.
/// - Resetting those blocks (marking them as "Down") when the client
///   disconnects, so stale hyperlinks don't linger on the map.
/// - Injecting the `map-server-url` into SimulatorFeatures for foreign
///   visitors so their viewer can load map tiles from the correct server.
pub struct HGWorldMapModule {
    enabled: bool,
    scenes: Vec<String>,
    map_image_server_url: String,
    local_grid_uri: String,

    /// Map blocks seen by each connected client, keyed by client UUID.
    seen_blocks: HashMap<Uuid, Vec<HGMapBlock>>,
}

impl HGWorldMapModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scenes: Vec::new(),
            map_image_server_url: String::new(),
            local_grid_uri: String::new(),
            seen_blocks: HashMap::new(),
        }
    }

    /// Record a set of map blocks as seen by a client.
    ///
    /// New blocks are merged with previously seen blocks (deduplicated by
    /// coordinates).
    pub fn record_seen_blocks(&mut self, client_id: Uuid, blocks: &[HGMapBlock]) {
        let entry = self.seen_blocks.entry(client_id).or_default();
        for block in blocks {
            if !entry.iter().any(|b| b.x == block.x && b.y == block.y) {
                entry.push(block.clone());
            }
        }
    }

    /// Get the "down" blocks that should be sent to a client on disconnect
    /// to reset the map tiles they had seen.
    ///
    /// Mirrors the C# `EventManager_OnClientClosed`.
    pub fn get_reset_blocks(&mut self, client_id: Uuid) -> Vec<HGMapBlock> {
        match self.seen_blocks.remove(&client_id) {
            Some(blocks) => blocks.iter().map(|b| b.as_down()).collect(),
            None => Vec::new(),
        }
    }

    /// Check whether a user is a foreign visitor (for map-server-URL
    /// injection).
    pub fn should_inject_map_url(&self, user_home_uri: &str) -> bool {
        if self.map_image_server_url.is_empty() {
            return false;
        }
        if user_home_uri.is_empty() {
            return false;
        }
        !is_same_grid(user_home_uri, &self.local_grid_uri)
    }

    /// Get the map image server URL for injection into SimulatorFeatures.
    pub fn map_image_server_url(&self) -> &str {
        &self.map_image_server_url
    }

    /// Number of clients with seen-block records.
    pub fn tracked_client_count(&self) -> usize {
        self.seen_blocks.len()
    }

    /// Total number of seen blocks across all clients.
    pub fn total_seen_blocks(&self) -> usize {
        self.seen_blocks.values().map(|v| v.len()).sum()
    }
}

impl Default for HGWorldMapModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for HGWorldMapModule {
    fn name(&self) -> &str {
        "HGWorldMapModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name = config.get_string("Map", "WorldMapModule", "WorldMap");
        if module_name != "HGWorldMap" {
            tracing::debug!("HGWorldMapModule disabled by configuration");
            return Ok(());
        }

        self.enabled = true;
        self.local_grid_uri = config.get_string("Hypergrid", "GatekeeperURI", "");

        let mut url = config.get_string("HGWorldMap", "MapTileURL", "");
        if !url.is_empty() {
            url = url.trim().to_string();
            if !url.ends_with('/') {
                url.push('/');
            }
        }
        self.map_image_server_url = url;

        tracing::info!(
            enabled = self.enabled,
            map_url = %self.map_image_server_url,
            "HGWorldMapModule initialised"
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
            "HGWorldMapModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        self.seen_blocks.clear();
        tracing::info!(
            region = %scene.region_name,
            "HGWorldMapModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.seen_blocks.clear();
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
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: HashMap::new(),
            }
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
        fn get_int(&self, _section: &str, _key: &str, default: i64) -> i64 {
            default
        }
        fn get_bool(&self, _section: &str, _key: &str, default: bool) -> bool {
            default
        }
        fn get_float(&self, _section: &str, _key: &str, default: f64) -> f64 {
            default
        }
    }

    fn scene_ref() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    // -- 1. Module initialises when configured --------------------------------

    #[test]
    fn module_initialise_enabled() {
        let mut module = HGWorldMapModule::new();
        let mut config = TestConfig::new();
        config.set_string("Map", "WorldMapModule", "HGWorldMap");
        config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
        config.set_string("HGWorldMap", "MapTileURL", "http://mygrid.com:8002/map");
        module.initialise(&config).unwrap();
        assert!(module.enabled);
        assert_eq!(module.map_image_server_url, "http://mygrid.com:8002/map/");
    }

    // -- 2. Module disabled by default ----------------------------------------

    #[test]
    fn module_disabled_by_default() {
        let mut module = HGWorldMapModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.enabled);
    }

    // -- 3. Record and reset seen blocks --------------------------------------

    #[test]
    fn record_and_reset_seen_blocks() {
        let mut module = HGWorldMapModule::new();
        let client = Uuid::new_v4();

        let blocks = vec![
            HGMapBlock::new(1000, 1000, "Region A"),
            HGMapBlock::new(1001, 1000, "Region B"),
        ];
        module.record_seen_blocks(client, &blocks);
        assert_eq!(module.total_seen_blocks(), 2);
        assert_eq!(module.tracked_client_count(), 1);

        let reset = module.get_reset_blocks(client);
        assert_eq!(reset.len(), 2);
        assert!(reset.iter().all(|b| b.access == 254)); // Down
        assert!(reset.iter().all(|b| b.name.is_empty()));
        assert_eq!(module.tracked_client_count(), 0);
    }

    // -- 4. Deduplicate seen blocks -------------------------------------------

    #[test]
    fn dedup_seen_blocks() {
        let mut module = HGWorldMapModule::new();
        let client = Uuid::new_v4();

        let blocks = vec![HGMapBlock::new(1000, 1000, "Region A")];
        module.record_seen_blocks(client, &blocks);
        module.record_seen_blocks(client, &blocks); // duplicate
        assert_eq!(module.total_seen_blocks(), 1);
    }

    // -- 5. Should inject map URL for foreign users ---------------------------

    #[test]
    fn should_inject_map_url() {
        let mut module = HGWorldMapModule::new();
        module.local_grid_uri = "http://mygrid.com:8002".to_string();
        module.map_image_server_url = "http://mygrid.com:8002/map/".to_string();

        // Foreign user should get map URL.
        assert!(module.should_inject_map_url("http://othergrid.com:8002"));

        // Local user should not.
        assert!(!module.should_inject_map_url("http://mygrid.com:8002"));

        // Empty home URI should not.
        assert!(!module.should_inject_map_url(""));
    }

    // -- 6. Reset blocks for unknown client returns empty ---------------------

    #[test]
    fn reset_blocks_unknown_client() {
        let mut module = HGWorldMapModule::new();
        let result = module.get_reset_blocks(Uuid::new_v4());
        assert!(result.is_empty());
    }

    // -- 7. Module lifecycle --------------------------------------------------

    #[test]
    fn module_lifecycle() {
        let mut module = HGWorldMapModule::new();
        let mut config = TestConfig::new();
        config.set_string("Map", "WorldMapModule", "HGWorldMap");
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scenes.len(), 1);

        // Add some blocks, then remove_region clears them.
        module.record_seen_blocks(Uuid::new_v4(), &[HGMapBlock::new(0, 0, "Test")]);
        module.remove_region(&scene).unwrap();
        assert!(module.scenes.is_empty());
        assert_eq!(module.tracked_client_count(), 0);

        module.close().unwrap();
    }

    // -- 8. Map URL trailing slash handling ------------------------------------

    #[test]
    fn map_url_trailing_slash() {
        let mut module = HGWorldMapModule::new();
        let mut config = TestConfig::new();
        config.set_string("Map", "WorldMapModule", "HGWorldMap");
        config.set_string("HGWorldMap", "MapTileURL", "http://tiles.example.com/map");
        module.initialise(&config).unwrap();
        assert!(module.map_image_server_url.ends_with('/'));

        // Already has trailing slash.
        let mut module2 = HGWorldMapModule::new();
        let mut config2 = TestConfig::new();
        config2.set_string("Map", "WorldMapModule", "HGWorldMap");
        config2.set_string("HGWorldMap", "MapTileURL", "http://tiles.example.com/map/");
        module2.initialise(&config2).unwrap();
        assert_eq!(module2.map_image_server_url, "http://tiles.example.com/map/");
    }
}
