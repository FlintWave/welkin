// SPDX-License-Identifier: GPL-3.0-or-later

//! Core module trait, types, and configuration interface.

use std::fmt;

/// Describes how a module is instantiated relative to regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleType {
    /// Loaded per-region, one instance per region.
    NonShared,
    /// One instance shared across all regions.
    Shared,
}

impl fmt::Display for ModuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleType::NonShared => write!(f, "NonShared"),
            ModuleType::Shared => write!(f, "Shared"),
        }
    }
}

/// Errors arising from module operations.
#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    #[error("initialisation failed: {0}")]
    InitFailed(String),

    #[error("module not found: {0}")]
    NotFound(String),

    #[error("module already registered: {0}")]
    AlreadyRegistered(String),

    #[error("{0}")]
    Other(String),
}

/// Read-only configuration source for module initialisation.
///
/// Mirrors the C# `IConfigSource` pattern: sections contain key-value pairs
/// with typed accessors that return a default when the key is absent.
pub trait ModuleConfig: Send + Sync {
    fn get_string(&self, section: &str, key: &str, default: &str) -> String;
    fn get_int(&self, section: &str, key: &str, default: i64) -> i64;
    fn get_bool(&self, section: &str, key: &str, default: bool) -> bool;
    fn get_float(&self, section: &str, key: &str, default: f64) -> f64;
}

/// A region module that participates in the scene lifecycle.
///
/// This is the Rust equivalent of OpenSim's `IRegionModuleBase` plus
/// `ISharedRegionModule` / `INonSharedRegionModule`.  Modules are registered
/// in a [`super::registry::ModuleRegistry`] and driven through the lifecycle
/// methods in order:
///
/// 1. `initialise` — called once with the configuration source.
/// 2. `add_region`  — called for each scene the module should attach to.
/// 3. (module is active)
/// 4. `remove_region` — called when a scene is shutting down.
/// 5. `close` — final cleanup.
pub trait RegionModule: Send + Sync + 'static {
    /// Human-readable name for logging and diagnostics.
    fn name(&self) -> &str;

    /// Whether this module is shared across regions or per-region.
    fn module_type(&self) -> ModuleType;

    /// One-time initialisation with the configuration source.
    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError>;

    /// Attach to a scene.
    fn add_region(&mut self, scene: &super::registry::SceneRef) -> Result<(), ModuleError>;

    /// Detach from a scene.
    fn remove_region(&mut self, scene: &super::registry::SceneRef) -> Result<(), ModuleError>;

    /// Final cleanup before the module is dropped.
    fn close(&mut self) -> Result<(), ModuleError>;
}
