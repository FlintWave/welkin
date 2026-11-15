// SPDX-License-Identifier: LGPL-3.0-or-later

//! Land management module — the main module that owns all parcels in a
//! region and presents viewer-protocol stubs.
//!
//! Mirrors `OpenSim.Region.CoreModules.World.Land.LandManagementModule`.

use std::collections::HashMap;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

use super::data::{AccessList, LandAccessEntry, LandData, LandDataUpdate};
use super::grid::{LAND_UNIT, ParcelGrid};
use super::parcel::LandParcel;

/// Manages all land parcels within a single region.
pub struct LandManagementModule {
    /// All parcels keyed by their `local_id`.
    parcels: HashMap<i32, LandParcel>,
    /// The spatial grid mapping cells to parcel IDs.
    grid: ParcelGrid,
    /// Counter for assigning the next `local_id`.
    next_local_id: i32,
    /// Region edge size in metres (read from config or default 256).
    region_size: u32,
}

impl LandManagementModule {
    pub fn new() -> Self {
        Self {
            parcels: HashMap::new(),
            grid: ParcelGrid::default(),
            next_local_id: 1,
            region_size: 256,
        }
    }

    // -- public API ----------------------------------------------------------

    /// Add a parcel.  Assigns a new `local_id`, updates grid cells from the
    /// parcel's bitmap, and returns the assigned `local_id`.
    pub fn add_parcel(&mut self, mut data: LandData) -> i32 {
        let id = self.next_local_id;
        self.next_local_id += 1;
        data.local_id = id;

        let mut parcel = LandParcel::new(data, self.region_size);
        // If the bitmap bytes are populated, decode them into the 2-D bitmap.
        if parcel.data.bitmap.iter().any(|&b| b != 0) {
            parcel.sync_data_to_bitmap(self.region_size);
        }

        // Paint grid cells owned by this parcel.
        self.paint_grid_from_bitmap(&parcel);
        self.parcels.insert(id, parcel);
        id
    }

    /// Remove a parcel by local_id.  Cells that pointed to this parcel are
    /// reset to 0.
    pub fn remove_parcel(&mut self, local_id: i32) {
        if let Some(parcel) = self.parcels.remove(&local_id) {
            self.clear_grid_from_bitmap(&parcel, local_id);
        }
    }

    /// Get the parcel at a world-space position (metres).
    pub fn get_parcel_at(&self, x: f32, y: f32) -> Option<&LandParcel> {
        let id = self.grid.parcels_at_point(x, y);
        if id == 0 {
            return None;
        }
        self.parcels.get(&id)
    }

    /// Get a parcel by its local_id.
    pub fn get_parcel(&self, local_id: i32) -> Option<&LandParcel> {
        self.parcels.get(&local_id)
    }

    /// Subdivide: carve a rectangle out of existing parcel(s) and create a
    /// new parcel for it, owned by `owner_id`.
    ///
    /// Coordinates are in metres and must be aligned to 4 m boundaries.
    pub fn subdivide(
        &mut self,
        start_x: u32,
        start_y: u32,
        end_x: u32,
        end_y: u32,
        owner_id: Uuid,
    ) -> Result<i32, ModuleError> {
        if start_x >= end_x || start_y >= end_y {
            return Err(ModuleError::Other("invalid subdivide rectangle".into()));
        }

        let new_bitmap =
            LandParcel::get_square_bitmap(self.region_size, start_x, start_y, end_x, end_y);

        // Remove the new rectangle from every existing parcel that overlaps.
        let dim = (self.region_size / LAND_UNIT) as usize;
        let sx = (start_x / LAND_UNIT) as usize;
        let sy = (start_y / LAND_UNIT) as usize;
        let ex = ((end_x / LAND_UNIT) as usize).min(dim);
        let ey = ((end_y / LAND_UNIT) as usize).min(dim);

        for parcel in self.parcels.values_mut() {
            for gx in sx..ex {
                for gy in sy..ey {
                    if parcel
                        .land_bitmap
                        .get(gx)
                        .and_then(|c| c.get(gy))
                        .copied()
                        .unwrap_or(false)
                    {
                        parcel.land_bitmap[gx][gy] = false;
                    }
                }
            }
            parcel.sync_bitmap_to_data();
            parcel.data.area = parcel.area();
        }

        // Create the new parcel.
        let mut data = LandData::default();
        data.owner_id = owner_id;
        data.name = "Subdivision".to_string();
        data.bitmap = LandParcel::bitmap_to_bytes(&new_bitmap);

        let id = self.next_local_id;
        self.next_local_id += 1;
        data.local_id = id;

        let mut parcel = LandParcel::new(data, self.region_size);
        parcel.land_bitmap = new_bitmap;
        parcel.sync_bitmap_to_data();
        parcel.data.area = parcel.area();

        self.paint_grid_from_bitmap(&parcel);
        self.parcels.insert(id, parcel);
        Ok(id)
    }

