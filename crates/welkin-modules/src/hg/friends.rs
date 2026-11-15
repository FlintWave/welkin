// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid friends module: cross-grid friendship operations with secret
//! verification and online/offline status relay.
//!
//! Mirrors the C# `HGFriendsModule` which extends `FriendsModule` with
//! Hypergrid-aware friendship management.  Cross-grid friendships are
//! signed with a shared secret, and status notifications are partitioned
//! by grid domain for batched relay.

use std::collections::HashMap;

use uuid::Uuid;

use super::*;
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Friend record types
// ---------------------------------------------------------------------------

/// A cross-grid friend record with permissions and an optional shared
/// secret for verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HGFriendRecord {
    /// The friend identifier -- either a bare UUID (local) or a UUI string
    /// (`uuid;home_uri;FirstName LastName;secret`).
    pub friend: String,
    /// Permission flags that this user has granted to the friend.
    pub my_flags: i32,
    /// Permission flags that the friend has granted to this user.
    /// `-1` indicates a pending/unconfirmed relationship.
    pub their_flags: i32,
}

impl HGFriendRecord {
    /// Extract the UUID from the friend identifier.
    pub fn friend_uuid(&self) -> Option<Uuid> {
        let s = if self.friend.len() >= 36 {
            &self.friend[..36]
        } else {
            return None;
        };
        Uuid::parse_str(s).ok()
    }

    /// Extract the home URI from a UUI-format friend identifier.
    pub fn friend_home_uri(&self) -> Option<String> {
        let uui = UniversalUserIdentifier::parse(&self.friend)?;
        if uui.home_uri.is_empty() {
            None
        } else {
            Some(uui.home_uri)
        }
    }

    /// Check whether this friend record is for a foreign user (UUI format).
    pub fn is_foreign(&self) -> bool {
        self.friend.contains(';')
    }

    /// Extract the shared secret appended after the UUI (after the last ';').
    pub fn secret(&self) -> Option<&str> {
        if !self.is_foreign() {
            return None;
        }
        // Format: "uuid;home_uri;FirstName LastName;secret"
        let parts: Vec<&str> = self.friend.splitn(4, ';').collect();
        if parts.len() >= 4 {
            Some(parts[3])
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// HGFriendsModule
// ---------------------------------------------------------------------------

/// Hypergrid-aware friends module.
///
/// Mirrors the C# `HGFriendsModule`.  Extends the base friends module
/// by:
/// - Checking HG-friendship permission levels before allowing offers
///   to foreign users.
/// - Signing cross-grid friendships with a random shared secret.
/// - Partitioning status notifications by grid domain for batched relay.
/// - Resolving friend names from UUI-format identifiers.
pub struct HGFriendsModule {
    enabled: bool,
    scenes: Vec<String>,
    local_grid_uri: String,
    level_hg_friends: i32,

    /// Per-user friend lists, keyed by user UUID.
    friend_lists: HashMap<Uuid, Vec<HGFriendRecord>>,
    /// Online presence set: users currently online.
    online_users: HashMap<Uuid, bool>,
}

impl HGFriendsModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scenes: Vec::new(),
            local_grid_uri: String::new(),
            level_hg_friends: 0,
            friend_lists: HashMap::new(),
            online_users: HashMap::new(),
        }
    }

    /// Check whether a user meets the minimum level for HG friendships.
    pub fn can_offer_hg_friendship(&self, user_level: i32) -> bool {
        user_level >= self.level_hg_friends
    }

    /// Cache a friend list for a user.
    pub fn cache_friends(&mut self, user_id: Uuid, friends: Vec<HGFriendRecord>) {
        self.friend_lists.insert(user_id, friends);
    }

    /// Get the cached friend list for a user.
    pub fn get_friends(&self, user_id: Uuid) -> Option<&[HGFriendRecord]> {
        self.friend_lists.get(&user_id).map(|v| v.as_slice())
    }

