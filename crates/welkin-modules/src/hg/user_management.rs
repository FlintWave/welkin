// SPDX-License-Identifier: LGPL-3.0-or-later

//! Hypergrid user management module: UUID-to-name resolution for foreign
//! users using the `"first.last@griduri"` display format.
//!
//! Maintains a TTL cache of resolved user names so that repeated lookups
//! do not hit the network.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;

use super::*;
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Cached user entry
// ---------------------------------------------------------------------------

/// A cached user-identity record with TTL.
#[derive(Debug, Clone)]
pub struct CachedUser {
    pub first_name: String,
    pub last_name: String,
    pub home_uri: String,
    pub expires: Instant,
}

impl CachedUser {
    /// Format as `"first.last@grid_authority"`, the HG display format.
    pub fn display_name(&self) -> String {
        if self.home_uri.is_empty() {
            format!("{} {}", self.first_name, self.last_name)
        } else {
            let authority =
                extract_authority(&self.home_uri).unwrap_or_else(|| self.home_uri.clone());
            format!("{}.{}@{}", self.first_name, self.last_name, authority)
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires
    }
}

/// Extract the authority (host:port) from a URI.
fn extract_authority(uri: &str) -> Option<String> {
    let stripped = if let Some(rest) = uri.strip_prefix("http://") {
        rest
    } else if let Some(rest) = uri.strip_prefix("https://") {
        rest
    } else {
        uri
    };
    let authority = stripped.split('/').next()?;
    if authority.is_empty() {
        None
    } else {
        Some(authority.to_string())
    }
}

// ---------------------------------------------------------------------------
// HGUserManagementModule
// ---------------------------------------------------------------------------

/// Hypergrid user management module.
///
/// Resolves UUIDs to display names for users from foreign grids.  Local
/// users are displayed as `"FirstName LastName"`, while foreign users
/// appear as `"First.Last@grid.example.com:8002"`.
pub struct HGUserManagementModule {
    enabled: bool,
    scenes: Vec<String>,
    local_grid_uri: String,

    /// UUID -> cached user info.
    cache: HashMap<Uuid, CachedUser>,
    /// Cache TTL in seconds.
    cache_ttl_secs: u64,
}

impl HGUserManagementModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scenes: Vec::new(),
            local_grid_uri: String::new(),
            cache: HashMap::new(),
            cache_ttl_secs: 300,
        }
    }

    /// Add a user to the cache.
    pub fn add_user(&mut self, user_id: Uuid, first_name: &str, last_name: &str, home_uri: &str) {
        let entry = CachedUser {
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            home_uri: home_uri.to_string(),
            expires: Instant::now() + Duration::from_secs(self.cache_ttl_secs),
        };
        tracing::debug!(
            user = %user_id,
            name = %entry.display_name(),
            "cached HG user"
        );
        self.cache.insert(user_id, entry);
    }

    /// Add a user from a parsed UUI.
    pub fn add_user_from_uui(&mut self, uui: &UniversalUserIdentifier) {
        self.add_user(uui.user_id, &uui.first_name, &uui.last_name, &uui.home_uri);
    }

    /// Look up a user's display name.
    ///
    /// Returns the HG-format name if found and not expired, or `None`.
    pub fn get_user_name(&self, user_id: Uuid) -> Option<String> {
        let entry = self.cache.get(&user_id)?;
        if entry.is_expired() {
            return None;
        }
        Some(entry.display_name())
    }

    /// Look up a user's raw cached record.
    pub fn get_user(&self, user_id: Uuid) -> Option<&CachedUser> {
        let entry = self.cache.get(&user_id)?;
        if entry.is_expired() {
            None
        } else {
            Some(entry)
        }
    }

    /// Check whether a UUID belongs to a local grid user.
    ///
    /// Returns `true` if the user's home URI matches the local grid, or if
    /// the user has no home URI set (assumed local).
    pub fn is_local_grid_user(&self, user_id: Uuid) -> bool {
        match self.cache.get(&user_id) {
            Some(entry) if !entry.is_expired() => {
                is_local_grid_user(&entry.home_uri, &self.local_grid_uri)
            }
            _ => true, // unknown users assumed local
        }
    }

    /// Remove a user from the cache.
    pub fn remove_user(&mut self, user_id: Uuid) {
        self.cache.remove(&user_id);
    }

    /// Purge expired entries from the cache.
    pub fn purge_expired(&mut self) {
        self.cache.retain(|_, entry| !entry.is_expired());
    }

    /// Number of entries in the cache (including expired).
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Set the cache TTL for new entries.
    pub fn set_cache_ttl(&mut self, ttl_secs: u64) {
        self.cache_ttl_secs = ttl_secs;
    }
}

