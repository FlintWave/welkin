// SPDX-License-Identifier: GPL-3.0-or-later

//! Dwell tracking module — records how long avatars spend on each parcel.
//!
//! This is a stub matching the `IDwellModule` interface in C# OpenSim.

use std::collections::HashMap;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

/// Simple dwell-score tracker keyed by parcel `local_id`.
pub struct DwellModule {
    dwell_data: HashMap<i32, f32>,
}

impl DwellModule {
    pub fn new() -> Self {
        Self {
            dwell_data: HashMap::new(),
        }
    }

    /// Get the current dwell score for a parcel.
    pub fn get_dwell(&self, parcel_local_id: i32) -> f32 {
        self.dwell_data
            .get(&parcel_local_id)
            .copied()
            .unwrap_or(0.0)
    }

    /// Add `delta` to the dwell score of a parcel.
    pub fn update_dwell(&mut self, parcel_local_id: i32, delta: f32) {
        let entry = self.dwell_data.entry(parcel_local_id).or_insert(0.0);
        *entry += delta;
    }
}

impl Default for DwellModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for DwellModule {
    fn name(&self) -> &str {
        "DwellModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("DwellModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(
            region = %scene.region_name,
            "DwellModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(
            region = %scene.region_name,
            "DwellModule detached from region"
        );
        self.dwell_data.clear();
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.dwell_data.clear();
        Ok(())
    }
}