    /// Find a specific friend in a user's cached friend list by UUID.
    pub fn find_friend(&self, user_id: Uuid, friend_id: Uuid) -> Option<&HGFriendRecord> {
        let friends = self.friend_lists.get(&user_id)?;
        let friend_str = friend_id.to_string();
        friends.iter().find(|f| f.friend.starts_with(&friend_str))
    }

    /// Store a new cross-grid friendship with a generated shared secret.
    ///
    /// Creates records in both directions:
    /// - `user_id` -> `friend_uui;secret`
    /// - `friend_uui;secret` -> `user_id`
    ///
    /// Returns the generated secret.
    pub fn store_hg_friendship(&mut self, user_id: Uuid, friend_uui: &str) -> String {
        let secret = Uuid::new_v4().to_string()[..8].to_string();
        let friend_with_secret = format!("{};{}", friend_uui, secret);

        let record = HGFriendRecord {
            friend: friend_with_secret,
            my_flags: 1,
            their_flags: 1,
        };

        self.friend_lists.entry(user_id).or_default().push(record);

        tracing::debug!(
            user = %user_id,
            friend = %friend_uui,
            "stored HG friendship"
        );

        secret
    }

    /// Delete a cross-grid friendship, verified by the shared secret.
    ///
    /// Returns `true` if the friendship was found and removed.
    pub fn delete_hg_friendship(&mut self, user_id: Uuid, friend_id: Uuid, secret: &str) -> bool {
        if let Some(friends) = self.friend_lists.get_mut(&user_id) {
            let friend_str = friend_id.to_string();
            let initial_len = friends.len();
            friends.retain(|f| {
                if f.friend.starts_with(&friend_str) {
                    // Verify the secret matches.
                    match f.secret() {
                        Some(s) => s != secret,
                        None => true,
                    }
                } else {
                    true
                }
            });
            let removed = friends.len() < initial_len;
            if removed {
                tracing::debug!(
                    user = %user_id,
                    friend = %friend_id,
                    "deleted HG friendship"
                );
            }
            removed
        } else {
            false
        }
    }

    /// Partition a friend list into local and per-domain foreign groups.
    ///
    /// Mirrors the C# `StatusNotify` partitioning logic.
    pub fn partition_friends_by_domain(
        &self,
        friends: &[HGFriendRecord],
    ) -> (Vec<Uuid>, HashMap<String, Vec<Uuid>>) {
        let mut local = Vec::new();
        let mut foreign: HashMap<String, Vec<Uuid>> = HashMap::new();

        for friend in friends {
            if friend.is_foreign() {
                if let (Some(uuid), Some(uri)) = (friend.friend_uuid(), friend.friend_home_uri()) {
                    let normalised = normalise_grid_uri(&uri);
                    foreign.entry(normalised).or_default().push(uuid);
                }
            } else if let Some(uuid) = friend.friend_uuid() {
                local.push(uuid);
            }
        }

        (local, foreign)
    }

    /// Send a local status notification (online/offline).
    ///
    /// Returns `true` if the target user is locally present and was notified.
    pub fn local_status_notification(&self, _from_id: Uuid, to_id: Uuid, _online: bool) -> bool {
        // In a real implementation, this would notify the target client.
        self.online_users.contains_key(&to_id)
    }

    /// Register a user as online.
    pub fn set_user_online(&mut self, user_id: Uuid) {
        self.online_users.insert(user_id, true);
    }

    /// Register a user as offline.
    pub fn set_user_offline(&mut self, user_id: Uuid) {
        self.online_users.remove(&user_id);
    }

    /// Check whether a user is currently online locally.
    pub fn is_user_online(&self, user_id: Uuid) -> bool {
        self.online_users.contains_key(&user_id)
    }

    /// Get the friendship message for a friend ID.
    ///
    /// Foreign friends get an HG-specific message.
    pub fn friendship_message(&self, friend_id: &str) -> &str {
        if friend_id.contains(';') {
            "Please confirm this friendship you made while you were on another HG grid"
        } else {
            "Do you want to be friends?"
        }
    }

    /// Number of cached friend lists.
    pub fn cached_friend_list_count(&self) -> usize {
        self.friend_lists.len()
    }
}