    /// Join multiple parcels into one.  All parcels must share the same
    /// `owner_id`.  Returns the local_id of the merged parcel.
    pub fn join(&mut self, parcel_ids: &[i32], owner_id: Uuid) -> Result<i32, ModuleError> {
        if parcel_ids.is_empty() {
            return Err(ModuleError::Other("no parcels to join".into()));
        }

        // Verify ownership.
        for &pid in parcel_ids {
            let p = self
                .parcels
                .get(&pid)
                .ok_or_else(|| ModuleError::NotFound(format!("parcel {pid}")))?;
            if p.data.owner_id != owner_id {
                return Err(ModuleError::Other(format!(
                    "parcel {pid} not owned by {owner_id}"
                )));
            }
        }

        // Merge bitmaps.
        let dim = (self.region_size / LAND_UNIT) as usize;
        let mut merged = vec![vec![false; dim]; dim];
        for &pid in parcel_ids {
            if let Some(p) = self.parcels.get(&pid) {
                merged = LandParcel::merge_bitmaps(&merged, &p.land_bitmap);
            }
        }

        // Remove old parcels.
        for &pid in parcel_ids {
            self.parcels.remove(&pid);
        }

        // Create new parcel.
        let mut data = LandData::default();
        data.owner_id = owner_id;
        data.name = "Merged Parcel".to_string();
        data.bitmap = LandParcel::bitmap_to_bytes(&merged);

        let id = self.next_local_id;
        self.next_local_id += 1;
        data.local_id = id;

        let mut parcel = LandParcel::new(data, self.region_size);
        parcel.land_bitmap = merged;
        parcel.sync_bitmap_to_data();
        parcel.data.area = parcel.area();

        self.paint_grid_from_bitmap(&parcel);
        self.parcels.insert(id, parcel);
        Ok(id)
    }

    /// Apply a viewer-originated parcel property update.
    pub fn update_parcel_info(&mut self, local_id: i32, update: LandDataUpdate) {
        if let Some(parcel) = self.parcels.get_mut(&local_id) {
            parcel.data.apply_update(&update);
        }
    }

    /// Replace the access list for a parcel.
    pub fn update_access_list(&mut self, local_id: i32, entries: Vec<LandAccessEntry>) {
        if let Some(parcel) = self.parcels.get_mut(&local_id) {
            parcel.data.access_list = entries;
        }
    }

    /// Is the given agent banned from the parcel?
    pub fn is_banned(&self, local_id: i32, agent_id: Uuid) -> bool {
        self.parcels
            .get(&local_id)
            .map(|p| {
                p.data.access_list.iter().any(|e| {
                    e.agent_id == agent_id
                        && e.flags == AccessList::Ban
                        && (e.expires == 0 || e.expires > current_epoch())
                })
            })
            .unwrap_or(false)
    }

    /// Is the parcel access-restricted (has an access list) and the agent is
    /// not on it?
    pub fn is_restricted(&self, local_id: i32, agent_id: Uuid) -> bool {
        self.parcels
            .get(&local_id)
            .map(|p| {
                let has_access_list = p
                    .data
                    .access_list
                    .iter()
                    .any(|e| e.flags == AccessList::Access);
                if !has_access_list {
                    return false; // no access restriction
                }
                // Agent must be on the access list.
                !p.data.access_list.iter().any(|e| {
                    e.agent_id == agent_id
                        && e.flags == AccessList::Access
                        && (e.expires == 0 || e.expires > current_epoch())
                })
            })
            .unwrap_or(false)
    }

    /// Can an agent be on this parcel?  True unless banned or restricted.
    pub fn can_be_on_land(&self, local_id: i32, agent_id: Uuid) -> bool {
        !self.is_banned(local_id, agent_id) && !self.is_restricted(local_id, agent_id)
    }

    /// Number of parcels currently managed.
    pub fn parcel_count(&self) -> usize {
        self.parcels.len()
    }

    /// Reference to the underlying grid.
    pub fn grid(&self) -> &ParcelGrid {
        &self.grid
    }

    // -- private helpers -----------------------------------------------------

    fn paint_grid_from_bitmap(&mut self, parcel: &LandParcel) {
        let dim = self.grid.grid_size() as usize;
        for gx in 0..dim {
            for gy in 0..dim {
                if parcel.contains_point(gx, gy) {
                    self.grid
                        .set_parcel_id(gx as u32, gy as u32, parcel.data.local_id);
                }
            }
        }
    }

    fn clear_grid_from_bitmap(&mut self, parcel: &LandParcel, local_id: i32) {
        let dim = self.grid.grid_size() as usize;
        for gx in 0..dim {
            for gy in 0..dim {
                if parcel.contains_point(gx, gy)
                    && self.grid.get_parcel_id(gx as u32, gy as u32) == local_id
                {
                    self.grid.set_parcel_id(gx as u32, gy as u32, 0);
                }
            }
        }
    }

    /// Create the default full-region parcel.
    fn create_default_parcel(&mut self, owner_id: Uuid) {
        let mut data = LandData::default();
        data.owner_id = owner_id;
        data.name = "Your Parcel".to_string();
        let full = LandParcel::basic_full_region_bitmap(self.region_size);
        data.bitmap = LandParcel::bitmap_to_bytes(&full);

        let id = self.next_local_id;
        self.next_local_id += 1;
        data.local_id = id;

        let mut parcel = LandParcel::new(data, self.region_size);
        parcel.land_bitmap = full;
        parcel.data.area = parcel.area();

        self.grid.fill(id);
        self.parcels.insert(id, parcel);
    }
}

impl Default for LandManagementModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for LandManagementModule {
    fn name(&self) -> &str {
        "LandManagementModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.region_size = config.get_int("Region", "RegionSize", 256) as u32;
        self.grid = ParcelGrid::new(self.region_size);
        tracing::info!(
            region_size = self.region_size,
            "LandManagementModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(
            region = %scene.region_name,
            "LandManagementModule: creating default full-region parcel"
        );
        // The initial parcel is owned by UUID::nil (estate / governor).
        self.create_default_parcel(Uuid::nil());
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(
            region = %scene.region_name,
            "LandManagementModule: removing parcels"
        );
        self.parcels.clear();
        self.grid = ParcelGrid::new(self.region_size);
        self.next_local_id = 1;
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.parcels.clear();
        Ok(())
    }
}

/// Placeholder epoch helper (seconds since Unix epoch).
fn current_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
