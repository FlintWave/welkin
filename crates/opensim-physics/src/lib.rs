// SPDX-License-Identifier: GPL-3.0-or-later

pub mod null;
pub mod vehicle;

use glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ActorType {
    Unknown = 0,
    Agent = 1,
    Prim = 2,
    Ground = 3,
    Water = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PidHoverType {
    Ground,
    GroundAndWater,
    Water,
    Absolute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayFilterFlags(pub u16);

impl RayFilterFlags {
    pub const WATER: Self = Self(0x01);
    pub const LAND: Self = Self(0x02);
    pub const AGENT: Self = Self(0x04);
    pub const NONPHYSICAL: Self = Self(0x08);
    pub const PHYSICAL: Self = Self(0x10);
    pub const PHANTOM: Self = Self(0x20);
    pub const VOLUME_DETECT: Self = Self(0x40);
    pub const CONTACTS_UNIMPORTANT: Self = Self(0x2000);
    pub const BACK_FACE_CULL: Self = Self(0x4000);
    pub const CLOSEST_HIT: Self = Self(0x8000);
    pub const ALL: Self = Self(0x3f);
}

impl std::ops::BitOr for RayFilterFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for RayFilterFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ContactResult {
    pub pos: Vec3,
    pub normal: Vec3,
    pub depth: f32,
    pub consumer_id: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct ContactPoint {
    pub position: Vec3,
    pub surface_normal: Vec3,
    pub penetration_depth: f32,
    pub relative_speed: f32,
    pub character_feet: bool,
}

pub trait PhysicsActor {
    fn local_id(&self) -> u32;
    fn name(&self) -> &str;
    fn actor_type(&self) -> ActorType;
    fn position(&self) -> Vec3;
    fn set_position(&mut self, pos: Vec3);
    fn velocity(&self) -> Vec3;
    fn set_velocity(&mut self, vel: Vec3);
    fn size(&self) -> Vec3;
    fn set_size(&mut self, size: Vec3);
    fn orientation(&self) -> Quat;
    fn set_orientation(&mut self, ori: Quat);
    fn acceleration(&self) -> Vec3;
    fn rotational_velocity(&self) -> Vec3;
    fn set_rotational_velocity(&mut self, vel: Vec3);
    fn force(&self) -> Vec3;
    fn set_force(&mut self, force: Vec3);
    fn torque(&self) -> Vec3;
    fn set_torque(&mut self, torque: Vec3);
    fn mass(&self) -> f32;
    fn is_physical(&self) -> bool;
    fn set_physical(&mut self, physical: bool);
    fn is_phantom(&self) -> bool;
    fn set_phantom(&mut self, phantom: bool);
    fn flying(&self) -> bool;
    fn set_flying(&mut self, flying: bool);
    fn is_colliding(&self) -> bool;
    fn colliding_ground(&self) -> bool;
    fn stopped(&self) -> bool;
    fn density(&self) -> f32;
    fn set_density(&mut self, d: f32);
    fn friction(&self) -> f32;
    fn set_friction(&mut self, f: f32);
    fn restitution(&self) -> f32;
    fn set_restitution(&mut self, r: f32);
    fn gravity_modifier(&self) -> f32;
    fn set_gravity_modifier(&mut self, g: f32);
    fn buoyancy(&self) -> f32;
    fn set_buoyancy(&mut self, b: f32);
    fn add_force(&mut self, force: Vec3, push: bool);
    fn add_angular_force(&mut self, force: Vec3, push: bool);
    fn set_momentum(&mut self, momentum: Vec3);
    fn link(&mut self, child_local_id: u32);
    fn delink(&mut self);
}

pub trait PhysicsScene {
    fn name(&self) -> &str;
    fn engine_type(&self) -> &str;
    fn add_avatar(
        &mut self,
        local_id: u32,
        name: &str,
        position: Vec3,
        velocity: Vec3,
        size: Vec3,
        is_flying: bool,
    ) -> u32;
    fn remove_avatar(&mut self, local_id: u32);
    fn add_prim(
        &mut self,
        local_id: u32,
        name: &str,
        position: Vec3,
        size: Vec3,
        rotation: Quat,
        is_physical: bool,
    ) -> u32;
    fn remove_prim(&mut self, local_id: u32);
    fn get_actor(&self, local_id: u32) -> Option<&dyn PhysicsActor>;
    fn get_actor_mut(&mut self, local_id: u32) -> Option<&mut dyn PhysicsActor>;
    fn simulate(&mut self, time_step: f32) -> f32;
    fn set_terrain(&mut self, height_map: &[f32]);
    fn set_water_level(&mut self, height: f32);
    fn get_top_colliders(&self) -> Vec<(u32, f32)>;
    fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        length: f32,
        count: u32,
        flags: RayFilterFlags,
    ) -> Vec<ContactResult>;
    fn dispose(&mut self);
}

#[derive(Debug, thiserror::Error)]
pub enum PhysicsError {
    #[error("actor not found: {0}")]
    ActorNotFound(u32),
    #[error("invalid terrain: {0}")]
    TerrainInvalid(String),
    #[error("engine error: {0}")]
    EngineError(String),
}
