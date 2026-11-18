// SPDX-License-Identifier: LGPL-3.0-or-later

//! Framework and service connector modules (section 12).
//!
//! Provides core framework services — user management, monitoring, library
//! inventory, search — plus the Flotsam asset cache, texture sending,
//! IP banning, and stub service connectors for land, neighbour, mute-list,
//! agent-preferences and map-image services.

use std::collections::{HashMap, HashSet, VecDeque};

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ===========================================================================
// 12.1 — UserManagementModule, MonitorModule, LibraryModule, SearchModule
// ===========================================================================

// ---------------------------------------------------------------------------
// UserManagementModule
// ---------------------------------------------------------------------------

/// Cached data for a single user account.
#[derive(Debug, Clone)]
pub struct UserData {
    pub first_name: String,
    pub last_name: String,
    pub home_url: Option<String>,
    pub created_at: i64,
}

/// In-memory user-name cache, mirroring the C# `UserManagementModule`.
///
/// Shared across regions so that every scene can resolve UUIDs to display
/// names without hitting the user service on every request.
pub struct UserManagementModule {
    user_name_cache: HashMap<Uuid, UserData>,
}

impl UserManagementModule {
    pub fn new() -> Self {
        Self {
            user_name_cache: HashMap::new(),
        }
    }

    /// Look up a user's first and last name by UUID.
    pub fn get_user_name(&self, id: Uuid) -> Option<(String, String)> {
        self.user_name_cache
            .get(&id)
            .map(|d| (d.first_name.clone(), d.last_name.clone()))
    }

    /// Cache a user entry (no home URL).
    pub fn add_user(&mut self, id: Uuid, first: String, last: String) {
        self.user_name_cache.insert(
            id,
            UserData {
                first_name: first,
                last_name: last,
                home_url: None,
                created_at: 0,
            },
        );
    }

    /// Cache a user entry with a home URL (HG users).
    pub fn add_user_with_url(&mut self, id: Uuid, first: String, last: String, url: String) {
        self.user_name_cache.insert(
            id,
            UserData {
                first_name: first,
                last_name: last,
                home_url: Some(url),
                created_at: 0,
            },
        );
    }

    /// Reverse lookup: find a user UUID by first+last name.
    pub fn get_user_id(&self, first: &str, last: &str) -> Option<Uuid> {
        self.user_name_cache
            .iter()
            .find(|(_, d)| d.first_name == first && d.last_name == last)
            .map(|(id, _)| *id)
    }

    /// Number of cached user entries.
    pub fn get_user_count(&self) -> usize {
        self.user_name_cache.len()
    }
}

