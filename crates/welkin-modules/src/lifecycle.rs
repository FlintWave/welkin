// SPDX-License-Identifier: LGPL-3.0-or-later

//! Skeleton region-module implementations.

use std::collections::HashMap;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// CapsModule — manages HTTP capabilities per agent
// ---------------------------------------------------------------------------

/// Tracks registered HTTP capabilities for each agent in the region.
///
/// Stub: real implementation would create/destroy HTTP handlers for each cap.
pub struct CapsModule {
    caps: HashMap<Uuid, Vec<String>>,
}

impl CapsModule {
    pub fn new() -> Self {
        Self {
            caps: HashMap::new(),
        }
    }
}

impl Default for CapsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for CapsModule {
    fn name(&self) -> &str {
        "CapsModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("CapsModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "CapsModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "CapsModule detached from region");
        self.caps.clear();
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.caps.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DynAttrsModule — dynamic attributes storage
// ---------------------------------------------------------------------------

/// Dynamic attributes storage stub.
///
/// In OpenSim this manages DAMap (Dynamic Attributes Map) entries on prims and
/// avatars.
pub struct DynAttrsModule;

impl DynAttrsModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DynAttrsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for DynAttrsModule {
    fn name(&self) -> &str {
        "DynAttrsModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("DynAttrsModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "DynAttrsModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "DynAttrsModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ServiceThrottleModule — rate limiting for service calls
// ---------------------------------------------------------------------------

/// Rate-limits outbound service calls to prevent overloading external
/// services.
///
/// Stub: real implementation would maintain token buckets per service
/// endpoint.
pub struct ServiceThrottleModule {
    _enabled: bool,
}

impl ServiceThrottleModule {
    pub fn new() -> Self {
        Self { _enabled: true }
    }
}

impl Default for ServiceThrottleModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for ServiceThrottleModule {
    fn name(&self) -> &str {
        "ServiceThrottleModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self._enabled = config.get_bool("ServiceThrottle", "Enabled", true);
        tracing::info!(enabled = self._enabled, "ServiceThrottleModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "ServiceThrottleModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "ServiceThrottleModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}
