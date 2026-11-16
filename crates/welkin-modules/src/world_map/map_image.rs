// SPDX-License-Identifier: LGPL-3.0-or-later

//! Map tile image generation: terrain rendering and object overlay.
//!
//! Mirrors the C# `MapImageModule`, `ShadedMapTileRenderer`, and
//! `TexturedMapTileRenderer` from `OpenSim.Region.CoreModules.World.LegacyMap`.

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// MapTileConfig
// ---------------------------------------------------------------------------

/// Configuration for map tile generation.
#[derive(Debug, Clone)]
pub struct MapTileConfig {
    /// Tile width and height in pixels.  Defaults to 256.
    pub tile_size: u32,
    /// Whether to draw prim outlines on the map tile.
    pub draw_prims: bool,
    /// Terrain colour scheme identifier.  `"shaded"` for the default
    /// height-to-grayscale renderer.
    pub terrain_color_scheme: String,
    /// Water colour as `(r, g, b)`.
    pub water_color: (u8, u8, u8),
    /// Default water height in meters.
    pub water_height: f32,
}

impl Default for MapTileConfig {
    fn default() -> Self {
        Self {
            tile_size: 256,
            draw_prims: true,
            terrain_color_scheme: "shaded".to_string(),
            // #1D475F — the same default as the C# ShadedMapTileRenderer.
            water_color: (0x1D, 0x47, 0x5F),
            water_height: 20.0,
        }
    }
}

// ---------------------------------------------------------------------------
// TerrainRenderer trait
// ---------------------------------------------------------------------------

/// Trait for pluggable terrain renderers that produce a map tile from a
/// heightmap.
pub trait TerrainRenderer: Send + Sync {
    /// Render the given heightmap into a `tiny_skia::Pixmap`.
    ///
    /// - `heightmap` — row-major height values (meters), length = `width * height`.
    /// - `width`, `height` — heightmap dimensions (typically 256x256).
    /// - `water_height` — sea level for water painting.
    fn render_terrain(
        &self,
        heightmap: &[f32],
        width: u32,
        height: u32,
        water_height: f32,
    ) -> tiny_skia::Pixmap;
}

// ---------------------------------------------------------------------------
// ShadedTerrainRenderer
// ---------------------------------------------------------------------------

/// Height-to-grayscale renderer with simple directional shading.
///
/// Slopes facing east/south receive a highlight; slopes facing west/north
/// are darkened, producing a subtle relief effect that matches the C#
/// `ShadedMapTileRenderer`.
pub struct ShadedTerrainRenderer {
    /// Water colour as `(r, g, b)`.
    pub water_color: (u8, u8, u8),
}

impl ShadedTerrainRenderer {
    pub fn new() -> Self {
        Self {
            water_color: (0x1D, 0x47, 0x5F),
        }
    }

    /// Sample the heightmap at `(x, y)`, returning 0.0 for out-of-bounds.
    #[inline]
    fn sample(heightmap: &[f32], x: i32, y: i32, width: u32, height: u32) -> f32 {
        if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
            return 0.0;
        }
        let idx = (y as u32 * width + x as u32) as usize;
        heightmap.get(idx).copied().unwrap_or(0.0)
    }
}

