// SPDX-License-Identifier: GPL-3.0-or-later

//! Miscellaneous environment-related region modules: media, region-ready,
//! restart, access control, vegetation, and scene serialisation.

use bitflags::bitflags;
use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ===================================================================
// MediaModule
// ===================================================================

/// A single media entry on a prim face, mirroring the C# `MediaEntry`.
#[derive(Debug, Clone)]
pub struct MediaEntry {
    /// URL of the media stream.
    pub media_url: String,
    /// Whether the media should auto-scale to the face.
    pub auto_scale: bool,
    /// Whether media playback controls are shown.
    pub controls: bool,
    /// Pixel width of the media surface.
    pub width: u32,
    /// Pixel height of the media surface.
    pub height: u32,
    /// MIME type of the media (e.g. `"text/html"`).
    pub media_type: String,
    /// URL whitelist for navigation.
    pub whitelist: Vec<String>,
}

impl Default for MediaEntry {
    fn default() -> Self {
        Self {
            media_url: String::new(),
            auto_scale: true,
            controls: true,
            width: 512,
            height: 512,
            media_type: String::from("text/html"),
            whitelist: Vec::new(),
        }
    }
}

/// Region module managing media-on-a-prim entries.
pub struct MediaModule {
    entries: Vec<MediaEntry>,
}

impl MediaModule {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Returns the current media entries.
    pub fn get_media(&self) -> &[MediaEntry] {
        &self.entries
    }

    /// Sets the media entries for this region.
    pub fn set_media(&mut self, entries: Vec<MediaEntry>) {
        self.entries = entries;
        tracing::debug!(count = self.entries.len(), "media entries updated (stub)");
    }
}

impl Default for MediaModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for MediaModule {
    fn name(&self) -> &str {
        "MediaModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("MediaModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "MediaModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.entries.clear();
        tracing::info!(region = %scene.region_name, "MediaModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.entries.clear();
        Ok(())
    }
}

// ===================================================================
// RegionReadyModule
// ===================================================================

/// Tracks whether the region has completed its startup sequence and is
/// ready to accept connections.
pub struct RegionReadyModule {
    ready: bool,
}

impl RegionReadyModule {
    pub fn new() -> Self {
        Self { ready: false }
    }

    /// Returns whether the region is ready.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Marks the region as ready.
    pub fn set_ready(&mut self) {
        self.ready = true;
        tracing::info!("region marked ready");
    }
}

impl Default for RegionReadyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for RegionReadyModule {
    fn name(&self) -> &str {
        "RegionReadyModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("RegionReadyModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "RegionReadyModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.ready = false;
        tracing::info!(region = %scene.region_name, "RegionReadyModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.ready = false;
        Ok(())
    }
}

// ===================================================================
// RestartModule
// ===================================================================

/// Manages scheduled region restarts.
pub struct RestartModule {
    /// Whether a restart is currently pending.
    restart_pending: bool,
    /// Scheduled restart time (seconds from now when scheduled, 0 when idle).
    restart_seconds: u32,
}

impl RestartModule {
    pub fn new() -> Self {
        Self {
            restart_pending: false,
            restart_seconds: 0,
        }
    }

    /// Schedule a region restart after the given number of seconds.
    pub fn schedule_restart(&mut self, seconds: u32) {
        self.restart_pending = true;
        self.restart_seconds = seconds;
        tracing::info!(seconds, "region restart scheduled");
    }

    /// Abort a pending restart.
    pub fn abort_restart(&mut self) {
        if self.restart_pending {
            self.restart_pending = false;
            self.restart_seconds = 0;
            tracing::info!("region restart aborted");
        }
    }

    /// Returns whether a restart is pending.
    pub fn is_restart_pending(&self) -> bool {
        self.restart_pending
    }

    /// Returns the restart countdown in seconds (0 when no restart is pending).
    pub fn restart_seconds(&self) -> u32 {
        self.restart_seconds
    }
}

impl Default for RestartModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for RestartModule {
    fn name(&self) -> &str {
        "RestartModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("RestartModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "RestartModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.abort_restart();
        tracing::info!(region = %scene.region_name, "RestartModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.abort_restart();
        Ok(())
    }
}

// ===================================================================
// AccessModule
// ===================================================================

bitflags! {
    /// Region-level access control flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RegionAccessFlags: u32 {
        const ALLOW_LANDMARK  = 1 << 0;
        const ALLOW_DIRECT    = 1 << 1;
        const ALLOW_TELEPORT  = 1 << 2;
    }
}

/// Region access-control module.  Checks whether an agent may enter the
/// region based on configured access flags.
pub struct AccessModule {
    flags: RegionAccessFlags,
}

impl AccessModule {
    pub fn new() -> Self {
        Self {
            flags: RegionAccessFlags::ALLOW_LANDMARK
                | RegionAccessFlags::ALLOW_DIRECT
                | RegionAccessFlags::ALLOW_TELEPORT,
        }
    }

    /// Returns the current access flags.
    pub fn access_flags(&self) -> RegionAccessFlags {
        self.flags
    }

    /// Sets the access flags.
    pub fn set_access_flags(&mut self, flags: RegionAccessFlags) {
        self.flags = flags;
        tracing::debug!(?flags, "region access flags updated");
    }

    /// Check whether an agent may enter the region.
    ///
    /// Stub implementation — always returns `true`.  In production this
    /// would consult ban lists, access lists, and estate settings.
    pub fn check_access(&self, _agent_id: Uuid) -> bool {
        tracing::debug!("check_access (stub) — allowing");
        true
    }
}

impl Default for AccessModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for AccessModule {
    fn name(&self) -> &str {
        "AccessModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("AccessModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "AccessModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "AccessModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ===================================================================
// VegetationModule
// ===================================================================

/// Stub vegetation module.  In the C# code base this handles Linden-tree
/// objects; here it is a lifecycle-only placeholder.
pub struct VegetationModule;

impl VegetationModule {
    pub fn new() -> Self {
        Self
    }
}

impl RegionModule for VegetationModule {
    fn name(&self) -> &str {
        "VegetationModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("VegetationModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "VegetationModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "VegetationModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ===================================================================
// SceneSerialiser
// ===================================================================

/// Region-scene serialisation / deserialisation stub.
///
/// In the C# code base this converts the full scene to XML for backup
/// and OAR archives.  Here it provides the structural entry points.
pub struct SceneSerialiser;

impl SceneSerialiser {
    pub fn new() -> Self {
        Self
    }

    /// Serialize the region to a byte buffer.
    ///
    /// Stub — returns an empty buffer.
    pub fn serialize_region(&self) -> Vec<u8> {
        tracing::debug!("serialize_region (stub)");
        Vec::new()
    }

    /// Deserialize region data from a byte buffer.
    ///
    /// Stub — logs and discards the data.
    pub fn deserialize_region(&self, data: &[u8]) {
        tracing::debug!(bytes = data.len(), "deserialize_region (stub)");
    }
}

impl RegionModule for SceneSerialiser {
    fn name(&self) -> &str {
        "SceneSerialiser"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("SceneSerialiser initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "SceneSerialiser attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "SceneSerialiser detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}
