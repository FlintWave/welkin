// SPDX-License-Identifier: LGPL-3.0-or-later

//! Environment modules: windlight presets, environment settings, and
//! LightShare environment management.

pub mod misc;
pub mod sound;
pub mod wind;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// Re-exports for convenience.
pub use misc::{
    AccessModule, MediaEntry, MediaModule, RegionAccessFlags, RegionReadyModule, RestartModule,
    SceneSerialiser, VegetationModule,
};
pub use sound::{SoundFlags, SoundModule, SoundTrigger};
pub use wind::{ConfigurableWind, SimpleRandomWind, WindModel, WindModule};

// ---------------------------------------------------------------------------
// WindlightPreset
// ---------------------------------------------------------------------------

/// Windlight environment preset, mapping the viewer's sky/water parameters.
///
/// Mirrors the key fields from the C# `RegionLightShareData` and the
/// `ViewerEnvironment` structures used by the EnvironmentModule.
#[derive(Debug, Clone)]
pub struct WindlightPreset {
    /// Ambient light colour (RGB).
    pub ambient: [f32; 3],
    /// Sun diffuse colour (RGB).
    pub sun_color: [f32; 3],
    /// Sun direction (normalised XYZ).
    pub sun_direction: [f32; 3],
    /// Cloud colour (RGB).
    pub cloud_color: [f32; 3],
    /// Cloud coverage factor (0.0 = clear, 1.0 = overcast).
    pub cloud_coverage: f32,
    /// Cloud scroll rate (X, Y).
    pub cloud_scroll: [f32; 2],
    /// Water colour (RGB).
    pub water_color: [f32; 3],
    /// Water fog density.
    pub water_fog_density: f32,
    /// Big wave direction (XY).
    pub big_wave_direction: [f32; 2],
    /// Little wave direction (XY).
    pub little_wave_direction: [f32; 2],
    /// Haze density.
    pub haze_density: f32,
    /// Haze horizon.
    pub haze_horizon: f32,
    /// Scene gamma.
    pub scene_gamma: f32,
    /// Star brightness.
    pub star_brightness: f32,
    /// Blue density (RGB).
    pub blue_density: [f32; 3],
    /// Horizon shading (RGBA).
    pub horizon: [f32; 4],
    /// East angle (radians).
    pub east_angle: f32,
    /// Sun glow focus.
    pub sun_glow_focus: f32,
    /// Sun glow size.
    pub sun_glow_size: f32,
    /// Underwater fog modifier.
    pub underwater_fog_modifier: f32,
    /// Normal-map texture UUID for water surface.
    pub normal_map_texture: Uuid,
    /// Reflection wavelet scale (XYZ).
    pub reflection_wavelet_scale: [f32; 3],
    /// Fresnel scale.
    pub fresnel_scale: f32,
    /// Fresnel offset.
    pub fresnel_offset: f32,
    /// Refract scale above water.
    pub refract_scale_above: f32,
    /// Refract scale below water.
    pub refract_scale_below: f32,
    /// Blur multiplier.
    pub blur_multiplier: f32,
}

impl Default for WindlightPreset {
    fn default() -> Self {
        Self {
            ambient: [0.25, 0.25, 0.25],
            sun_color: [1.0, 1.0, 1.0],
            sun_direction: [0.0, 0.707, 0.707],
            cloud_color: [0.9, 0.9, 0.9],
            cloud_coverage: 0.27,
            cloud_scroll: [0.2, 0.01],
            water_color: [0.0, 0.0, 0.2],
            water_fog_density: 16.0,
            big_wave_direction: [1.05, -0.42],
            little_wave_direction: [1.11, -0.76],
            haze_density: 0.7,
            haze_horizon: 0.19,
            scene_gamma: 1.0,
            star_brightness: 0.0,
            blue_density: [0.12, 0.22, 0.38],
            horizon: [0.25, 0.25, 0.32, 0.32],
            east_angle: 0.0,
            sun_glow_focus: 0.1,
            sun_glow_size: 1.75,
            underwater_fog_modifier: 0.25,
            normal_map_texture: Uuid::nil(),
            reflection_wavelet_scale: [2.0, 2.0, 2.0],
            fresnel_scale: 0.4,
            fresnel_offset: 0.5,
            refract_scale_above: 0.03,
            refract_scale_below: 0.2,
            blur_multiplier: 0.04,
        }
    }
}

// ---------------------------------------------------------------------------
// EnvironmentSettings
// ---------------------------------------------------------------------------

/// Region-level environment settings (day cycle, sky map, water map).
///
/// These are stubs for the full Linden-protocol environment system that
/// replaced Windlight in newer viewers.
#[derive(Debug, Clone, Default)]
pub struct EnvironmentSettings {
    /// Day-cycle data (opaque blob in production; stub here).
    pub day_cycle: Vec<u8>,
    /// Sky-map data (opaque blob in production; stub here).
    pub sky_map: Vec<u8>,
    /// Water-map data (opaque blob in production; stub here).
    pub water_map: Vec<u8>,
}

// ---------------------------------------------------------------------------
// LightShareModule
// ---------------------------------------------------------------------------

/// Manages the region's LightShare / Windlight environment.
///
/// Rust port of the C# `EnvironmentModule`, providing the Windlight preset
/// and the newer `EnvironmentSettings` (day-cycle) stubs.
pub struct LightShareModule {
    enabled: bool,
    windlight: WindlightPreset,
    env_settings: EnvironmentSettings,
    env_version: i32,
}

impl LightShareModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            windlight: WindlightPreset::default(),
            env_settings: EnvironmentSettings::default(),
            env_version: -1,
        }
    }

    /// Returns the current Windlight preset.
    pub fn get_windlight(&self) -> &WindlightPreset {
        &self.windlight
    }

    /// Sets the Windlight preset for this region.
    pub fn set_windlight(&mut self, preset: WindlightPreset) {
        self.windlight = preset;
        self.env_version += 1;
        tracing::debug!(version = self.env_version, "windlight preset updated");
    }

    /// Resets the Windlight preset to the defaults.
    pub fn reset_windlight(&mut self) {
        self.windlight = WindlightPreset::default();
        self.env_version += 1;
        tracing::debug!(
            version = self.env_version,
            "windlight preset reset to defaults"
        );
    }

    /// Returns whether the module is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the current environment settings.
    pub fn get_environment_settings(&self) -> &EnvironmentSettings {
        &self.env_settings
    }

    /// Sets the environment settings (day-cycle, sky-map, water-map).
    pub fn set_environment_settings(&mut self, settings: EnvironmentSettings) {
        self.env_settings = settings;
        self.env_version += 1;
        tracing::debug!(version = self.env_version, "environment settings updated");
    }

    /// Returns the current environment version counter.
    pub fn env_version(&self) -> i32 {
        self.env_version
    }
}

impl Default for LightShareModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for LightShareModule {
    fn name(&self) -> &str {
        "LightShareModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let cap = config.get_string("ClientStack.LindenCaps", "Cap_EnvironmentSettings", "");
        self.enabled = cap == "localhost";
        if self.enabled {
            tracing::info!("LightShareModule enabled");
        } else {
            tracing::info!("LightShareModule disabled (Cap_EnvironmentSettings != localhost)");
        }
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "LightShareModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "LightShareModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}