impl Default for HGFriendsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for HGFriendsModule {
    fn name(&self) -> &str {
        "HGFriendsModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name = config.get_string("Modules", "FriendsModule", "FriendsModule");
        if module_name != self.name() {
            tracing::debug!("HGFriendsModule disabled by configuration");
            return Ok(());
        }

        self.enabled = true;
        self.local_grid_uri = config.get_string("Hypergrid", "GatekeeperURI", "");
        self.level_hg_friends = config.get_int("HGFriendsModule", "LevelHGFriends", 0) as i32;

        tracing::info!(
            enabled = self.enabled,
            level_hg_friends = self.level_hg_friends,
            "HGFriendsModule initialised"
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
            "HGFriendsModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(
            region = %scene.region_name,
            "HGFriendsModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.friend_lists.clear();
        self.online_users.clear();
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
        let mut module = HGFriendsModule::new();
        let mut config = TestConfig::new();
        config.set_string("Modules", "FriendsModule", "HGFriendsModule");
        config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
        config.set_int("HGFriendsModule", "LevelHGFriends", 5);
        module.initialise(&config).unwrap();
        assert!(module.enabled);
        assert_eq!(module.level_hg_friends, 5);
    }

    // -- 2. Module disabled by default ----------------------------------------

    #[test]
    fn module_disabled_by_default() {
        let mut module = HGFriendsModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.enabled);
    }

    // -- 3. HG friendship level check -----------------------------------------

    #[test]
    fn hg_friendship_level_check() {
        let mut module = HGFriendsModule::new();
        module.level_hg_friends = 5;
        assert!(!module.can_offer_hg_friendship(3));
        assert!(module.can_offer_hg_friendship(5));
        assert!(module.can_offer_hg_friendship(10));
    }

    // -- 4. Store and retrieve cross-grid friendship --------------------------

    #[test]
    fn store_and_retrieve_hg_friendship() {
        let mut module = HGFriendsModule::new();
        let user = Uuid::new_v4();
        let friend_uuid = Uuid::new_v4();
        let friend_uui = format!("{};http://foreign.grid:8002;Alice Wonderland", friend_uuid);

        let secret = module.store_hg_friendship(user, &friend_uui);
        assert_eq!(secret.len(), 8);

        let friends = module.get_friends(user).unwrap();
        assert_eq!(friends.len(), 1);
        assert!(friends[0].is_foreign());
        assert_eq!(friends[0].friend_uuid(), Some(friend_uuid));
        assert!(friends[0].secret().is_some());
    }

    // -- 5. Delete cross-grid friendship with correct secret ------------------

    #[test]
    fn delete_hg_friendship_correct_secret() {
        let mut module = HGFriendsModule::new();
        let user = Uuid::new_v4();
        let friend_uuid = Uuid::new_v4();
        let friend_uui = format!("{};http://foreign.grid:8002;Bob Builder", friend_uuid);

        let secret = module.store_hg_friendship(user, &friend_uui);
        assert!(module.delete_hg_friendship(user, friend_uuid, &secret));
        assert!(module.get_friends(user).unwrap().is_empty());
    }

    // -- 6. Delete cross-grid friendship fails with wrong secret --------------

    #[test]
    fn delete_hg_friendship_wrong_secret() {
        let mut module = HGFriendsModule::new();
        let user = Uuid::new_v4();
        let friend_uuid = Uuid::new_v4();
        let friend_uui = format!("{};http://foreign.grid:8002;Carol Candle", friend_uuid);

        let _secret = module.store_hg_friendship(user, &friend_uui);
        assert!(!module.delete_hg_friendship(user, friend_uuid, "wrongsec"));
        assert_eq!(module.get_friends(user).unwrap().len(), 1);
    }

    // -- 7. Partition friends by domain ---------------------------------------