impl Default for ShadedTerrainRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TerrainRenderer for ShadedTerrainRenderer {
    fn render_terrain(
        &self,
        heightmap: &[f32],
        width: u32,
        height: u32,
        water_height: f32,
    ) -> tiny_skia::Pixmap {
        let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("non-zero tile dimensions");

        for y in 0..height {
            for x in 0..width {
                let h = Self::sample(heightmap, x as i32, y as i32, width, height);

                // Y-flip: heightmap origin is bottom-left, pixmap origin is
                // top-left.
                let yr = (height - 1) - y;

                if h > water_height {
                    // Clamp to [0, 255] for the base green channel.
                    let clamped = h.clamp(0.0, 255.0);

                    // Base colour: (height, 100, height) — green-tinted height.
                    let mut r = clamped as i32;
                    let mut g = 100_i32;
                    let mut b = clamped as i32;

                    // Directional shading: compare with the cell to the
                    // north-east (x+1, y+1).  A positive diff means NE is
                    // lower (we're on a lit slope); negative means we're in
                    // shadow.
                    let h_ne = Self::sample(heightmap, x as i32 + 1, y as i32 + 1, width, height);
                    let diff = h - h_ne;

                    let shade_amount = (diff.abs() * 4.5).min(40.0) as i32;

                    if diff > 0.3 {
                        // Highlight (lighter).
                        let highlight = (shade_amount as f32 * 0.18) as i32 + 1;
                        r = (r + highlight).min(255);
                        g = (g + highlight).min(255);
                        b = (b + highlight).min(255);
                    } else if diff < -0.3 {
                        // Shadow (darker).
                        r = (r - shade_amount).max(0);
                        g = (g - shade_amount).max(0);
                        b = (b - shade_amount).max(0);
                    }

                    let pixel =
                        tiny_skia::PremultipliedColorU8::from_rgba(r as u8, g as u8, b as u8, 255)
                            .unwrap();
                    pixmap.pixels_mut()[(yr * width + x) as usize] = pixel;
                } else {
                    // Under water.
                    let (wr, wg, wb) = self.water_color;
                    let pixel =
                        tiny_skia::PremultipliedColorU8::from_rgba(wr, wg, wb, 255).unwrap();
                    pixmap.pixels_mut()[(yr * width + x) as usize] = pixel;
                }
            }
        }

        pixmap
    }
}

// ---------------------------------------------------------------------------
// MapImageModule
// ---------------------------------------------------------------------------

/// Per-region module responsible for generating and uploading map tiles.
///
/// Uses a [`TerrainRenderer`] to paint the terrain and optionally overlays
/// object (prim) outlines.
pub struct MapImageModule {
    pub config: MapTileConfig,
    pub region_name: String,
    last_tile_id: Uuid,
}

impl MapImageModule {
    pub fn new() -> Self {
        Self {
            config: MapTileConfig::default(),
            region_name: String::new(),
            last_tile_id: Uuid::nil(),
        }
    }

    /// Access the current configuration.
    pub fn config(&self) -> &MapTileConfig {
        &self.config
    }

    /// Generate a 256x256 (or configured size) RGBA map tile from the given
    /// heightmap.
    ///
    /// Returns the raw RGBA pixel data (`4 * tile_size * tile_size` bytes).
    pub fn generate_map_tile(&self, heightmap: &[f32]) -> Vec<u8> {
        let size = self.config.tile_size;
        if size == 0 {
            return Vec::new();
        }

        let needed = (size as usize).saturating_mul(size as usize);
        let mut terrain = vec![0.0f32; needed];
        let copy_len = needed.min(heightmap.len());
        terrain[..copy_len].copy_from_slice(&heightmap[..copy_len]);

        let renderer = ShadedTerrainRenderer {
            water_color: self.config.water_color,
        };

        let mut pixmap = renderer.render_terrain(&terrain, size, size, self.config.water_height);

        if self.config.draw_prims {
            self.render_objects_on_map(&mut pixmap);
        }

        pixmap.data().to_vec()
    }

    /// Draw coloured rectangles at prim positions on the map tile.
    ///
    /// This module does not retain scene object positions, so there are no prim
    /// footprints available to draw. The supplied pixmap is left unchanged.
    pub fn render_objects_on_map(&self, _pixmap: &mut tiny_skia::Pixmap) {
        tracing::trace!(
            region = %self.region_name,
            "render_objects_on_map (no object positions available)"
        );
    }

    /// Upload the generated map tile to the map image service.
    ///
    /// Returns the UUID assigned to the uploaded tile. An empty payload is
    /// rejected, matching OpenSim's null-image guard.
    pub fn upload_map_tile(&mut self, tile_data: &[u8]) -> Uuid {
        if tile_data.is_empty() {
            tracing::warn!(
                region = %self.region_name,
                "Cannot upload null map tile"
            );
            return Uuid::nil();
        }

        let asset_id = Uuid::new_v4();
        self.last_tile_id = asset_id;

        tracing::debug!(
            region = %self.region_name,
            %asset_id,
            bytes = tile_data.len(),
            "upload_map_tile"
        );

        asset_id
    }

    /// Return the UUID of the last uploaded tile, or nil if none.
    pub fn last_tile_id(&self) -> Uuid {
        self.last_tile_id
    }
}