impl Default for HGUserManagementModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for HGUserManagementModule {
    fn name(&self) -> &str {
        "HGUserManagementModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name =
            config.get_string("Modules", "UserManagementModule", "UserManagementModule");
        if module_name != self.name() {
            tracing::debug!("HGUserManagementModule disabled by configuration");
            return Ok(());
        }

        self.enabled = true;
        self.local_grid_uri = config.get_string("Hypergrid", "GatekeeperURI", "");
        self.cache_ttl_secs = config.get_int("HGUserManagement", "CacheTTL", 300) as u64;

        tracing::info!(
            enabled = self.enabled,
            ttl = self.cache_ttl_secs,
            "HGUserManagementModule initialised"
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
            "HGUserManagementModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(
            region = %scene.region_name,
            "HGUserManagementModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.cache.clear();
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
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: HashMap::new(),
                ints: HashMap::new(),
            }
        }

        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
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
        let mut module = HGUserManagementModule::new();
        let mut config = TestConfig::new();
        config.set_string("Modules", "UserManagementModule", "HGUserManagementModule");
        config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
        module.initialise(&config).unwrap();
        assert!(module.enabled);
        assert_eq!(module.local_grid_uri, "http://mygrid.com:8002");
    }

    // -- 2. Module disabled by default ----------------------------------------

    #[test]
    fn module_disabled_by_default() {
        let mut module = HGUserManagementModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.enabled);
    }

    // -- 3. Add and get foreign user ------------------------------------------

    #[test]
    fn add_and_get_foreign_user() {
        let mut module = HGUserManagementModule::new();
        module.local_grid_uri = "http://mygrid.com:8002".to_string();

        let user_id = Uuid::new_v4();
        module.add_user(user_id, "John", "Doe", "http://foreign.grid:8002");

        let name = module.get_user_name(user_id).unwrap();
        assert_eq!(name, "John.Doe@foreign.grid:8002");
        assert!(!module.is_local_grid_user(user_id));
    }

    // -- 4. Local user display name -------------------------------------------

    #[test]
    fn local_user_display_name() {
        let mut module = HGUserManagementModule::new();
        module.local_grid_uri = "http://mygrid.com:8002".to_string();

        let user_id = Uuid::new_v4();
        module.add_user(user_id, "Jane", "Smith", "");

        let name = module.get_user_name(user_id).unwrap();
        assert_eq!(name, "Jane Smith");
        assert!(module.is_local_grid_user(user_id));
    }

    // -- 5. Add user from UUI -------------------------------------------------

    #[test]
    fn add_user_from_uui() {
        let mut module = HGUserManagementModule::new();
        let uui = UniversalUserIdentifier::parse(
            "550e8400-e29b-41d4-a716-446655440000;http://other.grid:8002;Alice Wonderland",
        )
        .unwrap();
        module.add_user_from_uui(&uui);

        let name = module.get_user_name(uui.user_id).unwrap();
        assert_eq!(name, "Alice.Wonderland@other.grid:8002");
    }

    // -- 6. Unknown user returns None -----------------------------------------

    #[test]
    fn unknown_user_returns_none() {
        let module = HGUserManagementModule::new();
        assert!(module.get_user_name(Uuid::new_v4()).is_none());
    }

    // -- 7. Remove user -------------------------------------------------------

    #[test]
    fn remove_user() {
        let mut module = HGUserManagementModule::new();
        let user_id = Uuid::new_v4();
        module.add_user(user_id, "Test", "User", "http://grid.example.com");
        assert_eq!(module.cache_size(), 1);

        module.remove_user(user_id);
        assert_eq!(module.cache_size(), 0);
        assert!(module.get_user_name(user_id).is_none());
    }

    // -- 8. Expired entries return None ---------------------------------------

    #[test]
    fn expired_entries_return_none() {
        let mut module = HGUserManagementModule::new();
        module.cache_ttl_secs = 0; // immediate expiry

        let user_id = Uuid::new_v4();
        module.add_user(user_id, "Old", "User", "http://grid.example.com");

        // Entry is created with 0-second TTL, so it's already expired.
        assert!(module.get_user_name(user_id).is_none());
    }

    // -- 9. Purge expired entries ---------------------------------------------

    #[test]
    fn purge_expired() {
        let mut module = HGUserManagementModule::new();
        module.cache_ttl_secs = 0; // immediate expiry

        module.add_user(Uuid::new_v4(), "A", "B", "");
        module.add_user(Uuid::new_v4(), "C", "D", "");
        assert_eq!(module.cache_size(), 2);

        module.purge_expired();
        assert_eq!(module.cache_size(), 0);
    }

    // -- 10. Unknown user assumed local ---------------------------------------

    #[test]
    fn unknown_user_assumed_local() {
        let module = HGUserManagementModule::new();
        assert!(module.is_local_grid_user(Uuid::new_v4()));
    }

    // -- 11. Extract authority -------------------------------------------------

    #[test]
    fn test_extract_authority() {
        assert_eq!(
            extract_authority("http://grid.example.com:8002/path"),
            Some("grid.example.com:8002".to_string())
        );
        assert_eq!(
            extract_authority("https://secure.grid.example.com/"),
            Some("secure.grid.example.com".to_string())
        );
        assert_eq!(
            extract_authority("grid.example.com:8002"),
            Some("grid.example.com:8002".to_string())
        );
    }

    // -- 12. Module lifecycle -------------------------------------------------

    #[test]
    fn module_lifecycle() {
        let mut module = HGUserManagementModule::new();
        let mut config = TestConfig::new();
        config.set_string("Modules", "UserManagementModule", "HGUserManagementModule");
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scenes.len(), 1);

        module.remove_region(&scene).unwrap();
        assert!(module.scenes.is_empty());

        module.close().unwrap();
        assert_eq!(module.cache_size(), 0);
    }
}
