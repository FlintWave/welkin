// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid lure module: cross-grid teleport lures with gatekeeper URI
//! resolution.
//!
//! Mirrors the C# `HGLureModule`.  When a user sends a teleport lure to
//! another avatar, the lure message is tagged with the local grid's
//! gatekeeper URI so that the recipient can resolve the destination via
//! the foreign gatekeeper.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;

use super::*;
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Pending-lure cache
// ---------------------------------------------------------------------------

/// A cached pending lure with its expiration time.
#[derive(Debug, Clone)]
struct PendingLure {
    /// UUID of the agent who initiated the lure.
    from_agent_id: Uuid,
    /// Name of the agent who initiated the lure.
    from_agent_name: String,
    /// The region ID the lure points to.
    region_id: Uuid,
    /// The message attached to the lure (includes gatekeeper URI after '@').
    message: String,
    /// Position to land at in the target region.
    position: [f32; 3],
    /// When this entry expires.
    expires: Instant,
}

/// An expiring cache of pending teleport lures, keyed by session ID.
#[derive(Debug)]
struct PendingLureCache {
    entries: HashMap<Uuid, PendingLure>,
    ttl: Duration,
}

impl PendingLureCache {
    fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    fn insert(&mut self, session_id: Uuid, lure: PendingLure) {
        self.entries.insert(session_id, lure);
    }

    fn get(&self, session_id: &Uuid) -> Option<&PendingLure> {
        if let Some(lure) = self.entries.get(session_id)
            && Instant::now() < lure.expires
        {
            return Some(lure);
        }
        None
    }

    fn remove(&mut self, session_id: &Uuid) -> Option<PendingLure> {
        self.entries.remove(session_id)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// HGLureModule
// ---------------------------------------------------------------------------

/// Hypergrid-aware teleport lure module.
///
/// Mirrors the C# `HGLureModule`.  Lure messages are annotated with the
/// local grid's gatekeeper URI (`message@gatekeeper_uri`), enabling the
/// recipient to resolve the destination across grid boundaries.
pub struct HGLureModule {
    enabled: bool,
    scenes: Vec<String>,
    gatekeeper_uri: String,
    pending_lures: PendingLureCache,
}

impl HGLureModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scenes: Vec::new(),
            gatekeeper_uri: String::new(),
            pending_lures: PendingLureCache::new(Duration::from_secs(7200)),
        }
    }

    /// Start a lure: create a pending-lure record and return the lure
    /// message tagged with the gatekeeper URI.
    ///
    /// Mirrors the C# `OnStartLure`.
    pub fn start_lure(
        &mut self,
        from_agent_id: Uuid,
        from_agent_name: &str,
        target_id: Uuid,
        message: &str,
        region_id: Uuid,
        position: [f32; 3],
    ) -> (Uuid, String) {
        let session_id = Uuid::new_v4();
        let tagged_message = format!("{}@{}", message, self.gatekeeper_uri);

        let lure = PendingLure {
            from_agent_id,
            from_agent_name: from_agent_name.to_string(),
            region_id,
            message: tagged_message.clone(),
            position,
            expires: Instant::now() + self.pending_lures.ttl,
        };
        self.pending_lures.insert(session_id, lure);

        tracing::debug!(
            from = %from_agent_id,
            to = %target_id,
            session = %session_id,
            "HG lure started"
        );

        (session_id, tagged_message)
    }

    /// Accept a pending lure by session ID.
    ///
    /// Returns the lure details (from_agent, region, message, position) if
    /// the lure is still valid, or `None` if it has expired or was not found.
    ///
    /// Mirrors the C# `OnTeleportLureRequest`.
    pub fn accept_lure(&mut self, lure_id: Uuid) -> Option<AcceptedLure> {
        let lure = self.pending_lures.remove(&lure_id)?;
        if Instant::now() >= lure.expires {
            return None;
        }

        Some(AcceptedLure {
            from_agent_id: lure.from_agent_id,
            from_agent_name: lure.from_agent_name,
            region_id: lure.region_id,
            message: lure.message.clone(),
            position: lure.position,
            gatekeeper_uri: self.extract_gatekeeper_uri(&lure.message),
        })
    }

    /// Handle an incoming lure IM (RequestTeleport or GodLikeRequestTeleport).
    ///
    /// Stores the lure in the pending cache and returns `true` if it is new.
    pub fn handle_incoming_lure(
        &mut self,
        session_id: Uuid,
        from_agent_id: Uuid,
        from_agent_name: &str,
        region_id: Uuid,
        message: &str,
        position: [f32; 3],
    ) -> bool {
        if self.pending_lures.get(&session_id).is_some() {
            return false; // already have it
        }

        let lure = PendingLure {
            from_agent_id,
            from_agent_name: from_agent_name.to_string(),
            region_id,
            message: message.to_string(),
            position,
            expires: Instant::now() + self.pending_lures.ttl,
        };
        self.pending_lures.insert(session_id, lure);

        tracing::debug!(
            session = %session_id,
            from = %from_agent_id,
            region = %region_id,
            "incoming HG lure stored"
        );
        true
    }

    /// Extract the gatekeeper URI from a tagged lure message.
    ///
    /// The message format is `"original text@gatekeeper_uri"`.
    fn extract_gatekeeper_uri(&self, message: &str) -> Option<String> {
        let parts: Vec<&str> = message.rsplitn(2, '@').collect();
        if parts.len() == 2 {
            let uri = parts[0].trim();
            if !uri.is_empty() {
                return Some(uri.to_string());
            }
        }
        None
    }

    /// Check whether a URI refers to the local grid.
    pub fn is_local_grid(&self, uri: &str) -> bool {
        is_same_grid(uri, &self.gatekeeper_uri)
    }

    /// Number of pending lures (for diagnostics).
    pub fn pending_count(&self) -> usize {
        self.pending_lures.len()
    }
}

