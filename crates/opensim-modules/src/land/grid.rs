// SPDX-License-Identifier: GPL-3.0-or-later

//! Parcel grid — maps every 4 m x 4 m cell in the region to a parcel
//! `local_id`.
//!
//! Mirrors the grid maintained by `LandManagementModule` in the C#
//! implementation.

/// The side length in metres of a single parcel-grid cell (the land unit).
pub const LAND_UNIT: u32 = 4;

/// A flat grid of parcel IDs, one per 4 m x 4 m cell in the region.
#[derive(Debug, Clone)]
pub struct ParcelGrid {
    /// Flat row-major array: `cells[y * grid_size + x]`.
    cells: Vec<i32>,
    /// Region edge length in metres (default 256).
    region_size: u32,
    /// Number of cells along one axis = `region_size / LAND_UNIT`.
    grid_size: u32,
}

impl ParcelGrid {
    /// Create a grid for a region of the given size (in metres).
    ///
    /// All cells are initialised to parcel id `0` (no parcel).
    pub fn new(region_size: u32) -> Self {
        let grid_size = region_size / LAND_UNIT;
        Self {
            cells: vec![0i32; (grid_size * grid_size) as usize],
            region_size,
            grid_size,
        }
    }

    /// Region edge length in metres.
    pub fn region_size(&self) -> u32 {
        self.region_size
    }

    /// Number of cells along one axis.
    pub fn grid_size(&self) -> u32 {
        self.grid_size
    }

    /// Convert metre coordinates to grid-cell indices, clamping to the valid
    /// range.
    fn to_cell(&self, x: f32, y: f32) -> (u32, u32) {
        let cx = (x as u32 / LAND_UNIT).min(self.grid_size.saturating_sub(1));
        let cy = (y as u32 / LAND_UNIT).min(self.grid_size.saturating_sub(1));
        (cx, cy)
    }

    /// Get the parcel `local_id` for the cell at grid indices `(gx, gy)`.
    ///
    /// Returns `0` if the indices are out of range.
    pub fn get_parcel_id(&self, gx: u32, gy: u32) -> i32 {
        if gx >= self.grid_size || gy >= self.grid_size {
            return 0;
        }
        self.cells[(gy * self.grid_size + gx) as usize]
    }

    /// Set the parcel `local_id` for the cell at grid indices `(gx, gy)`.
    pub fn set_parcel_id(&mut self, gx: u32, gy: u32, local_id: i32) {
        if gx < self.grid_size && gy < self.grid_size {
            self.cells[(gy * self.grid_size + gx) as usize] = local_id;
        }
    }

    /// Get the parcel `local_id` at metre coordinates `(x, y)`.
    pub fn parcels_at_point(&self, x: f32, y: f32) -> i32 {
        let (gx, gy) = self.to_cell(x, y);
        self.get_parcel_id(gx, gy)
    }

    /// Set every cell in the grid to the given parcel id.
    pub fn fill(&mut self, local_id: i32) {
        self.cells.fill(local_id);
    }

    /// Set cells within a rectangle (in metre coords) to the given parcel id.
    pub fn fill_rect(&mut self, start_x: u32, start_y: u32, end_x: u32, end_y: u32, local_id: i32) {
        let sx = (start_x / LAND_UNIT).min(self.grid_size);
        let sy = (start_y / LAND_UNIT).min(self.grid_size);
        let ex = (end_x / LAND_UNIT).min(self.grid_size);
        let ey = (end_y / LAND_UNIT).min(self.grid_size);
        for gy in sy..ey {
            for gx in sx..ex {
                self.cells[(gy * self.grid_size + gx) as usize] = local_id;
            }
        }
    }

    /// Count how many cells are assigned to the given parcel id.
    pub fn cell_count(&self, local_id: i32) -> usize {
        self.cells.iter().filter(|&&c| c == local_id).count()
    }
}

impl Default for ParcelGrid {
    fn default() -> Self {
        Self::new(256)
    }
}
