// SPDX-License-Identifier: GPL-3.0-or-later

//! Wind simulation module with pluggable wind models.
//!
//! Mirrors the C# `WindModule` and its `IWindModelPlugin` system, providing
//! `SimpleRandomWind` (deterministic pseudo-random) and `ConfigurableWind`
//! (predominate-direction with variance).

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// WindModel trait
// ---------------------------------------------------------------------------

/// Pluggable wind-model interface, equivalent to the C# `IWindModelPlugin`.
pub trait WindModel: Send + Sync {
    /// Human-readable name of this wind model.
    fn name(&self) -> &str;

    /// Return the wind speed vector at the given region coordinates.
    ///
    /// The region is divided into a 16x16 grid of patches (each 16m on a
    /// 256m region).  The returned `(x, y)` pair is the wind velocity at
    /// that position.
    fn wind_speed(&self, x: f32, y: f32) -> (f32, f32);

    /// Advance the wind model by `elapsed` seconds of simulation time.
    fn update(&mut self, elapsed: f32);
}

// ---------------------------------------------------------------------------
// SimpleRandomWind
// ---------------------------------------------------------------------------

/// Simple deterministic pseudo-random wind model.
///
/// Mirrors the C# `SimpleRandomWind` plugin: fills a 16x16 grid with
/// random-ish wind vectors scaled by `strength`.  Because we avoid an
/// `rand` dependency, the randomness comes from a simple LCG seeded by
/// the update counter.
pub struct SimpleRandomWind {
    /// Wind strength multiplier.
    pub strength: f32,
    /// 16x16 grid of (x, y) wind vectors.
    speeds: Vec<(f32, f32)>,
    /// Internal update counter used as LCG seed.
    counter: u32,
}

impl SimpleRandomWind {
    pub fn new() -> Self {
        Self {
            strength: 1.0,
            speeds: vec![(0.0, 0.0); 16 * 16],
            counter: 0,
        }
    }