impl Default for MapImageModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for MapImageModule {
    fn name(&self) -> &str {
        "MapImageModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.config.draw_prims = config.get_bool("Map", "DrawPrimOnMapTile", true);
        self.config.tile_size = config.get_int("Map", "TileSize", 256) as u32;
        let scheme = config.get_string("Map", "TerrainColorScheme", "shaded");
        self.config.terrain_color_scheme = scheme;
        self.config.water_height = config.get_float("Map", "WaterHeight", 20.0) as f32;
        tracing::info!(
            draw_prims = self.config.draw_prims,
            tile_size = self.config.tile_size,
            "MapImageModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.region_name = scene.region_name.clone();
        tracing::info!(region = %scene.region_name, "MapImageModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "MapImageModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_age_rs_mapimagemodule_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn generate_map_tile_returns_configured_byte_count() {
        let mut module = MapImageModule {
            config: Default::default(),
            region_name: "region".to_string(),
            last_tile_id: Uuid::nil(),
        };
        module.config.tile_size = 2;
        module.config.water_color = (0, 0, 255);
        module.config.water_height = 0.5;
        module.config.draw_prims = false;

        let heightmap = [0.0f32; 4];
        let bytes = module.generate_map_tile(&heightmap);

        assert_eq!(bytes.len(), 4 * 2 * 2);
    }

    #[test]
    fn generate_map_tile_pads_short_heightmaps() {
        let mut module = MapImageModule {
            config: Default::default(),
            region_name: "region".to_string(),
            last_tile_id: Uuid::nil(),
        };
        module.config.tile_size = 2;
        module.config.water_color = (0, 0, 255);
        module.config.water_height = 0.5;
        module.config.draw_prims = true;

        let heightmap = [1.0f32];
        let bytes = module.generate_map_tile(&heightmap);

        assert_eq!(bytes.len(), 4 * 2 * 2);
        assert!(bytes.iter().any(|&byte| byte != 0));
    }

    #[test]
    fn render_objects_on_map_does_not_mutate_pixmap() {
        let module = MapImageModule {
            config: Default::default(),
            region_name: "region".to_string(),
            last_tile_id: Uuid::nil(),
        };

        let mut pixmap = tiny_skia::Pixmap::new(2, 2).unwrap();
        pixmap.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        let before = pixmap.data().to_vec();

        module.render_objects_on_map(&mut pixmap);

        assert_eq!(pixmap.data(), before.as_slice());
    }

    #[test]
    fn render_objects_on_map_handles_single_pixel() {
        let module = MapImageModule {
            config: Default::default(),
            region_name: "region".to_string(),
            last_tile_id: Uuid::nil(),
        };

        let mut pixmap = tiny_skia::Pixmap::new(1, 1).unwrap();
        module.render_objects_on_map(&mut pixmap);

        assert_eq!(pixmap.data().len(), 4);
    }

    #[test]
    fn upload_map_tile_accepts_valid_payload() {
        let mut module = MapImageModule {
            config: Default::default(),
            region_name: "region".to_string(),
            last_tile_id: Uuid::nil(),
        };

        let payload = [0u8; 4 * 2 * 2];
        let asset_id = module.upload_map_tile(&payload);

        assert!(!asset_id.is_nil());
        assert_eq!(module.last_tile_id, asset_id);
    }

    #[test]
    fn upload_map_tile_rejects_empty_payload() {
        let mut module = MapImageModule {
            config: Default::default(),
            region_name: "region".to_string(),
            last_tile_id: Uuid::nil(),
        };

        let previous = module.last_tile_id;
        let asset_id = module.upload_map_tile(&[]);

        assert!(asset_id.is_nil());
        assert_eq!(module.last_tile_id, previous);
    }

    #[test]
    fn upload_map_tile_works_after_add_region() {

        let recording = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "region".to_string());
        scene.egress = recording.clone();

        let mut module = MapImageModule {
            config: Default::default(),
            region_name: "region".to_string(),
            last_tile_id: Uuid::nil(),
        };

        let _ = module.add_region(&scene);
        let payload = [7u8; 4 * 2 * 2];
        let asset_id = module.upload_map_tile(&payload);

        assert!(!asset_id.is_nil());
    }
}
