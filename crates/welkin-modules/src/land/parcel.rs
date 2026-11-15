// SPDX-License-Identifier: GPL-3.0-or-later

//! [`LandParcel`] wraps [`LandData`] and provides bitmap operations that
//! mirror the C# `LandObject` class.

use super::data::LandData;
use super::grid::LAND_UNIT;

/// A parcel with its decoded land bitmap and convenience geometry methods.
#[derive(Debug, Clone)]
pub struct LandParcel {
    pub data: LandData,
    /// 2-D bitmap: `land_bitmap[gx][gy]` — `true` means the cell belongs to
    /// this parcel.  Dimensions are `(region_size / 4) x (region_size / 4)`.
    pub land_bitmap: Vec<Vec<bool>>,
}

impl LandParcel {
    /// Create a parcel with the given data and an empty bitmap sized for
    /// `region_size` metres.
    pub fn new(data: LandData, region_size: u32) -> Self {
        let dim = (region_size / LAND_UNIT) as usize;
        Self {
            data,
            land_bitmap: vec![vec![false; dim]; dim],
        }
    }

    /// Does this parcel's bitmap include the cell at grid coordinates
    /// `(gx, gy)`?
    pub fn contains_point(&self, gx: usize, gy: usize) -> bool {
        self.land_bitmap
            .get(gx)
            .and_then(|col| col.get(gy))
            .copied()
            .unwrap_or(false)
    }

    /// A bitmap covering the entire region (all cells `true`).
    pub fn basic_full_region_bitmap(region_size: u32) -> Vec<Vec<bool>> {
        let dim = (region_size / LAND_UNIT) as usize;
        vec![vec![true; dim]; dim]
    }

    /// A bitmap with a rectangular region set to `true`.
    ///
    /// Coordinates are in **metres**.
    pub fn get_square_bitmap(
        region_size: u32,
        start_x: u32,
        start_y: u32,
        end_x: u32,
        end_y: u32,
    ) -> Vec<Vec<bool>> {
        let dim = (region_size / LAND_UNIT) as usize;
        let mut bm = vec![vec![false; dim]; dim];
        let sx = (start_x / LAND_UNIT) as usize;
        let sy = (start_y / LAND_UNIT) as usize;
        let ex = (end_x / LAND_UNIT).min(region_size / LAND_UNIT) as usize;
        let ey = (end_y / LAND_UNIT).min(region_size / LAND_UNIT) as usize;
        for gx in sx..ex {
            for gy in sy..ey {
                bm[gx][gy] = true;
            }
        }
        bm
    }

    /// OR two bitmaps together.  Both must have the same dimensions.
    pub fn merge_bitmaps(
        base: &[Vec<bool>],
        add: &[Vec<bool>],
    ) -> Vec<Vec<bool>> {
        assert_eq!(base.len(), add.len(), "bitmap x dimensions must match");
        let mut out = base.to_vec();
        for (col_out, col_add) in out.iter_mut().zip(add.iter()) {
            assert_eq!(col_out.len(), col_add.len(), "bitmap y dimensions must match");
            for (cell, &a) in col_out.iter_mut().zip(col_add.iter()) {
                *cell |= a;
            }
        }
        out
    }

    /// Serialise a 2-D bool bitmap to the packed byte format used in
    /// `LandData.bitmap` (one bit per cell, row-major Y-outer loop).
    pub fn bitmap_to_bytes(bm: &[Vec<bool>]) -> Vec<u8> {
        if bm.is_empty() {
            return Vec::new();
        }
        let x_len = bm.len();
        let y_len = bm[0].len();
        let total_bits = x_len * y_len;
        let mut bytes = vec![0u8; (total_bits + 7) / 8];
        let mut bit_idx = 0usize;
        for gy in 0..y_len {
            for gx in 0..x_len {
                if bm[gx][gy] {
                    bytes[bit_idx / 8] |= 1 << (bit_idx % 8);
                }
                bit_idx += 1;
            }
        }
        bytes
    }

    /// Deserialise packed bytes back to a 2-D bool bitmap.
    ///
    /// `dim` is the number of cells along one axis
    /// (`region_size / LAND_UNIT`).
    pub fn bytes_to_bitmap(bytes: &[u8], dim: usize) -> Vec<Vec<bool>> {
        let mut bm = vec![vec![false; dim]; dim];
        let mut bit_idx = 0usize;
        for gy in 0..dim {
            for gx in 0..dim {
                let byte_idx = bit_idx / 8;
                if byte_idx < bytes.len() && (bytes[byte_idx] & (1 << (bit_idx % 8))) != 0 {
                    bm[gx][gy] = true;
                }
                bit_idx += 1;
            }
        }
        bm
    }

    /// Compute the area in square metres from the bitmap.
    pub fn area(&self) -> i32 {
        let count: usize = self
            .land_bitmap
            .iter()
            .flat_map(|col| col.iter())
            .filter(|&&v| v)
            .count();
        (count as i32) * (LAND_UNIT as i32) * (LAND_UNIT as i32)
    }

    /// Lowest (min-x, min-y) corner in metres of the parcel's bounding box,
    /// computed from the bitmap.
    pub fn start_point(&self) -> (f32, f32) {
        let mut min_x = self.land_bitmap.len();
        let mut min_y = if self.land_bitmap.is_empty() {
            0
        } else {
            self.land_bitmap[0].len()
        };
        for (gx, col) in self.land_bitmap.iter().enumerate() {
            for (gy, &v) in col.iter().enumerate() {
                if v {
                    if gx < min_x {
                        min_x = gx;
                    }
                    if gy < min_y {
                        min_y = gy;
                    }
                }
            }
        }
        (
            (min_x as f32) * LAND_UNIT as f32,
            (min_y as f32) * LAND_UNIT as f32,
        )
    }

    /// Highest (max-x, max-y) corner in metres of the parcel's bounding box,
    /// computed from the bitmap.  The returned point is one land-unit past the
    /// last occupied cell (exclusive upper bound).
    pub fn end_point(&self) -> (f32, f32) {
        let mut max_x: usize = 0;
        let mut max_y: usize = 0;
        for (gx, col) in self.land_bitmap.iter().enumerate() {
            for (gy, &v) in col.iter().enumerate() {
                if v {
                    if gx + 1 > max_x {
                        max_x = gx + 1;
                    }
                    if gy + 1 > max_y {
                        max_y = gy + 1;
                    }
                }
            }
        }
        (
            (max_x as f32) * LAND_UNIT as f32,
            (max_y as f32) * LAND_UNIT as f32,
        )
    }

    /// Centre of the parcel's bounding box in metres.
    pub fn center_point(&self) -> (f32, f32) {
        let (sx, sy) = self.start_point();
        let (ex, ey) = self.end_point();
        ((sx + ex) / 2.0, (sy + ey) / 2.0)
    }

    /// Synchronise `self.data.bitmap` from the in-memory 2-D bitmap.
    pub fn sync_bitmap_to_data(&mut self) {
        self.data.bitmap = Self::bitmap_to_bytes(&self.land_bitmap);
    }

    /// Populate the in-memory 2-D bitmap from `self.data.bitmap`.
    pub fn sync_data_to_bitmap(&mut self, region_size: u32) {
        let dim = (region_size / LAND_UNIT) as usize;
        self.land_bitmap = Self::bytes_to_bitmap(&self.data.bitmap, dim);
    }
}
