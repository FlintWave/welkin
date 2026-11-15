// SPDX-License-Identifier: LGPL-3.0-or-later

//! Sound playback module for in-world audio.
//!
//! Mirrors the C# `SoundModule` and its `SoundFlags` enumeration, providing
//! triggered, looped, and controlled sound playback primitives.

use bitflags::bitflags;
use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// SoundFlags
// ---------------------------------------------------------------------------

bitflags! {
    /// Sound playback flags, mirroring the C# `SoundModule.SoundFlags`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SoundFlags: u8 {
        const NONE         = 0;
        const LOOP         = 1 << 0;
        const SYNC_MASTER  = 1 << 1;
        const SYNC_SLAVE   = 1 << 2;
        const SYNC_PENDING = 1 << 3;
        const QUEUE        = 1 << 4;
        const STOP         = 1 << 5;
        const SYNC_MASK    = Self::SYNC_MASTER.bits()
                           | Self::SYNC_SLAVE.bits()
                           | Self::SYNC_PENDING.bits();
    }
}

// ---------------------------------------------------------------------------
// SoundTrigger
// ---------------------------------------------------------------------------

/// Describes a triggered sound event in the region.
#[derive(Debug, Clone)]
pub struct SoundTrigger {
    /// Asset UUID of the sound to play.
    pub sound_id: Uuid,
    /// Agent or object that owns this sound.
    pub owner_id: Uuid,
    /// Object that triggered the sound.
    pub object_id: Uuid,
    /// Playback gain (0.0 = silent, 1.0 = full volume).
    pub gain: f32,
    /// World position of the sound source (x, y, z).
    pub position: [f32; 3],
    /// Maximum audible radius.
    pub radius: f32,
    /// Playback flags.
    pub flags: SoundFlags,
}

impl SoundTrigger {
    /// Creates a new sound trigger with sensible defaults.
    pub fn new(sound_id: Uuid, owner_id: Uuid, object_id: Uuid) -> Self {
        Self {
            sound_id,
            owner_id,
            object_id,
            gain: 1.0,
            position: [128.0, 128.0, 25.0],
            radius: 20.0,
            flags: SoundFlags::NONE,
        }
    }
}

// ---------------------------------------------------------------------------
// SoundModule
// ---------------------------------------------------------------------------

/// Region sound playback module.
///
/// Rust port of the C# `SoundModule`.  Provides trigger, loop, stop, and
/// volume-adjust stubs.  In production these would forward to the scene's
/// event pipeline; here they are structural placeholders.
pub struct SoundModule {
    enabled: bool,
    max_distance: f32,
    /// Active looping sounds: (object_id -> sound_id).
    active_loops: Vec<(Uuid, Uuid)>,
}

impl SoundModule {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_distance: 100.0,
            active_loops: Vec::new(),
        }
    }

    /// Returns whether the module is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the configured maximum audible distance.
    pub fn max_distance(&self) -> f32 {
        self.max_distance
    }

    /// Trigger a one-shot sound.
    pub fn trigger_sound(&self, trigger: &SoundTrigger) {
        tracing::debug!(
            sound = %trigger.sound_id,
            object = %trigger.object_id,
            gain = trigger.gain,
            "trigger_sound (stub)"
        );
    }

    /// Start looping a sound on an object.
    pub fn loop_sound(&mut self, object_id: Uuid, sound_id: Uuid, gain: f32) {
        // Remove existing loop on this object, if any.
        self.active_loops.retain(|(oid, _)| *oid != object_id);
        self.active_loops.push((object_id, sound_id));
        tracing::debug!(
            sound = %sound_id,
            object = %object_id,
            gain,
            "loop_sound registered"
        );
    }

    /// Stop a looping sound on an object.
    pub fn stop_sound(&mut self, object_id: Uuid) {
        self.active_loops.retain(|(oid, _)| *oid != object_id);
        tracing::debug!(object = %object_id, "stop_sound");
    }

    /// Adjust the volume of a currently playing sound.
    pub fn adjust_sound_volume(&self, object_id: Uuid, gain: f32) {
        tracing::debug!(
            object = %object_id,
            gain,
            "adjust_sound_volume"
        );
    }

    /// Returns the number of actively looping sounds.
    pub fn active_loop_count(&self) -> usize {
        self.active_loops.len()
    }
}

impl Default for SoundModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for SoundModule {
    fn name(&self) -> &str {
        "SoundModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.enabled = config.get_bool("Sounds", "Enabled", true);
        self.max_distance = config.get_float("Sounds", "MaxDistance", 100.0) as f32;
        if self.enabled {
            tracing::info!(max_distance = self.max_distance, "SoundModule initialised");
        }
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "SoundModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.active_loops.clear();
        tracing::info!(region = %scene.region_name, "SoundModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.active_loops.clear();
        Ok(())
    }
}
