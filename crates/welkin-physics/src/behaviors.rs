// SPDX-License-Identifier: GPL-3.0-or-later

use std::f32::consts::PI;

use glam::{Quat, Vec3};

use crate::PidHoverType;

pub struct MoveToTarget {
    pub target: Vec3,
    pub tau: f32,
    pub active: bool,
}

impl Default for MoveToTarget {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            tau: 1.0,
            active: false,
        }
    }
}

impl MoveToTarget {
    pub fn step(&self, position: Vec3, velocity: Vec3, gravity: Vec3, mass: f32, dt: f32) -> Vec3 {
        if !self.active || self.tau <= 0.0 || dt <= 0.0 {
            return Vec3::ZERO;
        }

        let error = self.target - position;
        if error.length_squared() < 0.000025 {
            return Vec3::ZERO;
        }

        let factor = (dt / self.tau).clamp(0.0, 1.0);
        let correction = error * factor;
        let mut force = (correction / dt - velocity) * mass;
        force -= gravity * mass;
        force
    }
}

pub struct PrimHover {
    pub height: f32,
    pub tau: f32,
    pub hover_type: PidHoverType,
    pub active: bool,
}

impl Default for PrimHover {
    fn default() -> Self {
        Self {
            height: 0.0,
            tau: 1.0,
            hover_type: PidHoverType::Ground,
            active: false,
        }
    }
}

impl PrimHover {
    pub fn step(
        &self,
        position_z: f32,
        velocity_z: f32,
        terrain_height: f32,
        water_height: f32,
        mass: f32,
        dt: f32,
    ) -> f32 {
        if !self.active || self.tau <= 0.0 || dt <= 0.0 {
            return 0.0;
        }

        let target_height = match self.hover_type {
            PidHoverType::Ground => terrain_height + self.height,
            PidHoverType::GroundAndWater => {
                if terrain_height > water_height {
                    terrain_height + self.height
                } else {
                    water_height + self.height
                }
            }
            PidHoverType::Water => water_height + self.height,
            PidHoverType::Absolute => self.height,
        };

        let error = target_height - position_z;
        let factor = (dt / self.tau).clamp(0.0, 1.0);
        let correction = error * factor;
        (correction / dt - velocity_z) * mass
    }
}

pub struct RotLookAt {
    pub target: Quat,
    pub strength: f32,
    pub damping: f32,
    pub active: bool,
}

impl Default for RotLookAt {
    fn default() -> Self {
        Self {
            target: Quat::IDENTITY,
            strength: 0.0,
            damping: 0.0,
            active: false,
        }
    }
}

impl RotLookAt {
    pub fn step(&self, current_rot: Quat, angular_velocity: Vec3, _dt: f32) -> Vec3 {
        if !self.active || self.strength <= 0.0 {
            return Vec3::ZERO;
        }

        let error_rot = self.target * current_rot.inverse();
        let (axis, mut angle) = error_rot.to_axis_angle();

        if angle > PI {
            angle -= 2.0 * PI;
        }

        if angle.abs() < 0.001 {
            return Vec3::ZERO;
        }

        axis * angle * self.strength - angular_velocity * self.damping
    }
}