    /// Simple LCG: returns a value in [-1.0, 1.0).
    fn lcg_next(state: &mut u32) -> f32 {
        *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        // Map to [-1.0, 1.0).
        ((*state >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
    }
}

impl Default for SimpleRandomWind {
    fn default() -> Self {
        Self::new()
    }
}

impl WindModel for SimpleRandomWind {
    fn name(&self) -> &str {
        "SimpleRandomWind"
    }

    fn wind_speed(&self, x: f32, y: f32) -> (f32, f32) {
        let gx = ((x as i32) / 16).clamp(0, 15) as usize;
        let gy = ((y as i32) / 16).clamp(0, 15) as usize;
        self.speeds[gy * 16 + gx]
    }

    fn update(&mut self, _elapsed: f32) {
        self.counter = self.counter.wrapping_add(1);
        let mut state = self.counter.wrapping_mul(2654435761); // Knuth hash
        for cell in self.speeds.iter_mut() {
            let vx = Self::lcg_next(&mut state) * self.strength;
            let vy = Self::lcg_next(&mut state) * self.strength;
            *cell = (vx, vy);
        }
    }
}

// ---------------------------------------------------------------------------
// ConfigurableWind
// ---------------------------------------------------------------------------

/// Predominate-direction wind model with configurable variance.
///
/// Mirrors the C# `ConfigurableWind` plugin: produces a slowly-varying
/// wind vector based on trigonometric functions of simulated time, with
/// configurable average strength/direction and variance parameters.
pub struct ConfigurableWind {
    /// Average wind strength.
    pub avg_strength: f32,
    /// Average wind direction in degrees.
    pub avg_direction: f32,
    /// Maximum strength variance.
    pub variance_strength: f32,
    /// Maximum direction variance in degrees.
    pub variance_direction: f32,
    /// Rate of change multiplier.
    pub rate_of_change: f32,

    /// Accumulated simulated time.
    time: f32,
    /// Current predominate wind vector.
    current_wind: (f32, f32),
}

impl ConfigurableWind {
    pub fn new() -> Self {
        Self {
            avg_strength: 5.0,
            avg_direction: 0.0,
            variance_strength: 5.0,
            variance_direction: 30.0,
            rate_of_change: 1.0,
            time: 0.0,
            current_wind: (5.0, 0.0),
        }
    }
}

impl Default for ConfigurableWind {
    fn default() -> Self {
        Self::new()
    }
}

impl WindModel for ConfigurableWind {
    fn name(&self) -> &str {
        "ConfigurableWind"
    }

    fn wind_speed(&self, _x: f32, _y: f32) -> (f32, f32) {
        // ConfigurableWind uses the same vector everywhere.
        self.current_wind
    }

    fn update(&mut self, elapsed: f32) {
        self.time += elapsed;

        let avg_ang = self.avg_direction.to_radians();
        let var_dir = self.variance_direction.to_radians();

        // Normalize time to a fraction of a virtual day (86400 seconds).
        let t = self.time / 86400.0;
        let theta = t * std::f32::consts::TAU * self.rate_of_change;

        // Direction offset using multiple sine harmonics (from the C# source).
        let dir_offset =
            theta.sin() * (theta * 2.0).sin() * (theta * 9.0).sin() * (theta * 4.0).cos();
        let wind_dir = avg_ang + var_dir * dir_offset;

        // Speed offset using different harmonics.
        let spd_offset = theta.sin() * (theta * 4.0).sin() + (theta * 13.0).sin() / 3.0;
        let wind_speed = (self.avg_strength + self.variance_strength * spd_offset).abs();

        // Normalize direction then scale by speed.
        let dx = wind_dir.cos();
        let dy = wind_dir.sin();
        self.current_wind = (dx * wind_speed, dy * wind_speed);
    }
}

// ---------------------------------------------------------------------------
// WindModule
// ---------------------------------------------------------------------------

/// Region wind module that holds a pluggable wind model and drives it
/// through the scene frame loop.
///
/// Rust port of the C# `WindModule`.
pub struct WindModule {
    enabled: bool,
    frame_update_rate: u32,
    frame_counter: u32,
    model: Box<dyn WindModel>,
    model_name: String,
}

impl WindModule {
    pub fn new() -> Self {
        Self {
            enabled: true,
            frame_update_rate: 150,
            frame_counter: 0,
            model: Box::new(SimpleRandomWind::new()),
            model_name: String::from("SimpleRandomWind"),
        }
    }

    /// Replace the active wind model.
    pub fn set_model(&mut self, model: Box<dyn WindModel>) {
        self.model_name = model.name().to_string();
        self.model = model;
        tracing::debug!(model = %self.model_name, "wind model changed");
    }

    /// Returns the name of the active wind model.
    pub fn active_model_name(&self) -> &str {
        &self.model_name
    }

    /// Returns whether the module is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Query the wind speed at a position.
    pub fn wind_speed(&self, x: f32, y: f32) -> (f32, f32) {
        self.model.wind_speed(x, y)
    }

    /// Advance the wind simulation by one frame tick.
    ///
    /// The actual wind model is updated only every `frame_update_rate`
    /// frames, matching the C# behaviour.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        self.frame_counter += 1;
        if self.frame_counter >= self.frame_update_rate {
            self.frame_counter = 0;
            self.model.update(dt);
        }
    }

    /// Returns the frame-update rate (how many frames between wind updates).
    pub fn frame_update_rate(&self) -> u32 {
        self.frame_update_rate
    }

    /// Returns a reference to the active wind model.
    pub fn model(&self) -> &dyn WindModel {
        self.model.as_ref()
    }

    /// Returns a mutable reference to the active wind model.
    pub fn model_mut(&mut self) -> &mut dyn WindModel {
        self.model.as_mut()
    }
}

impl Default for WindModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for WindModule {
    fn name(&self) -> &str {
        "WindModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.enabled = config.get_bool("Wind", "enabled", true);
        self.frame_update_rate = config.get_int("Wind", "wind_update_rate", 150) as u32;

        let plugin_name = config.get_string("Wind", "wind_plugin", "SimpleRandomWind");
        match plugin_name.as_str() {
            "ConfigurableWind" => {
                let mut cw = ConfigurableWind::new();
                cw.avg_strength = config.get_float("Wind", "avg_strength", 5.0) as f32;
                cw.avg_direction = config.get_float("Wind", "avg_direction", 0.0) as f32;
                cw.variance_strength = config.get_float("Wind", "var_strength", 5.0) as f32;
                cw.variance_direction = config.get_float("Wind", "var_direction", 30.0) as f32;
                cw.rate_of_change = config.get_float("Wind", "rate_change", 1.0) as f32;
                self.model_name = "ConfigurableWind".to_string();
                self.model = Box::new(cw);
            }
            _ => {
                let mut sw = SimpleRandomWind::new();
                sw.strength = config.get_float("Wind", "strength", 1.0) as f32;
                self.model_name = "SimpleRandomWind".to_string();
                self.model = Box::new(sw);
            }
        }

        if self.enabled {
            tracing::info!(
                model = %self.model_name,
                update_rate = self.frame_update_rate,
                "WindModule initialised"
            );
        }
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        if self.enabled {
            tracing::info!(region = %scene.region_name, "WindModule attached to region");
        }
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "WindModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}