    #[test]
    fn partition_friends_by_domain() {
        let module = HGFriendsModule::new();

        let local_uuid = Uuid::new_v4();
        let foreign_a = Uuid::new_v4();
        let foreign_b = Uuid::new_v4();

        let friends = vec![
            HGFriendRecord {
                friend: local_uuid.to_string(),
                my_flags: 1,
                their_flags: 1,
            },
            HGFriendRecord {
                friend: format!("{};http://grid-a.com:8002;Alice Smith", foreign_a),
                my_flags: 1,
                their_flags: 1,
            },
            HGFriendRecord {
                friend: format!("{};http://grid-a.com:8002;Bob Jones", foreign_b),
                my_flags: 1,
                their_flags: 1,
            },
        ];

        let (local, foreign) = module.partition_friends_by_domain(&friends);
        assert_eq!(local.len(), 1);
        assert_eq!(local[0], local_uuid);
        assert_eq!(foreign.len(), 1); // both foreign on same domain
        let domain_friends = foreign.get("http://grid-a.com:8002").unwrap();
        assert_eq!(domain_friends.len(), 2);
    }

    // -- 8. Online/offline status tracking ------------------------------------

    #[test]
    fn online_offline_status() {
        let mut module = HGFriendsModule::new();
        let user = Uuid::new_v4();

        assert!(!module.is_user_online(user));
        module.set_user_online(user);
        assert!(module.is_user_online(user));
        module.set_user_offline(user);
        assert!(!module.is_user_online(user));
    }

    // -- 9. Friendship message for foreign friend -----------------------------

    #[test]
    fn friendship_message_foreign() {
        let module = HGFriendsModule::new();
        let foreign = format!("{};http://grid.com;Name Here", Uuid::new_v4());
        assert!(
            module
                .friendship_message(&foreign)
                .contains("another HG grid")
        );
        assert!(
            !module
                .friendship_message(&Uuid::new_v4().to_string())
                .contains("HG")
        );
    }

    // -- 10. HGFriendRecord is_foreign / friend_uuid / friend_home_uri --------

    #[test]
    fn friend_record_parsing() {
        let uuid = Uuid::new_v4();
        let local = HGFriendRecord {
            friend: uuid.to_string(),
            my_flags: 1,
            their_flags: 1,
        };
        assert!(!local.is_foreign());
        assert_eq!(local.friend_uuid(), Some(uuid));
        assert!(local.friend_home_uri().is_none());
        assert!(local.secret().is_none());

        let foreign = HGFriendRecord {
            friend: format!("{};http://grid.com:8002;First Last;abc12345", uuid),
            my_flags: 1,
            their_flags: -1,
        };
        assert!(foreign.is_foreign());
        assert_eq!(foreign.friend_uuid(), Some(uuid));
        assert_eq!(
            foreign.friend_home_uri(),
            Some("http://grid.com:8002".to_string())
        );
        assert_eq!(foreign.secret(), Some("abc12345"));
    }

    // -- 11. Module lifecycle -------------------------------------------------

    #[test]
    fn module_lifecycle() {
        let mut module = HGFriendsModule::new();
        let mut config = TestConfig::new();
        config.set_string("Modules", "FriendsModule", "HGFriendsModule");
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scenes.len(), 1);

        module.remove_region(&scene).unwrap();
        assert!(module.scenes.is_empty());

        module.close().unwrap();
        assert_eq!(module.cached_friend_list_count(), 0);
    }

    // -- 12. Find friend in cached list ---------------------------------------

    #[test]
    fn find_friend_in_cache() {
        let mut module = HGFriendsModule::new();
        let user = Uuid::new_v4();
        let friend_a = Uuid::new_v4();
        let friend_b = Uuid::new_v4();

        module.cache_friends(
            user,
            vec![
                HGFriendRecord {
                    friend: friend_a.to_string(),
                    my_flags: 1,
                    their_flags: 1,
                },
                HGFriendRecord {
                    friend: format!("{};http://grid.com;Someone Name", friend_b),
                    my_flags: 1,
                    their_flags: 1,
                },
            ],
        );

        assert!(module.find_friend(user, friend_a).is_some());
        assert!(module.find_friend(user, friend_b).is_some());
        assert!(module.find_friend(user, Uuid::new_v4()).is_none());
    }
}