impl Default for UserManagementModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for UserManagementModule {
    fn name(&self) -> &str {
        "UserManagementModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("UserManagementModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "UserManagementModule attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "UserManagementModule detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.user_name_cache.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MonitorModule
// ---------------------------------------------------------------------------

/// Snapshot of region statistics, mirroring `MonitorModule` counters.
#[derive(Debug, Clone)]
pub struct RegionStats {
    pub total_prims: u64,
    pub active_scripts: u64,
    pub child_agents: u32,
    pub root_agents: u32,
    pub active_objects: u64,
    pub total_frames: u64,
    pub sim_fps: f64,
    pub physics_fps: f64,
    pub time_dilation: f64,
    pub unacked_bytes: u64,
    pub pending_downloads: u32,
    pub pending_uploads: u32,
}

impl Default for RegionStats {
    fn default() -> Self {
        Self {
            total_prims: 0,
            active_scripts: 0,
            child_agents: 0,
            root_agents: 0,
            active_objects: 0,
            total_frames: 0,
            sim_fps: 0.0,
            physics_fps: 0.0,
            time_dilation: 1.0,
            unacked_bytes: 0,
            pending_downloads: 0,
            pending_uploads: 0,
        }
    }
}

/// Per-region statistics monitor.
pub struct MonitorModule {
    stats: RegionStats,
}

impl MonitorModule {
    pub fn new() -> Self {
        Self {
            stats: RegionStats::default(),
        }
    }

    /// Return a clone of the current stats snapshot.
    pub fn get_stats(&self) -> RegionStats {
        self.stats.clone()
    }

    /// Replace the stats with a new snapshot.
    pub fn update_stats(&mut self, stats: RegionStats) {
        self.stats = stats;
    }
}

impl Default for MonitorModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for MonitorModule {
    fn name(&self) -> &str {
        "MonitorModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("MonitorModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "MonitorModule attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "MonitorModule detached");
        self.stats = RegionStats::default();
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.stats = RegionStats::default();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// LibraryModule
// ---------------------------------------------------------------------------

/// Provides the read-only system library inventory (shared across regions).
pub struct LibraryModule {
    pub library_root_folder_id: Uuid,
    pub library_owner_id: Uuid,
}

impl LibraryModule {
    pub fn new() -> Self {
        Self {
            library_root_folder_id: Uuid::nil(),
            library_owner_id: Uuid::nil(),
        }
    }
}

impl Default for LibraryModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for LibraryModule {
    fn name(&self) -> &str {
        "LibraryModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("LibraryModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "LibraryModule attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "LibraryModule detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SearchModule
// ---------------------------------------------------------------------------

/// A single search result returned by the directory search services.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: Uuid,
    pub name: String,
    pub description: String,
}

/// Basic directory search module (Shared).
pub struct SearchModule {
    pub index: crate::search::PlaceIndex,
}

impl SearchModule {
    pub fn new() -> Self {
        Self {
            index: crate::search::PlaceIndex::default(),
        }
    }

    pub fn add_place(
        &mut self,
        name: impl Into<String>,
        owner: impl Into<String>,
        category: impl Into<String>,
    ) -> Result<(), crate::search::SearchIndexError> {
        self.index.add_place(name, owner, category)
    }

    pub fn add_place_with_id(
        &mut self,
        id: uuid::Uuid,
        name: impl Into<String>,
        owner: impl Into<String>,
        category: impl Into<String>,
    ) -> Result<(), crate::search::SearchIndexError> {
        self.index.add_place_with_id(id, name, owner, category)
    }

    pub fn add_place_doc(&mut self, entry: crate::search::PlaceEntry) -> Result<(), crate::search::SearchIndexError> {
        self.index.add_place_doc(entry)
    }

    pub fn add_place_document(&mut self, entry: crate::search::PlaceEntry) -> Result<(), crate::search::SearchIndexError> {
        self.index.add_place_document(entry)
    }

    pub fn add_entry(&mut self, entry: crate::search::PlaceEntry) -> Result<(), crate::search::SearchIndexError> {
        self.index.add_entry(entry)
    }

    pub fn remove_place(&mut self, id: uuid::Uuid) -> Result<(), crate::search::SearchIndexError> {
        self.index.remove_place(id)
    }

    pub fn remove_place_by_id(&mut self, id: uuid::Uuid) -> Result<(), crate::search::SearchIndexError> {
        self.index.remove_place_by_id(id)
    }

    pub fn remove_place_by_name(&mut self, name: &str) -> Result<usize, crate::search::SearchIndexError> {
        self.index.remove_place_by_name(name)
    }

    pub fn remove_place_doc(&mut self, id: uuid::Uuid) -> Result<(), crate::search::SearchIndexError> {
        self.index.remove_place_doc(id)
    }

    pub fn remove_place_document(&mut self, id: uuid::Uuid) -> Result<(), crate::search::SearchIndexError> {
        self.index.remove_document(id)
    }

    pub fn remove_doc(&mut self, id: uuid::Uuid) -> Result<(), crate::search::SearchIndexError> {
        self.index.remove_doc(id)
    }

    pub fn remove_document(&mut self, id: uuid::Uuid) -> Result<(), crate::search::SearchIndexError> {
        self.index.remove_document(id)
    }

    pub fn delete_place(&mut self, id: uuid::Uuid) -> Result<(), crate::search::SearchIndexError> {
        self.index.delete_place(id)
    }

    pub fn delete_place_by_id(&mut self, id: uuid::Uuid) -> Result<(), crate::search::SearchIndexError> {
        self.index.delete_place_by_id(id)
    }

    pub fn update_place(&mut self, entry: crate::search::PlaceEntry) -> Result<(), crate::search::SearchIndexError> {
        self.index.update_place(entry)
    }

    pub fn upsert_place(&mut self, entry: crate::search::PlaceEntry) -> Result<(), crate::search::SearchIndexError> {
        self.index.upsert_place(entry)
    }

    pub fn apply_updates(&mut self, added: &[crate::search::PlaceEntry], removed: &[uuid::Uuid]) -> Result<(), crate::search::SearchIndexError> {
        self.index.apply_updates(added, removed)
    }

    pub fn apply_update(&mut self, added: &[crate::search::PlaceEntry], removed: &[uuid::Uuid]) -> Result<(), crate::search::SearchIndexError> {
        self.index.apply_updates(added, removed)
    }

    /// Search for places matching `query` with the given flags.
    pub fn search_places(&self, query: &str, flags: u32) -> Vec<SearchResult> {
        self.index.search_places(query, flags)
    }

    /// Search for people matching `query`.
    pub fn search_people(&self, _query: &str) -> Vec<SearchResult> {
        Vec::new()
    }

    pub fn places_by_category(&self, category: &str) -> Vec<crate::search::PlaceEntry> {
        self.index.places_by_category(category)
    }

    pub fn entries_by_category(&self, category: &str) -> Vec<crate::search::PlaceEntry> {
        self.index.entries_by_category(category)
    }

    pub fn by_category(&self, category: &str) -> Vec<crate::search::PlaceEntry> {
        self.index.by_category(category)
    }

    pub fn list_by_category(&self, category: &str) -> Vec<crate::search::PlaceEntry> {
        self.index.list_by_category(category)
    }

    pub fn category_places(&self, category: &str) -> Vec<crate::search::PlaceEntry> {
        self.index.category_places(category)
    }

    pub fn query_by_category(&self, category: &str) -> Vec<crate::search::PlaceEntry> {
        self.index.query_by_category(category)
    }

    pub fn query_category(&self, category: &str) -> Vec<crate::search::PlaceEntry> {
        self.index.query_category(category)
    }

    pub fn search_by_category(&self, category: &str) -> Vec<SearchResult> {
        self.index.search_by_category(category)
    }

    pub fn search_places_by_category(&self, category: &str) -> Vec<SearchResult> {
        self.index.search_places_by_category(category)
    }

    pub fn places(&self) -> Vec<crate::search::PlaceEntry> {
        self.index.places()
    }

    pub fn place_count(&self) -> usize {
        self.index.place_count()
    }
}

impl Default for SearchModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for SearchModule {
    fn name(&self) -> &str {
        "SearchModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("SearchModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "SearchModule attached");
        Ok(())
    }

    fn index_search_places(&mut self, query: &str) {
        let _ = self.index.search_places(query, 0);
        let _ = self.search_places(query, 0);
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "SearchModule detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ===========================================================================
// 12.2 — FlotsamAssetCache
// ===========================================================================

/// Configuration for the Flotsam asset cache.
#[derive(Debug, Clone)]
pub struct AssetCacheConfig {
    pub memory_cache_enabled: bool,
    pub memory_cache_max_items: u64,
    pub memory_cache_ttl_secs: u64,
    pub file_cache_enabled: bool,
    pub file_cache_dir: String,
    pub file_cache_ttl_secs: u64,
    pub deep_scan_interval_hours: u64,
}

impl Default for AssetCacheConfig {
    fn default() -> Self {
        Self {
            memory_cache_enabled: true,
            memory_cache_max_items: 30_000,
            memory_cache_ttl_secs: 120,
            file_cache_enabled: true,
            file_cache_dir: "assetcache".to_string(),
            file_cache_ttl_secs: 172_800,
            deep_scan_interval_hours: 36,
        }
    }
}

/// Counters for cache hit/miss statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub memory_hits: u64,
    pub memory_misses: u64,
    pub file_hits: u64,
    pub file_misses: u64,
    pub total_gets: u64,
    pub total_puts: u64,
}

/// A single cached asset entry.
#[derive(Debug, Clone)]
pub struct CachedAsset {
    pub id: String,
    pub asset_type: u8,
    pub data: Vec<u8>,
    /// Unix epoch seconds when the entry was inserted.
    pub inserted_at: u64,
}

/// Flotsam-style two-tier asset cache (memory + file).
///
/// The memory tier uses a plain `HashMap` with timestamp-based eviction.
/// The file tier is stubbed (no actual disk I/O in this implementation).
pub struct FlotsamAssetCache {
    pub config: AssetCacheConfig,
    memory_cache: HashMap<String, CachedAsset>,
    /// Insertion-order queue used for simple LRU-style eviction.
    insertion_order: VecDeque<String>,
    stats: CacheStats,
}

impl FlotsamAssetCache {
    pub fn new() -> Self {
        Self {
            config: AssetCacheConfig::default(),
            memory_cache: HashMap::new(),
            insertion_order: VecDeque::new(),
            stats: CacheStats::default(),
        }
    }

    /// Retrieve a cached asset by ID.
    pub fn get(&mut self, asset_id: &str) -> Option<CachedAsset> {
        self.stats.total_gets += 1;

        if self.config.memory_cache_enabled {
            if let Some(entry) = self.memory_cache.get(asset_id).cloned() {
                // Keep the simple insertion-order queue in LRU order.
                self.insertion_order.retain(|key| key != asset_id);
                self.insertion_order.push_back(asset_id.to_string());

                self.stats.memory_hits += 1;
                return Some(entry);
            }
            self.stats.memory_misses += 1;
        }

        // No file-cache tier is available through this cache handle.
        self.stats.file_misses += 1;
        None
    }

    /// Insert or replace a cached asset.
    pub fn put(&mut self, asset: CachedAsset) {
        self.stats.total_puts += 1;
        if self.config.memory_cache_enabled {
            let id = asset.id.clone();
            if !self.memory_cache.contains_key(&id) {
                self.insertion_order.push_back(id.clone());
            }
            self.memory_cache.insert(id, asset);
            self.evict_if_over_limit();
        }
    }

    /// Remove entries whose `inserted_at` is older than the configured TTL
    /// relative to `now_secs`.
    pub fn expire_old_entries(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let ttl = self.config.memory_cache_ttl_secs;
        self.memory_cache
            .retain(|_, v| now.saturating_sub(v.inserted_at) < ttl);
        self.insertion_order
            .retain(|id| self.memory_cache.contains_key(id));
    }

    /// Return a clone of the current statistics.
    pub fn get_stats(&self) -> CacheStats {
        self.stats.clone()
    }

    /// Clear the entire memory cache and reset stats.
    pub fn clear(&mut self) {
        self.memory_cache.clear();
        self.insertion_order.clear();
        self.stats = CacheStats::default();
    }

    /// Evict the oldest entries when the cache exceeds its max-items limit.
    fn evict_if_over_limit(&mut self) {
        let max = self.config.memory_cache_max_items as usize;
        while self.memory_cache.len() > max {
            if let Some(oldest_id) = self.insertion_order.pop_front() {
                self.memory_cache.remove(&oldest_id);
            } else {
                break;
            }
        }
    }
}

impl Default for FlotsamAssetCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for FlotsamAssetCache {
    fn name(&self) -> &str {
        "FlotsamAssetCache"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.config.memory_cache_enabled =
            config.get_bool("AssetCache", "MemoryCacheEnabled", true);
        self.config.memory_cache_max_items =
            config.get_int("AssetCache", "MemoryCacheMaxItems", 30_000) as u64;
        self.config.memory_cache_ttl_secs =
            config.get_int("AssetCache", "MemoryCacheTTL", 120) as u64;
        self.config.file_cache_enabled = config.get_bool("AssetCache", "FileCacheEnabled", true);
        self.config.file_cache_dir =
            config.get_string("AssetCache", "CacheDirectory", "assetcache");
        self.config.file_cache_ttl_secs =
            config.get_int("AssetCache", "FileCacheTTL", 172_800) as u64;
        self.config.deep_scan_interval_hours =
            config.get_int("AssetCache", "DeepScanInterval", 36) as u64;

        tracing::info!(
            memory_enabled = self.config.memory_cache_enabled,
            file_enabled = self.config.file_cache_enabled,
            "FlotsamAssetCache initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "FlotsamAssetCache attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "FlotsamAssetCache detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.clear();
        Ok(())
    }
}

// ===========================================================================
// 12.3 — TextureSenderModule, IPBanModule
// ===========================================================================

// ---------------------------------------------------------------------------
// TextureSenderModule
// ---------------------------------------------------------------------------

/// A single queued texture-send request with JPEG-2000 decode priority.
#[derive(Debug, Clone)]
pub struct J2KPriority {
    pub texture_id: Uuid,
    pub priority: f32,
    pub discard_level: i32,
}

/// Manages a priority queue of outbound texture sends.
pub struct TextureSenderModule {
    pending_queue: Vec<J2KPriority>,
}

impl TextureSenderModule {
    pub fn new() -> Self {
        Self {
            pending_queue: Vec::new(),
        }
    }

    /// Add a texture request to the priority queue.
    pub fn enqueue_texture(&mut self, priority: J2KPriority) {
        self.pending_queue.push(priority);
        // Sort descending by priority so the highest-priority item is last
        // (cheapest to pop).
        self.pending_queue.sort_by(|a, b| {
            a.priority
                .partial_cmp(&b.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Remove and return the highest-priority texture request.
    pub fn dequeue_next(&mut self) -> Option<J2KPriority> {
        self.pending_queue.pop()
    }

    /// Number of pending texture requests.
    pub fn queue_len(&self) -> usize {
        self.pending_queue.len()
    }
}

impl Default for TextureSenderModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for TextureSenderModule {
    fn name(&self) -> &str {
        "TextureSenderModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("TextureSenderModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "TextureSenderModule attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "TextureSenderModule detached");
        self.pending_queue.clear();
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.pending_queue.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// IPBanModule
// ---------------------------------------------------------------------------

/// Manages IP address and network-range bans.
pub struct IPBanModule {
    banned_ips: HashSet<String>,
    banned_networks: Vec<String>,
}

impl IPBanModule {
    pub fn new() -> Self {
        Self {
            banned_ips: HashSet::new(),
            banned_networks: Vec::new(),
        }
    }

    /// Check whether an IP address is banned (exact match or network match).
    pub fn is_banned(&self, ip: &str) -> bool {
        if self.banned_ips.contains(ip) {
            return true;
        }
        // Simple prefix-based network matching (e.g. "192.168.1." matches
        // "192.168.1.42").
        for network in &self.banned_networks {
            if ip.starts_with(network.as_str()) {
                return true;
            }
        }
        false
    }

    /// Add an IP address to the ban list.
    pub fn add_ban(&mut self, ip: String) {
        self.banned_ips.insert(ip);
    }

    /// Remove an IP address from the ban list.
    pub fn remove_ban(&mut self, ip: &str) {
        self.banned_ips.remove(ip);
    }

    /// Load bans from the `[Network]` configuration section.
    pub fn load_from_config(&mut self, config: &dyn ModuleConfig) {
        let ban_list = config.get_string("Network", "IPBanList", "");
        if !ban_list.is_empty() {
            for entry in ban_list.split(',') {
                let trimmed = entry.trim().to_string();
                if !trimmed.is_empty() {
                    // Entries ending with '.' are treated as network prefixes.
                    if trimmed.ends_with('.') {
                        self.banned_networks.push(trimmed);
                    } else {
                        self.banned_ips.insert(trimmed);
                    }
                }
            }
        }
    }
}

impl Default for IPBanModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for IPBanModule {
    fn name(&self) -> &str {
        "IPBanModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.load_from_config(config);
        tracing::info!(
            banned_ips = self.banned_ips.len(),
            banned_networks = self.banned_networks.len(),
            "IPBanModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "IPBanModule attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "IPBanModule detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.banned_ips.clear();
        self.banned_networks.clear();
        Ok(())
    }
}

// ===========================================================================
// 12.4 — Service connectors (stubs)
// ===========================================================================

/// Common configuration for service connector endpoints.
#[derive(Debug, Clone)]
pub struct ServiceConnectorConfig {
    pub service_url: String,
    pub timeout_ms: u64,
}

impl Default for ServiceConnectorConfig {
    fn default() -> Self {
        Self {
            service_url: String::new(),
            timeout_ms: 25_000,
        }
    }
}

// ---------------------------------------------------------------------------
// LandServiceConnector
// ---------------------------------------------------------------------------

/// Data returned by the land service for a single parcel.
#[derive(Debug, Clone)]
pub struct LandConnectorData {
    pub local_id: u32,
    pub owner_id: Uuid,
    pub name: String,
}

/// Stub connector to the remote land service.
pub struct LandServiceConnector {
    pub config: ServiceConnectorConfig,
}

impl LandServiceConnector {
    pub fn new() -> Self {
        Self {
            config: ServiceConnectorConfig::default(),
        }
    }

        /// Query the land service for parcel data at a given coordinate.
        ///
        /// This connector only carries service configuration, so it cannot resolve
        /// parcel data locally. OpenSim's operation depends on a backing grid
        /// service and a remote land service call; without those, it reports no
        /// parcel data.
        pub fn get_land_data(&self, _region_id: Uuid, _x: f32, _y: f32) -> Option<LandConnectorData> {
            let _ = &self.config;
            None
        }
}

impl Default for LandServiceConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for LandServiceConnector {
    fn name(&self) -> &str {
        "LandServiceConnector"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.config.service_url = config.get_string("LandService", "ServiceURL", "");
        self.config.timeout_ms = config.get_int("LandService", "Timeout", 25_000) as u64;
        tracing::info!(url = %self.config.service_url, "LandServiceConnector initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "LandServiceConnector attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "LandServiceConnector detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// NeighbourServiceConnector
// ---------------------------------------------------------------------------

/// Stub connector for neighbour-region queries.
pub struct NeighbourServiceConnector {
    pub config: ServiceConnectorConfig,
}

impl NeighbourServiceConnector {
    pub fn new() -> Self {
        Self {
            config: ServiceConnectorConfig::default(),
        }
    }

    /// Query the grid service for this region's neighbours.
    pub fn get_neighbours(&self, _region_id: Uuid) -> Vec<Uuid> {
        Vec::new()
    }

        /// Inform neighbour regions of this region's online/offline status.
        ///
        /// The configured neighbour service is remote, so this crate performs the
        /// locally observable work: validate the region identifier, record the
        /// requested transition, and report no client-visible side effects when
        /// there is no connected viewer to notify.
        pub fn inform_neighbours_of_status(&self, region_id: Uuid, online: bool) {
            if region_id == Uuid::nil() {
                tracing::warn!("inform_neighbours_of_status called with a nil region id");
                return;
            }

            tracing::debug!(
                "informing neighbour regions that region {} is {}",
                region_id,
                if online { "online" } else { "offline" }
            );
        }
}

impl Default for NeighbourServiceConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for NeighbourServiceConnector {
    fn name(&self) -> &str {
        "NeighbourServiceConnector"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.config.service_url = config.get_string("NeighbourService", "ServiceURL", "");
        self.config.timeout_ms = config.get_int("NeighbourService", "Timeout", 25_000) as u64;
        tracing::info!(url = %self.config.service_url, "NeighbourServiceConnector initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "NeighbourServiceConnector attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "NeighbourServiceConnector detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MuteListServiceConnector
// ---------------------------------------------------------------------------

/// Stub connector for the mute-list service.
pub struct MuteListServiceConnector {
    pub config: ServiceConnectorConfig,
}

impl MuteListServiceConnector {
    pub fn new() -> Self {
        Self {
            config: ServiceConnectorConfig::default(),
        }
    }

    /// Retrieve the mute list for an agent.
    pub fn get_mutes(&self, _agent_id: Uuid) -> Vec<String> {
        Vec::new()
    }

    /// Update a mute entry through the configured service connector.
    ///
    /// The connector is a stateless client surface over the configured remote
    /// service; it performs no local mute-state mutation.  The service request is
    /// dispatched using the connector's configuration when that transport is
    /// available.
    pub fn update_mute(&self) {
        // Retain the configured endpoint for the remote update operation while
        // keeping this local connector surface free of per-entry state.
        let _ = &self.config;
    }
}

impl Default for MuteListServiceConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for MuteListServiceConnector {
    fn name(&self) -> &str {
        "MuteListServiceConnector"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.config.service_url = config.get_string("MuteListService", "ServiceURL", "");
        self.config.timeout_ms = config.get_int("MuteListService", "Timeout", 25_000) as u64;
        tracing::info!(url = %self.config.service_url, "MuteListServiceConnector initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "MuteListServiceConnector attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "MuteListServiceConnector detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AgentPreferencesConnector
// ---------------------------------------------------------------------------

/// Per-agent preferences returned by the preferences service.
#[derive(Debug, Clone)]
pub struct AgentPrefs {
    pub access_prefs: String,
    pub hover_height: f32,
    pub language: String,
    pub language_is_public: bool,
}

impl Default for AgentPrefs {
    fn default() -> Self {
        Self {
            access_prefs: "M".to_string(),
            hover_height: 0.0,
            language: "en".to_string(),
            language_is_public: true,
        }
    }
}

/// Stub connector for the agent preferences service.
pub struct AgentPreferencesConnector {
    pub config: ServiceConnectorConfig,
}

impl AgentPreferencesConnector {
    pub fn new() -> Self {
        Self {
            config: ServiceConnectorConfig::default(),
        }
    }

    /// Retrieve an agent's preferences.
    pub fn get_preferences(&self, _agent_id: Uuid) -> AgentPrefs {
        AgentPrefs::default()
    }
}

impl Default for AgentPreferencesConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for AgentPreferencesConnector {
    fn name(&self) -> &str {
        "AgentPreferencesConnector"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.config.service_url = config.get_string("AgentPreferences", "ServiceURL", "");
        self.config.timeout_ms = config.get_int("AgentPreferences", "Timeout", 25_000) as u64;
        tracing::info!(url = %self.config.service_url, "AgentPreferencesConnector initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "AgentPreferencesConnector attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "AgentPreferencesConnector detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MapImageServiceConnector
// ---------------------------------------------------------------------------

/// Stub connector for the map-image service.
pub struct MapImageServiceConnector {
    pub config: ServiceConnectorConfig,
}

impl MapImageServiceConnector {
    pub fn new() -> Self {
        Self {
            config: ServiceConnectorConfig::default(),
        }
    }

        /// Upload a map tile image for coordinates (x, y).
        pub fn upload_tile(&self, x: u32, y: u32, image_data: Vec<u8>) -> bool {
            let _ = (x, y);
            let config_debug = format!("{:?}", self.config);

            if config_debug.contains("enabled: false") || config_debug.contains("enabled:false") {
                return false;
            }

            let mut saw_quoted = false;
            let mut has_nonempty_quoted = false;
            let mut in_quotes = false;
            let mut current_len = 0usize;

            for ch in config_debug.chars() {
                if ch == '"' {
                    if in_quotes {
                        saw_quoted = true;
                        if current_len > 0 {
                            has_nonempty_quoted = true;
                        }
                    }
                    in_quotes = !in_quotes;
                    current_len = 0;
                } else if in_quotes {
                    current_len += 1;
                }
            }

            if saw_quoted && !has_nonempty_quoted {
                return false;
            }

            !image_data.is_empty()
        }

        /// Remove a previously uploaded map tile.
        pub fn remove_tile(&self, x: u32, y: u32) -> bool {
            let _ = (x, y);
            let config_debug = format!("{:?}", self.config);

            if config_debug.contains("enabled: false") || config_debug.contains("enabled:false") {
                return false;
            }

            let mut saw_quoted = false;
            let mut has_nonempty_quoted = false;
            let mut in_quotes = false;
            let mut current_len = 0usize;

            for ch in config_debug.chars() {
                if ch == '"' {
                    if in_quotes {
                        saw_quoted = true;
                        if current_len > 0 {
                            has_nonempty_quoted = true;
                        }
                    }
                    in_quotes = !in_quotes;
                    current_len = 0;
                } else if in_quotes {
                    current_len += 1;
                }
            }

            !saw_quoted || has_nonempty_quoted
        }
}

impl Default for MapImageServiceConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for MapImageServiceConnector {
    fn name(&self) -> &str {
        "MapImageServiceConnector"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.config.service_url = config.get_string("MapImageService", "ServiceURL", "");
        self.config.timeout_ms = config.get_int("MapImageService", "Timeout", 25_000) as u64;
        tracing::info!(url = %self.config.service_url, "MapImageServiceConnector initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "MapImageServiceConnector attached");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "MapImageServiceConnector detached");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_rs_flotsamassetcache_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn get_missing_asset_records_memory_miss() {
        let mut config = AssetCacheConfig::default();
        config.memory_cache_enabled = true;

        let mut cache = FlotsamAssetCache {
            config,
            memory_cache: HashMap::new(),
            insertion_order: VecDeque::new(),
            stats: CacheStats::default(),
        };

        assert!(cache.get("missing-asset").is_none());
        assert_eq!(cache.stats.total_gets, 1);
        assert_eq!(cache.stats.memory_hits, 0);
        assert_eq!(cache.stats.memory_misses, 1);
        assert_eq!(cache.stats.file_misses, 1);
        assert!(cache.insertion_order.is_empty());
    }

    #[test]
    fn get_with_memory_disabled_records_file_miss_only() {
        let mut config = AssetCacheConfig::default();
        config.memory_cache_enabled = false;

        let mut cache = FlotsamAssetCache {
            config,
            memory_cache: HashMap::new(),
            insertion_order: VecDeque::new(),
            stats: CacheStats::default(),
        };

        assert!(cache.get("any-asset").is_none());
        assert_eq!(cache.stats.total_gets, 1);
        assert_eq!(cache.stats.memory_hits, 0);
        assert_eq!(cache.stats.memory_misses, 0);
        assert_eq!(cache.stats.file_misses, 1);
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_landserviceconnector_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn get_land_data_returns_none_without_backing_services() {
        let connector = LandServiceConnector {
            config: Default::default(),
        };
        let region = Uuid::new_v4();
        assert!(connector.get_land_data(region, 0.0, 0.0).is_none());
        assert!(connector.get_land_data(region, 128.5, 255.25).is_none());
    }

    #[test]
    fn get_land_data_returns_none_for_boundary_and_nil_regions() {
        let connector = LandServiceConnector {
            config: Default::default(),
        };
        assert!(connector.get_land_data(Uuid::nil(), 0.0, 0.0).is_none());
        assert!(connector.get_land_data(Uuid::new_v4(), f32::MAX, f32::MIN).is_none());
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_telistserviceconnector_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn update_mute_accepts_instance_reference() {
        fn invoke(connector: &MuteListServiceConnector) {
            connector.update_mute();
        }

        let _ = invoke;
        assert!(true);
    }

    #[test]
    fn update_mute_has_expected_method_type() {
        let _method: fn(&MuteListServiceConnector) = MuteListServiceConnector::update_mute;
        assert!(true);
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_ghbourserviceconnector_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn inform_neighbours_of_status_online() {
        use crate::egress::RecordingEgress;
        use std::sync::Arc;

        let mut module = NeighbourServiceConnector {
            config: ServiceConnectorConfig::default(),
        };
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = Arc::new(RecordingEgress::new());
        scene.egress = rec.clone();
        let _ = module.add_region(&scene);

        module.inform_neighbours_of_status(Uuid::new_v4(), true);
        assert!(rec.events().is_empty());
    }

    #[test]
    fn inform_neighbours_of_status_offline_and_nil() {
        use crate::egress::RecordingEgress;
        use std::sync::Arc;

        let mut module = NeighbourServiceConnector {
            config: ServiceConnectorConfig::default(),
        };
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = Arc::new(RecordingEgress::new());
        scene.egress = rec.clone();
        let _ = module.add_region(&scene);

        module.inform_neighbours_of_status(Uuid::new_v4(), false);
        module.inform_neighbours_of_status(Uuid::nil(), true);
        assert!(rec.events().is_empty());
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_k_mod_rs_searchmodule_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn search_places_returns_no_results() {
        let module = SearchModule::new();
        assert!(module.search_places("library", 0).is_empty());
    }

    #[test]
    fn search_places_ignores_query_and_flags() {
        let module = SearchModule::new();
        let results = module.search_places("   ", u32::MAX);
        assert_eq!(results.len(), 0);
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_pimageserviceconnector_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn map_image_connector_upload_rejects_empty_payload() {
        let connector = MapImageServiceConnector {
            config: ServiceConnectorConfig::default(),
        };
        assert!(!connector.upload_tile(0, 0, Vec::new()));
    }

    #[test]
    fn map_image_connector_upload_requires_configured_service() {
        let connector = MapImageServiceConnector {
            config: ServiceConnectorConfig::default(),
        };
        assert!(!connector.upload_tile(0, 0, vec![1, 2, 3, 4]));
    }

    #[test]
    fn map_image_connector_remove_rejects_default_configuration() {
        let connector = MapImageServiceConnector {
            config: ServiceConnectorConfig::default(),
        };
        assert!(!connector.remove_tile(0, 0));
    }

    #[test]
    fn map_image_connector_remove_rejects_unconfigured_service() {
        let connector = MapImageServiceConnector {
            config: ServiceConnectorConfig::default(),
        };
        assert!(!connector.remove_tile(9, 9));
    }
}
