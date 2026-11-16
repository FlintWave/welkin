// SPDX-License-Identifier: LGPL-3.0-or-later

pub const TERRAIN_PATCH_SIZE: u32 = 16;
pub const DEFAULT_REGION_SIZE: u32 = 256;
pub const DEFAULT_TERRAIN_HEIGHT: f32 = 21.0;
pub const REGION_HEIGHT_LIMIT: f32 = 4096.0;

#[derive(Debug)]
pub struct TerrainChannel {
    size_x: u32,
    size_y: u32,
    heights: Vec<f32>,
    dirty: Vec<bool>,
    patch_size: u32,
}

impl TerrainChannel {
    pub fn new(size_x: u32, size_y: u32) -> Self {
        let len = (size_x * size_y) as usize;
        let patches_x = size_x / TERRAIN_PATCH_SIZE;
        let patches_y = size_y / TERRAIN_PATCH_SIZE;
        Self {
            size_x,
            size_y,
            heights: vec![DEFAULT_TERRAIN_HEIGHT; len],
            dirty: vec![false; (patches_x * patches_y) as usize],
            patch_size: TERRAIN_PATCH_SIZE,
        }
    }

    pub fn new_default() -> Self {
        Self::new(DEFAULT_REGION_SIZE, DEFAULT_REGION_SIZE)
    }

    pub fn size_x(&self) -> u32 {
        self.size_x
    }

    pub fn size_y(&self) -> u32 {
        self.size_y
    }

    pub fn patches_x(&self) -> u32 {
        self.size_x / self.patch_size
    }

    pub fn patches_y(&self) -> u32 {
        self.size_y / self.patch_size
    }

    pub fn get(&self, x: u32, y: u32) -> f32 {
        assert!(
            x < self.size_x && y < self.size_y,
            "terrain index ({x}, {y}) out of bounds ({}, {})",
            self.size_x,
            self.size_y,
        );
        self.heights[(y * self.size_x + x) as usize]
    }

    pub fn get_checked(&self, x: u32, y: u32) -> Option<f32> {
        if x < self.size_x && y < self.size_y {
            Some(self.heights[(y * self.size_x + x) as usize])
        } else {
            None
        }
    }

    pub fn set(&mut self, x: u32, y: u32, height: f32) {
        assert!(
            x < self.size_x && y < self.size_y,
            "terrain index ({x}, {y}) out of bounds ({}, {})",
            self.size_x,
            self.size_y,
        );
        if !height.is_finite() {
            return; // NaN/inf would poison the height field (and physics); keep the old value
        }
        let clamped = height.clamp(0.0, REGION_HEIGHT_LIMIT);
        self.heights[(y * self.size_x + x) as usize] = clamped;

        let px = x / self.patch_size;
        let py = y / self.patch_size;
        let patches_x = self.size_x / self.patch_size;
        self.dirty[(py * patches_x + px) as usize] = true;
    }

    pub fn is_dirty(&self, px: u32, py: u32) -> bool {
        self.dirty[(py * self.patches_x() + px) as usize]
    }

    pub fn clear_dirty(&mut self) {
        self.dirty.fill(false);
    }

    pub fn dirty_patches(&self) -> Vec<(u32, u32)> {
        let px_count = self.patches_x();
        self.dirty
            .iter()
            .enumerate()
            .filter(|(_, d)| **d)
            .map(|(i, _)| {
                let i = i as u32;
                (i % px_count, i / px_count)
            })
            .collect()
    }

    pub fn patch_min_max(&self, px: u32, py: u32) -> (f32, f32) {
        let base_x = px * self.patch_size;
        let base_y = py * self.patch_size;
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for dy in 0..self.patch_size {
            for dx in 0..self.patch_size {
                let h = self.get(base_x + dx, base_y + dy);
                if h < min {
                    min = h;
                }
                if h > max {
                    max = h;
                }
            }
        }
        (min, max)
    }

    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    pub fn heights_mut(&mut self) -> &mut [f32] {
        &mut self.heights
    }

    pub fn mark_all_dirty(&mut self) {
        self.dirty.fill(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_terrain() {
        let t = TerrainChannel::new_default();
        assert_eq!(t.size_x(), 256);
        assert_eq!(t.size_y(), 256);
        assert_eq!(t.heights().len(), 256 * 256);
        for &h in t.heights() {
            assert_eq!(h, DEFAULT_TERRAIN_HEIGHT);
        }
    }

    #[test]
    fn test_get_set() {
        let mut t = TerrainChannel::new_default();
        t.set(10, 20, 50.0);
        assert_eq!(t.get(10, 20), 50.0);
        assert_eq!(t.get(0, 0), DEFAULT_TERRAIN_HEIGHT);
    }

    #[test]
    fn test_dirty_tracking() {
        let mut t = TerrainChannel::new_default();
        let px = 0;
        let py = 0;
        assert!(!t.is_dirty(px, py));

        t.set(5, 5, 30.0);
        assert!(t.is_dirty(px, py));

        let dirty = t.dirty_patches();
        assert!(dirty.contains(&(px, py)));

        t.clear_dirty();
        assert!(!t.is_dirty(px, py));
        assert!(t.dirty_patches().is_empty());
    }

    #[test]
    fn test_patch_min_max() {
        let mut t = TerrainChannel::new_default();
        t.set(0, 0, 10.0);
        t.set(5, 5, 100.0);
        let (min, max) = t.patch_min_max(0, 0);
        assert_eq!(min, 10.0);
        assert_eq!(max, 100.0);
    }

    #[test]
    fn test_out_of_bounds() {
        let t = TerrainChannel::new_default();
        assert!(t.get_checked(255, 255).is_some());
        assert!(t.get_checked(256, 0).is_none());
        assert!(t.get_checked(0, 256).is_none());
        assert!(t.get_checked(300, 300).is_none());
    }

    #[test]
    fn test_variable_size() {
        let t = TerrainChannel::new(512, 512);
        assert_eq!(t.size_x(), 512);
        assert_eq!(t.size_y(), 512);
        assert_eq!(t.patches_x(), 32);
        assert_eq!(t.patches_y(), 32);
        assert_eq!(t.heights().len(), 512 * 512);
    }

    #[test]
    fn test_height_clamping() {
        let mut t = TerrainChannel::new_default();
        t.set(0, 0, -10.0);
        assert_eq!(t.get(0, 0), 0.0);
        t.set(0, 0, 5000.0);
        assert_eq!(t.get(0, 0), REGION_HEIGHT_LIMIT);
    }

    #[test]
    fn test_mark_all_dirty() {
        let mut t = TerrainChannel::new_default();
        t.mark_all_dirty();
        let patches = t.patches_x() * t.patches_y();
        assert_eq!(t.dirty_patches().len(), patches as usize);
    }

    #[test]
    fn non_finite_heights_are_ignored() {
        let mut t = TerrainChannel::new(16, 16);
        t.set(3, 3, 7.0);
        t.clear_dirty();
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            t.set(3, 3, bad);
        }
        assert_eq!(t.get(3, 3), 7.0);
        assert!(t.dirty_patches().is_empty());
    }
}
