// SPDX-License-Identifier: GPL-3.0-or-later

//! World map module: map block requests, map item queries, agent location
//! broadcasting, and map name search.
//!
//! Mirrors the C# `WorldMapModule` and `MapSearchModule` from
//! `OpenSim.Region.CoreModules.World.WorldMap`.

pub mod map_image;

use std::collections::HashMap;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// SimAccess — region maturity rating
// ---------------------------------------------------------------------------

/// Region access / maturity rating, matching the Second Life viewer protocol
/// `SimAccess` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[derive(Default)]
pub enum SimAccess {
    /// Unknown or unset.
    Unknown = 0,
    /// Trial account region (historical).
    Trial = 7,
    /// PG — general audience.
    #[default]
    PG = 13,
    /// Mature — moderate content.
    Mature = 21,
    /// Adult — adult content.
    Adult = 42,
    /// Region is offline / down.
    Down = 254,
    /// Region does not exist (used as search-result terminator).
    NonExistent = 255,
}

impl SimAccess {
    /// Convert from a raw `u8`, returning [`SimAccess::Unknown`] for
    /// unrecognised values.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Unknown,
            7 => Self::Trial,
            13 => Self::PG,
            21 => Self::Mature,
            42 => Self::Adult,
            254 => Self::Down,
            255 => Self::NonExistent,
            _ => Self::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// MapBlock — one region's summary for the world map
// ---------------------------------------------------------------------------

/// Summary data for a single region on the world map, sent to viewers in
/// response to map block requests.
#[derive(Debug, Clone)]
pub struct MapBlock {
    /// Grid X coordinate (region units, not meters).
    pub x: u16,
    /// Grid Y coordinate (region units, not meters).
    pub y: u16,
    /// Human-readable region name.
    pub name: String,
    /// Region maturity / access level.
    pub access: SimAccess,
    /// UUID of the map tile image asset.
    pub map_image_id: Uuid,
    /// Number of agents currently in the region.
    pub agents: u8,
    /// Region size in X (meters).  Typically 256.
    pub size_x: u16,
    /// Region size in Y (meters).  Typically 256.
    pub size_y: u16,
    /// Region flags (informational).
    pub region_flags: u32,
}

impl MapBlock {
    /// Create a new `MapBlock` with the given coordinates and name, defaulting
    /// to PG access and a nil map image.
    pub fn new(x: u16, y: u16, name: impl Into<String>) -> Self {
        Self {
            x,
            y,
            name: name.into(),
            access: SimAccess::default(),
            map_image_id: Uuid::nil(),
            agents: 0,
            size_x: 256,
            size_y: 256,
            region_flags: 0,
        }
    }

    /// Build the special sentinel block that terminates a map search result
    /// in the viewer protocol.
    pub fn terminal_block(query_name: &str) -> Self {
        Self {
            x: 0,
            y: 0,
            name: query_name.to_string(),
            access: SimAccess::NonExistent,
            map_image_id: Uuid::nil(),
            agents: 0,
            size_x: 0,
            size_y: 0,
            region_flags: 0,
        }
    }
}

impl Default for MapBlock {
    fn default() -> Self {
        Self::new(0, 0, "")
    }
}

// ---------------------------------------------------------------------------
// MapItemType / MapItem — point-of-interest layer on the map
// ---------------------------------------------------------------------------

/// Types of map items (points of interest) that can appear on the world map.
///
/// Values match the LL viewer protocol constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum MapItemType {
    Telehub = 1,
    PGEvent = 2,
    MatureEvent = 3,
    AdultEvent = 4,
    PopularEvent = 5,
    AgentLocations = 6,
    LandForSale = 7,
    Classified = 8,
}

impl MapItemType {
    /// Convert from a raw `u32`, returning `None` for unrecognised values.
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::Telehub),
            2 => Some(Self::PGEvent),
            3 => Some(Self::MatureEvent),
            4 => Some(Self::AdultEvent),
            5 => Some(Self::PopularEvent),
            6 => Some(Self::AgentLocations),
            7 => Some(Self::LandForSale),
            8 => Some(Self::Classified),
            _ => None,
        }
    }
}

