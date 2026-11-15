// SPDX-License-Identifier: LGPL-3.0-or-later

//! WebRTC voice module: voice channel management via wolfvoice sidecar.

pub mod capabilities;
pub mod docker;
pub mod ossl_integration;
pub mod portal_integration;
pub mod wolfvoice_client;

use std::collections::HashMap;
use std::time::Instant;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;
use wolfvoice_client::{ChannelInfo, ChannelType};

// ---------------------------------------------------------------------------
// IVoiceModule trait
// ---------------------------------------------------------------------------

/// Trait for voice module implementations, mirroring C#'s IVoiceModule
/// interface. Allows other modules (e.g. land management) to configure
/// voice channels for parcels.
pub trait IVoiceModule: Send + Sync {
    /// Associate a SIP address with a parcel, overriding the default
    /// wolfvoice-assigned channel.
    fn set_land_sip_address(&mut self, parcel_global_id: Uuid, sip_address: &str);
}

// ---------------------------------------------------------------------------
// WebRtcVoiceConfig
// ---------------------------------------------------------------------------

/// Configuration loaded from the `[Voice]` and `[WebRtcVoice]` INI sections.
#[derive(Debug, Clone)]
pub struct WebRtcVoiceConfig {
    /// Base URL of the wolfvoice sidecar (default `http://localhost:8090`).
    pub wolfvoice_endpoint: String,
    /// TURN relay server address.
    pub turn_server: String,
    /// TURN credential username.
    pub turn_username: String,
    /// TURN credential password.
    pub turn_credential: String,
    /// Whether the voice module is enabled (default `false`).
    pub enabled: bool,
    /// Account provisioning API path (default `/api/provision`).
    pub voice_account_api: String,
    /// Channel management API path (default `/api/channel`).
    pub voice_channel_api: String,
}

