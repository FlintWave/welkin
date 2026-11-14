// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use glam::{Quat, Vec3};

use crate::{ActorType, ContactResult, PhysicsActor, PhysicsScene, RayFilterFlags};

pub struct NullActor {
    local_id: u32,
    name: String,
    actor_type: ActorType,
    position: Vec3,
    velocity: Vec3,
    size: Vec3,
    orientation: Quat,
    is_physical: bool,
    is_phantom: bool,
    flying: bool,
    density: f32,
    friction: f32,
    restitution: f32,
    gravity_modifier: f32,
    buoyancy: f32,
}

impl PhysicsActor for NullActor {
    fn local_id(&self) -> u32 {
        self.local_id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn actor_type(&self) -> ActorType {
        self.actor_type
    }
    fn position(&self) -> Vec3 {
        self.position
    }
    fn set_position(&mut self, pos: Vec3) {
        self.position = pos;
    }
    fn velocity(&self) -> Vec3 {
        self.velocity
    }
    fn set_velocity(&mut self, vel: Vec3) {
        self.velocity = vel;
    }
    fn size(&self) -> Vec3 {
        self.size
    }
    fn set_size(&mut self, size: Vec3) {
        self.size = size;
    }
    fn orientation(&self) -> Quat {
        self.orientation
    }
    fn set_orientation(&mut self, ori: Quat) {
        self.orientation = ori;
    }
    fn acceleration(&self) -> Vec3 {
        Vec3::ZERO
    }
    fn rotational_velocity(&self) -> Vec3 {
        Vec3::ZERO
    }
    fn set_rotational_velocity(&mut self, _vel: Vec3) {}
    fn force(&self) -> Vec3 {
        Vec3::ZERO
    }
    fn set_force(&mut self, _force: Vec3) {}
    fn torque(&self) -> Vec3 {
        Vec3::ZERO
    }
    fn set_torque(&mut self, _torque: Vec3) {}
    fn mass(&self) -> f32 {
        1.0
    }
    fn is_physical(&self) -> bool {
        self.is_physical
    }
    fn set_physical(&mut self, physical: bool) {
        self.is_physical = physical;
    }
    fn is_phantom(&self) -> bool {
        self.is_phantom
    }
    fn set_phantom(&mut self, phantom: bool) {
        self.is_phantom = phantom;
    }
    fn flying(&self) -> bool {
        self.flying
    }
    fn set_flying(&mut self, flying: bool) {
        self.flying = flying;
    }
    fn is_colliding(&self) -> bool {
        false
    }
    fn colliding_ground(&self) -> bool {
        false
    }
    fn stopped(&self) -> bool {
        true
    }
    fn density(&self) -> f32 {
        self.density
    }
    fn set_density(&mut self, d: f32) {
        self.density = d;
    }
    fn friction(&self) -> f32 {
        self.friction
    }
    fn set_friction(&mut self, f: f32) {
        self.friction = f;
    }
    fn restitution(&self) -> f32 {
        self.restitution
    }
    fn set_restitution(&mut self, r: f32) {
        self.restitution = r;
    }
    fn gravity_modifier(&self) -> f32 {
        self.gravity_modifier
    }
    fn set_gravity_modifier(&mut self, g: f32) {
        self.gravity_modifier = g;
    }
    fn buoyancy(&self) -> f32 {
        self.buoyancy
    }
    fn set_buoyancy(&mut self, b: f32) {
        self.buoyancy = b;
    }
    fn add_force(&mut self, _force: Vec3, _push: bool) {}
    fn add_angular_force(&mut self, _force: Vec3, _push: bool) {}
    fn set_momentum(&mut self, _momentum: Vec3) {}
    fn link(&mut self, _child_local_id: u32) {}
    fn delink(&mut self) {}
}

pub struct NullPhysicsScene {
    name: String,
    actors: HashMap<u32, NullActor>,
}

impl NullPhysicsScene {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            actors: HashMap::new(),
        }
    }
}

impl PhysicsScene for NullPhysicsScene {
    fn name(&self) -> &str {
        &self.name
    }
    fn engine_type(&self) -> &str {
        "Null"
    }
    fn add_avatar(
        &mut self,
        local_id: u32,
        name: &str,
        position: Vec3,
        velocity: Vec3,
        size: Vec3,
        is_flying: bool,
    ) -> u32 {
        self.actors.insert(
            local_id,
            NullActor {
                local_id,
                name: name.to_string(),
                actor_type: ActorType::Agent,
                position,
                velocity,
                size,
                orientation: Quat::IDENTITY,
                is_physical: false,
                is_phantom: false,
                flying: is_flying,
                density: 1.0,
                friction: 0.5,
                restitution: 0.0,
                gravity_modifier: 1.0,
                buoyancy: 0.0,
            },
        );
        local_id
    }
    fn remove_avatar(&mut self, local_id: u32) {
        self.actors.remove(&local_id);
    }
    fn add_prim(
        &mut self,
        local_id: u32,
        name: &str,
        position: Vec3,
        size: Vec3,
        rotation: Quat,
        is_physical: bool,
    ) -> u32 {
        self.actors.insert(
            local_id,
            NullActor {
                local_id,
                name: name.to_string(),
                actor_type: ActorType::Prim,
                position,
                velocity: Vec3::ZERO,
                size,
                orientation: rotation,
                is_physical,
                is_phantom: false,
                flying: false,
                density: 1.0,
                friction: 0.5,
                restitution: 0.0,
                gravity_modifier: 1.0,
                buoyancy: 0.0,
            },
        );
        local_id
    }
    fn remove_prim(&mut self, local_id: u32) {
        self.actors.remove(&local_id);
    }
    fn get_actor(&self, local_id: u32) -> Option<&dyn PhysicsActor> {
        self.actors.get(&local_id).map(|a| a as &dyn PhysicsActor)
    }
    fn get_actor_mut(&mut self, local_id: u32) -> Option<&mut dyn PhysicsActor> {
        self.actors
            .get_mut(&local_id)
            .map(|a| a as &mut dyn PhysicsActor)
    }
    fn simulate(&mut self, _time_step: f32) -> f32 {
        0.0
    }
    fn set_terrain(&mut self, _height_map: &[f32]) {}
    fn set_water_level(&mut self, _height: f32) {}
    fn get_top_colliders(&self) -> Vec<(u32, f32)> {
        Vec::new()
    }
    fn raycast(
        &self,
        _origin: Vec3,
        _direction: Vec3,
        _length: f32,
        _count: u32,
        _flags: RayFilterFlags,
    ) -> Vec<ContactResult> {
        Vec::new()
    }
    fn dispose(&mut self) {
        self.actors.clear();
    }
}