/// A single map item (point of interest) on the world map.
#[derive(Debug, Clone)]
pub struct MapItem {
    /// Global X position in the grid (meters).
    pub x: u32,
    /// Global Y position in the grid (meters).
    pub y: u32,
    /// Unique identifier for this item.
    pub id: Uuid,
    /// Display name.
    pub name: String,
    /// Extra data field (interpretation depends on item type).
    pub extra: i32,
    /// Second extra data field.
    pub extra2: i32,
    /// The type of map item.
    pub item_type: MapItemType,
}

impl MapItem {
    pub fn new(x: u32, y: u32, item_type: MapItemType, name: impl Into<String>) -> Self {
        Self {
            x,
            y,
            id: Uuid::new_v4(),
            name: name.into(),
            extra: 0,
            extra2: 0,
            item_type,
        }
    }
}

// ---------------------------------------------------------------------------
// WorldMapModule
// ---------------------------------------------------------------------------

/// Per-region world map module handling map block requests, map item queries,
/// and agent location broadcasting.
///
/// This is the Rust equivalent of C# `WorldMapModule` (INonSharedRegionModule).
pub struct WorldMapModule {
    pub enabled: bool,
    pub region_name: String,
    pub region_id: Uuid,
    pub region_grid_x: u16,
    pub region_grid_y: u16,
    pub region_size_x: u16,
    pub region_size_y: u16,
    pub map_image_id: Uuid,
    pub access: SimAccess,

    /// Cached map blocks for nearby regions.  Keyed by `(grid_x, grid_y)`.
    cached_blocks: HashMap<(u16, u16), MapBlock>,

    /// Map items registered in this region.
    map_items: Vec<MapItem>,

    /// Number of root agents (used for the agent count in map blocks).
    pub agent_count: u8,

    /// Blacklist timeout in seconds.
    pub blacklist_timeout: u32,
}

impl WorldMapModule {
    pub fn new() -> Self {
        Self {
            enabled: true,
            region_name: String::new(),
            region_id: Uuid::nil(),
            region_grid_x: 1000,
            region_grid_y: 1000,
            region_size_x: 256,
            region_size_y: 256,
            map_image_id: Uuid::nil(),
            access: SimAccess::PG,
            cached_blocks: HashMap::new(),
            map_items: Vec::new(),
            agent_count: 0,
            blacklist_timeout: 300,
        }
    }

    /// Return the [`MapBlock`] for this region.
    pub fn get_own_block(&self) -> MapBlock {
        MapBlock {
            x: self.region_grid_x,
            y: self.region_grid_y,
            name: self.region_name.clone(),
            access: self.access,
            map_image_id: self.map_image_id,
            agents: self.agent_count,
            size_x: self.region_size_x,
            size_y: self.region_size_y,
            region_flags: 0,
        }
    }

    /// Handle a map block request for regions inside a grid rectangle.
    ///
    /// Returns all cached [`MapBlock`]s whose coordinates fall within
    /// `[min_x..=max_x, min_y..=max_y]`.  Always includes this region's
    /// own block if it falls in range.
    pub fn handle_map_block_request(
        &self,
        min_x: u16,
        min_y: u16,
        max_x: u16,
        max_y: u16,
    ) -> Vec<MapBlock> {
        let mut results = Vec::new();

        // Include own region if in range.
        if self.region_grid_x >= min_x
            && self.region_grid_x <= max_x
            && self.region_grid_y >= min_y
            && self.region_grid_y <= max_y
        {
            results.push(self.get_own_block());
        }

        // Include cached neighbours.
        for (&(gx, gy), block) in &self.cached_blocks {
            if gx >= min_x && gx <= max_x && gy >= min_y && gy <= max_y {
                results.push(block.clone());
            }
        }

        results
    }

    /// Handle a map item request for items of a given type in this region.
    pub fn handle_map_item_request(&self, item_type: MapItemType) -> Vec<MapItem> {
        self.map_items
            .iter()
            .filter(|item| item.item_type == item_type)
            .cloned()
            .collect()
    }

    /// Register a map item in this region.
    pub fn add_map_item(&mut self, item: MapItem) {
        tracing::debug!(
            item_type = ?item.item_type,
            name = %item.name,
            "added map item"
        );
        self.map_items.push(item);
    }

    /// Remove all map items of a given type.
    pub fn remove_items_by_type(&mut self, item_type: MapItemType) {
        self.map_items.retain(|item| item.item_type != item_type);
    }

