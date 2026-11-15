// SPDX-License-Identifier: GPL-3.0-or-later

//! Prim-count tracking per parcel.
//!
//! Mirrors the prim-accounting logic scattered across `LandManagementModule`
//! and the various `IPrimCounts` implementations in the C# codebase.

use std::collections::HashMap;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

/// Prim counts broken out by ownership category.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrimCounts {
    /// Prims currently selected by users.
    pub selected: i32,
    /// Total prims on the parcel.
    pub total: i32,
    /// Prims owned by the parcel owner.
    pub owner: i32,
    /// Prims owned by the parcel's group.
    pub group: i32,
    /// Prims owned by others (not owner, not group).
    pub other: i32,
    /// Sum of prims across the whole simulator for this owner.
    pub simulator_total: i32,
}

/// Tracks per-parcel primitive counts, subscribing to scene events to keep
/// them up to date.
pub struct PrimCountModule {
    counts: HashMap<i32, PrimCounts>,
}

impl PrimCountModule {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Get the current prim counts for a parcel.  Returns the default
    /// (all-zero) counts if the parcel is unknown.
    pub fn get_counts(&self, parcel_local_id: i32) -> PrimCounts {
        self.counts
            .get(&parcel_local_id)
            .copied()
            .unwrap_or_default()
    }

    /// Force a recount by resetting the stored counts for a parcel to zero.
    ///
    /// A real implementation would walk the scene graph and re-tally; this
    /// stub simply zeroes the entry.
    pub fn recount(&mut self, parcel_local_id: i32) {
        self.counts.insert(parcel_local_id, PrimCounts::default());
    }

    /// Increment the total count for a parcel (used when an object is added).
    pub fn increment(&mut self, parcel_local_id: i32, delta: i32) {
        let entry = self.counts.entry(parcel_local_id).or_default();
        entry.total += delta;
    }

    /// Decrement the total count for a parcel (used when an object is
    /// removed).
    pub fn decrement(&mut self, parcel_local_id: i32, delta: i32) {
        let entry = self.counts.entry(parcel_local_id).or_default();
        entry.total = (entry.total - delta).max(0);
    }
}

impl Default for PrimCountModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for PrimCountModule {
    fn name(&self) -> &str {
        "PrimCountModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("PrimCountModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(
            region = %scene.region_name,
            "PrimCountModule attached to region"
        );
        // A full implementation would subscribe to
        // scene.events.on_object_added / on_object_removed here.
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(
            region = %scene.region_name,
            "PrimCountModule detached from region"
        );
        self.counts.clear();
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.counts.clear();
        Ok(())
    }
}