impl Default for WebRtcVoiceConfig {
    fn default() -> Self {
        Self {
            wolfvoice_endpoint: "http://localhost:8090".to_string(),
            turn_server: String::new(),
            turn_username: String::new(),
            turn_credential: String::new(),
            enabled: false,
            voice_account_api: "/api/provision".to_string(),
            voice_channel_api: "/api/channel".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// ActiveChannel
// ---------------------------------------------------------------------------

/// An active voice channel associated with a parcel.
#[derive(Debug)]
pub struct ActiveChannel {
    /// Channel metadata returned by the wolfvoice sidecar.
    pub channel_info: ChannelInfo,
    /// When this channel was created.
    pub created_at: Instant,
    /// Number of viewers currently connected to this channel.
    pub viewer_count: usize,
}

// ---------------------------------------------------------------------------
// WebRtcVoiceModule
// ---------------------------------------------------------------------------

/// Region module that manages WebRTC voice channels through a wolfvoice
/// sidecar process. One channel is created per parcel that has voice enabled.
pub struct WebRtcVoiceModule {
    /// Module configuration loaded from INI.
    config: WebRtcVoiceConfig,
    /// The scene this module is attached to.
    scene: Option<SceneRef>,
    /// Active voice channels keyed by parcel local ID.
    active_channels: HashMap<i32, ActiveChannel>,
    /// Per-parcel SIP address overrides keyed by parcel global UUID.
    parcel_sip_overrides: HashMap<Uuid, String>,
}

impl WebRtcVoiceModule {
    /// Create a new module with default configuration.
    pub fn new() -> Self {
        Self {
            config: WebRtcVoiceConfig::default(),
            scene: None,
            active_channels: HashMap::new(),
            parcel_sip_overrides: HashMap::new(),
        }
    }

    /// Read-only access to the module configuration.
    pub fn config(&self) -> &WebRtcVoiceConfig {
        &self.config
    }

    /// Create a voice channel for the given parcel.
    ///
    /// The channel is stored in `active_channels` and can be retrieved with
    /// [`get_channel`](Self::get_channel).
    pub fn create_channel(
        &mut self,
        parcel_local_id: i32,
        channel_name: &str,
        channel_type: ChannelType,
    ) -> Result<&ActiveChannel, ModuleError> {
        let info = ChannelInfo {
            channel_uri: format!(
                "{}/channel/{}/{}",
                self.config.wolfvoice_endpoint,
                channel_type_str(channel_type),
                channel_name,
            ),
            channel_name: channel_name.to_string(),
        };
        let channel = ActiveChannel {
            channel_info: info,
            created_at: Instant::now(),
            viewer_count: 0,
        };
        self.active_channels.insert(parcel_local_id, channel);
        tracing::debug!(
            parcel_local_id,
            channel_name,
            "WebRtcVoiceModule: created voice channel"
        );
        Ok(self
            .active_channels
            .get(&parcel_local_id)
            .expect("just inserted"))
    }

    /// Destroy the voice channel for a given parcel.
    pub fn destroy_channel(&mut self, parcel_local_id: i32) -> Result<(), ModuleError> {
        match self.active_channels.remove(&parcel_local_id) {
            Some(ch) => {
                tracing::debug!(
                    parcel_local_id,
                    channel = %ch.channel_info.channel_name,
                    "WebRtcVoiceModule: destroyed voice channel"
                );
                Ok(())
            }
            None => Err(ModuleError::NotFound(format!(
                "no voice channel for parcel {}",
                parcel_local_id
            ))),
        }
    }

    /// Destroy all active voice channels.
    pub fn destroy_all_channels(&mut self) {
        let count = self.active_channels.len();
        self.active_channels.clear();
        tracing::debug!(count, "WebRtcVoiceModule: destroyed all voice channels");
    }

    /// Look up the active channel for a parcel.
    pub fn get_channel(&self, parcel_local_id: i32) -> Option<&ActiveChannel> {
        self.active_channels.get(&parcel_local_id)
    }

    /// Enable or disable voice for a parcel. When disabled, the existing
    /// channel (if any) is destroyed.
    pub fn set_parcel_voice_enabled(&mut self, parcel_local_id: i32, enabled: bool) {
        if !enabled && self.active_channels.remove(&parcel_local_id).is_some() {
            tracing::debug!(
                parcel_local_id,
                "WebRtcVoiceModule: voice disabled for parcel, channel removed"
            );
        }
    }

    /// Number of active voice channels.
    pub fn active_channel_count(&self) -> usize {
        self.active_channels.len()
    }

    /// Total viewers across all active channels.
    pub fn total_viewer_count(&self) -> usize {
        self.active_channels
            .values()
            .map(|ch| ch.viewer_count)
            .sum()
    }

    /// Increment the viewer count on a parcel's voice channel.
    pub fn increment_viewers(&mut self, parcel_local_id: i32) {
        if let Some(ch) = self.active_channels.get_mut(&parcel_local_id) {
            ch.viewer_count += 1;
        }
    }

    /// Decrement the viewer count on a parcel's voice channel.
    pub fn decrement_viewers(&mut self, parcel_local_id: i32) {
        if let Some(ch) = self.active_channels.get_mut(&parcel_local_id) {
            ch.viewer_count = ch.viewer_count.saturating_sub(1);
        }
    }

    /// Look up a SIP override for a parcel global ID.
    pub fn get_sip_override(&self, parcel_global_id: Uuid) -> Option<&String> {
        self.parcel_sip_overrides.get(&parcel_global_id)
    }

    /// Remove a SIP address override.  Returns `true` if one was present.
    pub fn remove_sip_override(&mut self, parcel_global_id: Uuid) -> bool {
        self.parcel_sip_overrides
            .remove(&parcel_global_id)
            .is_some()
    }
}

impl Default for WebRtcVoiceModule {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IVoiceModule implementation
// ---------------------------------------------------------------------------

impl IVoiceModule for WebRtcVoiceModule {
    fn set_land_sip_address(&mut self, parcel_global_id: Uuid, sip_address: &str) {
        tracing::debug!(
            parcel = %parcel_global_id,
            sip = sip_address,
            "WebRtcVoiceModule: SIP address override set"
        );
        self.parcel_sip_overrides
            .insert(parcel_global_id, sip_address.to_string());
    }
}

// ---------------------------------------------------------------------------
// RegionModule implementation
// ---------------------------------------------------------------------------

impl RegionModule for WebRtcVoiceModule {
    fn name(&self) -> &str {
        "WebRtcVoiceModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        // [Voice] section -- shared voice settings.
        self.config.enabled = config.get_bool("Voice", "enabled", false);

        // [WebRtcVoice] section -- module-specific settings.
        self.config.wolfvoice_endpoint =
            config.get_string("WebRtcVoice", "wolfvoice_endpoint", "http://localhost:8090");
        self.config.turn_server = config.get_string("WebRtcVoice", "turn_server", "");
        self.config.turn_username = config.get_string("WebRtcVoice", "turn_username", "");
        self.config.turn_credential = config.get_string("WebRtcVoice", "turn_credential", "");
        self.config.voice_account_api =
            config.get_string("WebRtcVoice", "voice_account_api", "/api/provision");
        self.config.voice_channel_api =
            config.get_string("WebRtcVoice", "voice_channel_api", "/api/channel");

        tracing::info!(
            enabled = self.config.enabled,
            endpoint = %self.config.wolfvoice_endpoint,
            "WebRtcVoiceModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        tracing::info!(
            region = %scene.region_name,
            "WebRtcVoiceModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.active_channels.clear();
        self.scene = None;
        tracing::info!(
            region = %scene.region_name,
            "WebRtcVoiceModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.active_channels.clear();
        self.parcel_sip_overrides.clear();
        self.scene = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a `ChannelType` enum to a URL-safe string segment.
fn channel_type_str(ct: ChannelType) -> &'static str {
    match ct {
        ChannelType::Parcel => "parcel",
        ChannelType::Estate => "estate",
        ChannelType::Group => "group",
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::ModuleConfig;
    use std::collections::HashMap as StdHashMap;

    // -- TestConfig (mirrors concierge pattern) ------------------------------

    struct TestConfig {
        strings: StdHashMap<(String, String), String>,
        bools: StdHashMap<(String, String), bool>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: StdHashMap::new(),
                bools: StdHashMap::new(),
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

    fn test_scene() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    fn setup_module() -> WebRtcVoiceModule {
        let mut module = WebRtcVoiceModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Voice", "enabled", true);
        module.initialise(&config).unwrap();
        let scene = test_scene();
        module.add_region(&scene).unwrap();
        module
    }

    // -- Test 1: Module lifecycle (init, add_region, remove_region, close) ---

    #[test]
    fn module_lifecycle() {
        let mut module = WebRtcVoiceModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Voice", "enabled", true);
        let scene = test_scene();

        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "WebRtcVoiceModule");
        assert_eq!(module.module_type(), ModuleType::Shared);

        // Create a channel, then remove_region clears it.
        module
            .create_channel(1, "test-channel", ChannelType::Parcel)
            .unwrap();
        assert_eq!(module.active_channel_count(), 1);

        module.remove_region(&scene).unwrap();
        assert_eq!(module.active_channel_count(), 0);
        assert!(module.scene.is_none());

        module.close().unwrap();
    }

    // -- Test 2: Config loading from INI sections ----------------------------

    #[test]
    fn config_loading_from_ini() {
        let mut module = WebRtcVoiceModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Voice", "enabled", true);
        config.set_string("WebRtcVoice", "wolfvoice_endpoint", "http://voice:9090");
        config.set_string("WebRtcVoice", "turn_server", "turn:example.com:3478");
        config.set_string("WebRtcVoice", "turn_username", "user1");
        config.set_string("WebRtcVoice", "turn_credential", "pass1");
        config.set_string("WebRtcVoice", "voice_account_api", "/api/v2/provision");
        config.set_string("WebRtcVoice", "voice_channel_api", "/api/v2/channel");

        module.initialise(&config).unwrap();

        assert!(module.config.enabled);
        assert_eq!(module.config.wolfvoice_endpoint, "http://voice:9090");
        assert_eq!(module.config.turn_server, "turn:example.com:3478");
        assert_eq!(module.config.turn_username, "user1");
        assert_eq!(module.config.turn_credential, "pass1");
        assert_eq!(module.config.voice_account_api, "/api/v2/provision");
        assert_eq!(module.config.voice_channel_api, "/api/v2/channel");
    }

    // -- Test 3: Config defaults when not set ---------------------------------

    #[test]
    fn config_defaults_when_not_set() {
        let mut module = WebRtcVoiceModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();

        assert!(!module.config.enabled);
        assert_eq!(module.config.wolfvoice_endpoint, "http://localhost:8090");
        assert_eq!(module.config.turn_server, "");
        assert_eq!(module.config.turn_username, "");
        assert_eq!(module.config.turn_credential, "");
        assert_eq!(module.config.voice_account_api, "/api/provision");
        assert_eq!(module.config.voice_channel_api, "/api/channel");
    }

    // -- Test 4: set_land_sip_address stores and retrieves --------------------

    #[test]
    fn set_land_sip_address_stores_and_retrieves() {
        let mut module = setup_module();
        let parcel_id = Uuid::new_v4();
        let sip = "sip:parcel@voice.example.com";

        module.set_land_sip_address(parcel_id, sip);
        assert_eq!(module.get_sip_override(parcel_id), Some(&sip.to_string()));

        // Unknown parcel returns None.
        assert_eq!(module.get_sip_override(Uuid::new_v4()), None);
    }

    // -- Test 5: create_channel adds to active_channels -----------------------

    #[test]
    fn create_channel_adds_to_active() {
        let mut module = setup_module();
        let ch = module
            .create_channel(42, "parcel-voice", ChannelType::Parcel)
            .unwrap();
        assert_eq!(ch.channel_info.channel_name, "parcel-voice");
        assert!(ch.channel_info.channel_uri.contains("parcel"));
        assert_eq!(ch.viewer_count, 0);
        assert_eq!(module.active_channel_count(), 1);
    }

    // -- Test 6: destroy_channel removes from active_channels -----------------

    #[test]
    fn destroy_channel_removes_from_active() {
        let mut module = setup_module();
        module
            .create_channel(10, "to-destroy", ChannelType::Estate)
            .unwrap();
        assert_eq!(module.active_channel_count(), 1);

        module.destroy_channel(10).unwrap();
        assert_eq!(module.active_channel_count(), 0);
        assert!(module.get_channel(10).is_none());

        // Destroying a non-existent channel returns an error.
        assert!(module.destroy_channel(10).is_err());
    }

    // -- Test 7: destroy_all_channels clears everything -----------------------

    #[test]
    fn destroy_all_channels_clears() {
        let mut module = setup_module();
        module
            .create_channel(1, "ch-1", ChannelType::Parcel)
            .unwrap();
        module
            .create_channel(2, "ch-2", ChannelType::Estate)
            .unwrap();
        module
            .create_channel(3, "ch-3", ChannelType::Group)
            .unwrap();
        assert_eq!(module.active_channel_count(), 3);

        module.destroy_all_channels();
        assert_eq!(module.active_channel_count(), 0);
    }

    // -- Test 8: set_parcel_voice_enabled disables channel --------------------

    #[test]
    fn set_parcel_voice_enabled_disables() {
        let mut module = setup_module();
        module
            .create_channel(5, "voice-ch", ChannelType::Parcel)
            .unwrap();
        assert_eq!(module.active_channel_count(), 1);

        // Disabling removes the channel.
        module.set_parcel_voice_enabled(5, false);
        assert!(module.get_channel(5).is_none());
        assert_eq!(module.active_channel_count(), 0);

        // Enabling a non-existent channel does nothing (no panic).
        module.set_parcel_voice_enabled(5, true);
        assert_eq!(module.active_channel_count(), 0);
    }

    // -- Test 9: viewer_count increment/decrement -----------------------------

    #[test]
    fn viewer_count_increment_decrement() {
        let mut module = setup_module();
        module
            .create_channel(7, "viewers-ch", ChannelType::Parcel)
            .unwrap();

        module.increment_viewers(7);
        module.increment_viewers(7);
        module.increment_viewers(7);
        assert_eq!(module.get_channel(7).unwrap().viewer_count, 3);

        module.decrement_viewers(7);
        assert_eq!(module.get_channel(7).unwrap().viewer_count, 2);

        // Decrement past zero saturates at 0.
        module.decrement_viewers(7);
        module.decrement_viewers(7);
        module.decrement_viewers(7);
        assert_eq!(module.get_channel(7).unwrap().viewer_count, 0);

        // Increment/decrement on non-existent channel is a no-op.
        module.increment_viewers(999);
        module.decrement_viewers(999);
    }

    // -- Test 10: active_channel_count and total_viewer_count ------------------

    #[test]
    fn active_and_total_viewer_counts() {
        let mut module = setup_module();
        assert_eq!(module.active_channel_count(), 0);
        assert_eq!(module.total_viewer_count(), 0);

        module
            .create_channel(1, "ch-a", ChannelType::Parcel)
            .unwrap();
        module
            .create_channel(2, "ch-b", ChannelType::Estate)
            .unwrap();
        assert_eq!(module.active_channel_count(), 2);

        module.increment_viewers(1);
        module.increment_viewers(1);
        module.increment_viewers(2);
        module.increment_viewers(2);
        module.increment_viewers(2);
        assert_eq!(module.total_viewer_count(), 5);

        // Destroy one channel; total adjusts.
        module.destroy_channel(1).unwrap();
        assert_eq!(module.active_channel_count(), 1);
        assert_eq!(module.total_viewer_count(), 3);
    }

    // -- Test 11: close clears all state --------------------------------------

    #[test]
    fn close_clears_all_state() {
        let mut module = setup_module();
        let parcel_id = Uuid::new_v4();
        module.set_land_sip_address(parcel_id, "sip:test@example.com");
        module.create_channel(1, "ch", ChannelType::Parcel).unwrap();

        module.close().unwrap();

        assert_eq!(module.active_channel_count(), 0);
        assert!(module.get_sip_override(parcel_id).is_none());
        assert!(module.scene.is_none());
    }

    // -- Test 12: channel_type_str helper ------------------------------------

    #[test]
    fn channel_type_str_values() {
        assert_eq!(channel_type_str(ChannelType::Parcel), "parcel");
        assert_eq!(channel_type_str(ChannelType::Estate), "estate");
        assert_eq!(channel_type_str(ChannelType::Group), "group");
    }
}