    /// Update the agent count for the map.  Stub — in a full implementation
    /// this would broadcast to neighbouring regions.
    pub fn update_agent_location(&mut self, agent_count: u8) {
        self.agent_count = agent_count;
        tracing::debug!(agent_count, "updated agent count for map (stub)");
    }

    /// Cache a neighbour region's map block.
    pub fn cache_block(&mut self, block: MapBlock) {
        self.cached_blocks.insert((block.x, block.y), block);
    }

    /// Set the map tile image UUID for this region.
    pub fn set_map_image(&mut self, image_id: Uuid) {
        self.map_image_id = image_id;
        tracing::debug!(%image_id, "map tile image set");
    }

    /// Return the number of cached neighbour blocks.
    pub fn cached_block_count(&self) -> usize {
        self.cached_blocks.len()
    }

    /// Return the number of registered map items.
    pub fn map_item_count(&self) -> usize {
        self.map_items.len()
    }
}

impl Default for WorldMapModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for WorldMapModule {
    fn name(&self) -> &str {
        "WorldMapModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name = config.get_string("Map", "WorldMapModule", "WorldMap");
        self.enabled = module_name == "WorldMap";
        self.blacklist_timeout = config.get_int("Map", "BlacklistTimeout", 300) as u32;
        tracing::info!(enabled = self.enabled, "WorldMapModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        if !self.enabled {
            return Ok(());
        }
        self.region_name = scene.region_name.clone();
        self.region_id = scene.region_id;
        tracing::info!(region = %scene.region_name, "WorldMapModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.cached_blocks.clear();
        self.map_items.clear();
        self.agent_count = 0;
        tracing::info!(region = %scene.region_name, "WorldMapModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.cached_blocks.clear();
        self.map_items.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MapSearchModule
// ---------------------------------------------------------------------------

/// Shared module providing map name search across regions.
///
/// Mirrors the C# `MapSearchModule` (ISharedRegionModule).  In a full grid
/// deployment this would query the grid service; here it searches locally
/// registered region names.
pub struct MapSearchModule {
    /// Known regions indexed by name (lowercased) for search.
    regions: HashMap<String, MapBlock>,
    /// Minimum query length to avoid over-broad searches.
    min_query_length: usize,
}

impl MapSearchModule {
    pub fn new() -> Self {
        Self {
            regions: HashMap::new(),
            min_query_length: 3,
        }
    }

    /// Register a region for search.
    pub fn register_region(&mut self, block: MapBlock) {
        let key = block.name.to_lowercase();
        self.regions.insert(key, block);
    }

    /// Unregister a region by name.
    pub fn unregister_region(&mut self, name: &str) {
        self.regions.remove(&name.to_lowercase());
    }

    /// Search for regions whose name contains `query` (case-insensitive).
    ///
    /// Returns up to `max_results` matching [`MapBlock`]s, plus a terminal
    /// sentinel block.  Returns an error for queries shorter than the
    /// minimum length (3 characters by default).
    pub fn search_map(&self, query: &str, max_results: usize) -> Result<Vec<MapBlock>, String> {
        let trimmed = query.trim().trim_end_matches('#');
        if trimmed.len() < self.min_query_length {
            return Err(format!(
                "Use a search string with at least {} characters",
                self.min_query_length
            ));
        }

        let needle = trimmed.to_lowercase();
        let mut results: Vec<MapBlock> = self
            .regions
            .values()
            .filter(|block| block.name.to_lowercase().contains(&needle))
            .take(max_results)
            .cloned()
            .collect();

        // Append the terminal sentinel.
        results.push(MapBlock::terminal_block(query));

        Ok(results)
    }

    /// Number of registered regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }
}

impl Default for MapSearchModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for MapSearchModule {
    fn name(&self) -> &str {
        "MapSearchModule"
    }

    fn module_type(&self) -> ModuleType {
        // MapSearchModule in C# is ISharedRegionModule.
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("MapSearchModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        // Register this scene as a searchable region.
        let block = MapBlock::new(0, 0, scene.region_name.clone());
        self.register_region(block);
        tracing::info!(region = %scene.region_name, "MapSearchModule added region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.unregister_region(&scene.region_name);
        tracing::info!(region = %scene.region_name, "MapSearchModule removed region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.regions.clear();
        Ok(())
    }
}