impl Default for HGLureModule {
    fn default() -> Self {
        Self::new()
    }
}

/// Details of an accepted lure, returned by [`HGLureModule::accept_lure`].
#[derive(Debug, Clone)]
pub struct AcceptedLure {
    pub from_agent_id: Uuid,
    pub from_agent_name: String,
    pub region_id: Uuid,
    pub message: String,
    pub position: [f32; 3],
    /// The gatekeeper URI extracted from the lure message, if present.
    pub gatekeeper_uri: Option<String>,
}

impl RegionModule for HGLureModule {
    fn name(&self) -> &str {
        "HGLureModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name = config.get_string("Messaging", "LureModule", "");
        if module_name != self.name() {
            tracing::debug!("HGLureModule disabled by configuration");
            return Ok(());
        }

        self.enabled = true;
        self.gatekeeper_uri = config.get_string("Hypergrid", "GatekeeperURI", "");

        tracing::info!(
            enabled = self.enabled,
            gatekeeper = %self.gatekeeper_uri,
            "HGLureModule initialised"
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
            "HGLureModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(
            region = %scene.region_name,
            "HGLureModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.pending_lures.clear();
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
        let mut module = HGLureModule::new();
        let mut config = TestConfig::new();
        config.set_string("Messaging", "LureModule", "HGLureModule");
        config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
        module.initialise(&config).unwrap();
        assert!(module.enabled);
        assert_eq!(module.gatekeeper_uri, "http://mygrid.com:8002");
    }

    // -- 2. Module disabled by default ----------------------------------------

    #[test]
    fn module_disabled_by_default() {
        let mut module = HGLureModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.enabled);
    }

    // -- 3. Start lure creates pending record ---------------------------------

    #[test]
    fn start_lure_creates_pending() {
        let mut module = HGLureModule::new();
        module.enabled = true;
        module.gatekeeper_uri = "http://mygrid.com:8002".to_string();

        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let region = Uuid::new_v4();
        let (session, message) = module.start_lure(
            from,
            "Test User",
            to,
            "Join me",
            region,
            [128.0, 128.0, 25.0],
        );

        assert!(!session.is_nil());
        assert!(message.contains("@http://mygrid.com:8002"));
        assert_eq!(module.pending_count(), 1);
    }

    // -- 4. Accept lure returns details and removes from cache ----------------

    #[test]
    fn accept_lure_returns_details() {
        let mut module = HGLureModule::new();
        module.enabled = true;
        module.gatekeeper_uri = "http://mygrid.com:8002".to_string();

        let from = Uuid::new_v4();
        let region = Uuid::new_v4();
        let (session, _) = module.start_lure(
            from,
            "Test User",
            Uuid::new_v4(),
            "Come here",
            region,
            [100.0, 200.0, 30.0],
        );

        let accepted = module.accept_lure(session).unwrap();
        assert_eq!(accepted.from_agent_id, from);
        assert_eq!(accepted.region_id, region);
        assert_eq!(accepted.position, [100.0, 200.0, 30.0]);
        assert!(accepted.gatekeeper_uri.is_some());
        assert_eq!(accepted.gatekeeper_uri.unwrap(), "http://mygrid.com:8002");
        // Lure removed from cache.
        assert_eq!(module.pending_count(), 0);
    }

    // -- 5. Accept unknown lure returns None ----------------------------------

    #[test]
    fn accept_unknown_lure_returns_none() {
        let mut module = HGLureModule::new();
        assert!(module.accept_lure(Uuid::new_v4()).is_none());
    }

    // -- 6. Handle incoming lure stores new, rejects duplicate ----------------

    #[test]
    fn handle_incoming_lure_dedup() {
        let mut module = HGLureModule::new();
        module.enabled = true;

        let session = Uuid::new_v4();
        let from = Uuid::new_v4();
        let region = Uuid::new_v4();

        let first = module.handle_incoming_lure(
            session,
            from,
            "Sender",
            region,
            "hi@http://other.grid",
            [0.0, 0.0, 0.0],
        );
        assert!(first);

        let second = module.handle_incoming_lure(
            session,
            from,
            "Sender",
            region,
            "hi@http://other.grid",
            [0.0, 0.0, 0.0],
        );
        assert!(!second);

        assert_eq!(module.pending_count(), 1);
    }

    // -- 7. Is local grid check -----------------------------------------------

    #[test]
    fn is_local_grid_check() {
        let mut module = HGLureModule::new();
        module.gatekeeper_uri = "http://mygrid.com:8002".to_string();

        assert!(module.is_local_grid("http://mygrid.com:8002/"));
        assert!(module.is_local_grid("http://MYGRID.COM:8002"));
        assert!(!module.is_local_grid("http://othergrid.com:8002"));
    }

    // -- 8. Module lifecycle --------------------------------------------------

    #[test]
    fn module_lifecycle() {
        let mut module = HGLureModule::new();
        let mut config = TestConfig::new();
        config.set_string("Messaging", "LureModule", "HGLureModule");
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scenes.len(), 1);

        module.remove_region(&scene).unwrap();
        assert!(module.scenes.is_empty());

        module.close().unwrap();
    }
}
