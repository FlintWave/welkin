// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use rapier3d::parry::shape::SharedShape;
use rapier3d::parry::utils::Array2;

use serde::{Deserialize, Serialize};

use welkin_physics::{
    ActorType, ContactResult, PhysicsActor, PhysicsScene, PidHoverType, RayFilterFlags,
};

fn to_rapier(v: Vec3) -> Vector {
    Vector::new(v.x, v.y, v.z)
}

fn from_rapier(v: Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

fn to_rapier_quat(q: Quat) -> Rotation {
    Rotation::from_xyzw(q.x, q.y, q.z, q.w)
}

fn from_rapier_quat(r: Rotation) -> Quat {
    Quat::from_xyzw(r.x, r.y, r.z, r.w)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsShapeType {
    Prim,
    Convex,
    None,
}

pub struct RapierActor {
    local_id: u32,
    name: String,
    actor_type: ActorType,
    body_handle: RigidBodyHandle,
    collider_handle: ColliderHandle,
    position: Vec3,
    velocity: Vec3,
    orientation: Quat,
    size: Vec3,
    is_physical: bool,
    phantom: bool,
    flying: bool,
    colliding: bool,
    colliding_ground: bool,
    density: f32,
    friction: f32,
    restitution: f32,
    gravity_modifier: f32,
    buoyancy: f32,
    dirty: bool,
    children: Vec<u32>,
    pending_forces: Vec<(Vec3, bool)>,
    pending_torques: Vec<(Vec3, bool)>,
    collision_score: f32,
    pub shape_type: PhysicsShapeType,
    pid_target: Vec3,
    pid_tau: f32,
    pid_active: bool,
    pid_hover_height: f32,
    pid_hover_tau: f32,
    pid_hover_type: PidHoverType,
    pid_hover_active: bool,
    apid_target: Quat,
    apid_strength: f32,
    apid_damping: f32,
    apid_active: bool,
    /// Persistent world-space force set by scripts (llSetForce), applied every step.
    force: Vec3,
    /// Persistent world-space torque set by scripts (llSetTorque), applied every step.
    torque: Vec3,
}

impl RapierActor {
    fn new(
        local_id: u32,
        name: String,
        actor_type: ActorType,
        body_handle: RigidBodyHandle,
        collider_handle: ColliderHandle,
        position: Vec3,
        size: Vec3,
        is_physical: bool,
    ) -> Self {
        Self {
            local_id,
            name,
            actor_type,
            body_handle,
            collider_handle,
            position,
            velocity: Vec3::ZERO,
            orientation: Quat::IDENTITY,
            size,
            is_physical,
            phantom: false,
            flying: false,
            colliding: false,
            colliding_ground: false,
            density: 1000.0,
            friction: 0.5,
            restitution: 0.0,
            gravity_modifier: 1.0,
            buoyancy: 0.0,
            dirty: false,
            children: Vec::new(),
            pending_forces: Vec::new(),
            pending_torques: Vec::new(),
            collision_score: 0.0,
            shape_type: PhysicsShapeType::Prim,
            pid_target: Vec3::ZERO,
            pid_tau: 1.0,
            pid_active: false,
            pid_hover_height: 0.0,
            pid_hover_tau: 1.0,
            pid_hover_type: PidHoverType::Ground,
            pid_hover_active: false,
            apid_target: Quat::IDENTITY,
            apid_strength: 0.0,
            apid_damping: 0.0,
            apid_active: false,
            force: Vec3::ZERO,
            torque: Vec3::ZERO,
        }
    }
}

impl PhysicsActor for RapierActor {
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
        self.dirty = true;
    }
    fn velocity(&self) -> Vec3 {
        self.velocity
    }
    fn set_velocity(&mut self, vel: Vec3) {
        self.velocity = vel;
        self.dirty = true;
    }
    fn size(&self) -> Vec3 {
        self.size
    }
    fn set_size(&mut self, size: Vec3) {
        self.size = size;
        self.dirty = true;
    }
    fn orientation(&self) -> Quat {
        self.orientation
    }
    fn set_orientation(&mut self, ori: Quat) {
        self.orientation = ori;
        self.dirty = true;
    }
    fn acceleration(&self) -> Vec3 {
        Vec3::ZERO
    }
    fn rotational_velocity(&self) -> Vec3 {
        Vec3::ZERO
    }
    fn set_rotational_velocity(&mut self, _vel: Vec3) {
        self.dirty = true;
    }
    fn force(&self) -> Vec3 {
        self.force
    }
    fn set_force(&mut self, force: Vec3) {
        self.force = force;
    }
    fn torque(&self) -> Vec3 {
        self.torque
    }
    fn set_torque(&mut self, torque: Vec3) {
        self.torque = torque;
    }
    fn mass(&self) -> f32 {
        self.density * self.size.x * self.size.y * self.size.z
    }
    fn is_physical(&self) -> bool {
        self.is_physical
    }
    fn set_physical(&mut self, physical: bool) {
        self.is_physical = physical;
        self.dirty = true;
    }
    fn is_phantom(&self) -> bool {
        self.phantom
    }
    fn set_phantom(&mut self, phantom: bool) {
        self.phantom = phantom;
        self.dirty = true;
    }
    fn flying(&self) -> bool {
        self.flying
    }
    fn set_flying(&mut self, flying: bool) {
        self.flying = flying;
    }
    fn is_colliding(&self) -> bool {
        self.colliding
    }
    fn colliding_ground(&self) -> bool {
        self.colliding_ground
    }
    fn stopped(&self) -> bool {
        !self.is_physical
    }
    fn density(&self) -> f32 {
        self.density
    }
    fn set_density(&mut self, d: f32) {
        self.density = d;
        self.dirty = true;
    }
    fn friction(&self) -> f32 {
        self.friction
    }
    fn set_friction(&mut self, f: f32) {
        self.friction = f;
        self.dirty = true;
    }
    fn restitution(&self) -> f32 {
        self.restitution
    }
    fn set_restitution(&mut self, r: f32) {
        self.restitution = r;
        self.dirty = true;
    }
    fn gravity_modifier(&self) -> f32 {
        self.gravity_modifier
    }
    fn set_gravity_modifier(&mut self, g: f32) {
        self.gravity_modifier = g;
        self.dirty = true;
    }
    fn buoyancy(&self) -> f32 {
        self.buoyancy
    }
    fn set_buoyancy(&mut self, b: f32) {
        self.buoyancy = b;
    }
    fn pid_target(&self) -> Vec3 {
        self.pid_target
    }
    fn set_pid_target(&mut self, target: Vec3) {
        self.pid_target = target;
    }
    fn pid_active(&self) -> bool {
        self.pid_active
    }
    fn set_pid_active(&mut self, active: bool) {
        self.pid_active = active;
    }
    fn pid_tau(&self) -> f32 {
        self.pid_tau
    }
    fn set_pid_tau(&mut self, tau: f32) {
        self.pid_tau = tau;
    }
    fn pid_hover_active(&self) -> bool {
        self.pid_hover_active
    }
    fn set_pid_hover_active(&mut self, active: bool) {
        self.pid_hover_active = active;
    }
    fn pid_hover_height(&self) -> f32 {
        self.pid_hover_height
    }
    fn set_pid_hover_height(&mut self, height: f32) {
        self.pid_hover_height = height;
    }
    fn pid_hover_type(&self) -> PidHoverType {
        self.pid_hover_type
    }
    fn set_pid_hover_type(&mut self, hover_type: PidHoverType) {
        self.pid_hover_type = hover_type;
    }
    fn pid_hover_tau(&self) -> f32 {
        self.pid_hover_tau
    }
    fn set_pid_hover_tau(&mut self, tau: f32) {
        self.pid_hover_tau = tau;
    }
    fn apid_target(&self) -> Quat {
        self.apid_target
    }
    fn set_apid_target(&mut self, target: Quat) {
        self.apid_target = target;
    }
    fn apid_active(&self) -> bool {
        self.apid_active
    }
    fn set_apid_active(&mut self, active: bool) {
        self.apid_active = active;
    }
    fn apid_strength(&self) -> f32 {
        self.apid_strength
    }
    fn set_apid_strength(&mut self, strength: f32) {
        self.apid_strength = strength;
    }
    fn apid_damping(&self) -> f32 {
        self.apid_damping
    }
    fn set_apid_damping(&mut self, damping: f32) {
        self.apid_damping = damping;
    }
    fn add_force(&mut self, force: Vec3, push: bool) {
        self.pending_forces.push((force, push));
    }
    fn add_angular_force(&mut self, force: Vec3, push: bool) {
        self.pending_torques.push((force, push));
    }
    fn set_momentum(&mut self, momentum: Vec3) {
        self.velocity = momentum / self.mass().max(0.001);
        self.dirty = true;
    }
    fn link(&mut self, child_local_id: u32) {
        if !self.children.contains(&child_local_id) {
            self.children.push(child_local_id);
        }
    }
    fn delink(&mut self) {
        self.children.clear();
    }
}

pub struct RapierPhysicsScene {
    scene_name: String,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    soft_bodies: SoftBodySet,
    ccd: CCDSolver,
    params: IntegrationParameters,
    gravity: Vector,
    actors: HashMap<u32, RapierActor>,
    water_level: f32,
    terrain_handle: Option<RigidBodyHandle>,
    terrain_collider_handle: Option<ColliderHandle>,
}

impl RapierPhysicsScene {
    pub fn new(name: &str) -> Self {
        Self {
            scene_name: name.to_string(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft_bodies: SoftBodySet::new(),
            ccd: CCDSolver::new(),
            params: IntegrationParameters {
                dt: 1.0 / 54.5454,
                ..IntegrationParameters::default()
            },
            gravity: Vector::new(0.0, 0.0, -9.8),
            actors: HashMap::new(),
            water_level: 0.0,
            terrain_handle: None,
            terrain_collider_handle: None,
        }
    }

    fn flush_dirty(&mut self) {
        let ids: Vec<u32> = self.actors.keys().copied().collect();
        for id in ids {
            let actor = match self.actors.get(&id) {
                Some(a)
                    if a.dirty || !a.pending_forces.is_empty() || !a.pending_torques.is_empty() =>
                {
                    a
                }
                _ => continue,
            };
            let body_handle = actor.body_handle;
            let coll_handle = actor.collider_handle;
            let pos = actor.position;
            let vel = actor.velocity;
            let ori = actor.orientation;
            let is_phantom = actor.phantom;
            let friction = actor.friction;
            let restitution = actor.restitution;
            let density = actor.density;
            let gravity_modifier = actor.gravity_modifier;
            let forces: Vec<(Vec3, bool)> = actor.pending_forces.clone();
            let torques: Vec<(Vec3, bool)> = actor.pending_torques.clone();
            let was_dirty = actor.dirty;

            if let Some(body) = self.bodies.get_mut(body_handle) {
                if was_dirty {
                    body.set_translation(to_rapier(pos), true);
                    body.set_linvel(to_rapier(vel), true);
                    body.set_rotation(to_rapier_quat(ori), true);
                    body.set_gravity_scale(gravity_modifier, true);
                }
                for (f, push) in &forces {
                    if *push {
                        body.apply_impulse(to_rapier(*f), true);
                    }
                }
                for (t, push) in &torques {
                    if *push {
                        body.apply_torque_impulse(to_rapier(*t), true);
                    }
                }
            }

            if was_dirty && let Some(coll) = self.colliders.get_mut(coll_handle) {
                coll.set_sensor(is_phantom);
                coll.set_friction(friction);
                coll.set_restitution(restitution);
                coll.set_density(density);
            }

            if let Some(a) = self.actors.get_mut(&id) {
                a.dirty = false;
                a.pending_forces.retain(|(_, push)| !*push);
                a.pending_torques.retain(|(_, push)| !*push);
            }
        }
    }

    fn apply_actor_dynamics(&mut self) {
        let ids: Vec<u32> = self.actors.keys().copied().collect();
        for id in ids {
            let actor = match self.actors.get(&id) {
                Some(a) => a,
                None => continue,
            };
            if actor.actor_type != ActorType::Prim || !actor.is_physical || actor.phantom {
                continue;
            }

            let body_handle = actor.body_handle;
            let persistent_force = actor.force;
            let persistent_torque = actor.torque;
            let pending_forces = actor.pending_forces.clone();
            let pending_torques = actor.pending_torques.clone();
            let buoyancy = actor.buoyancy;
            let pid_active = actor.pid_active;
            let pid_target = actor.pid_target;
            let pid_tau = actor.pid_tau;

            let gravity = Vec3::new(self.gravity.x, self.gravity.y, self.gravity.z);

            let Some(body) = self.bodies.get_mut(body_handle) else {
                continue;
            };

            body.reset_forces(false);
            body.reset_torques(false);

            let mass = body.mass();
            let pos = Vec3::new(body.translation().x, body.translation().y, body.translation().z);
            let vel = Vec3::new(body.linvel().x, body.linvel().y, body.linvel().z);

            let mut total_force = persistent_force;
            for (f, push) in &pending_forces {
                if !*push {
                    total_force += *f;
                }
            }
            total_force += -gravity * (mass * buoyancy);

            if pid_active {
                let tau = pid_tau.max(0.2);
                let error = pid_target - pos;
                total_force += (error / (tau * tau) - vel * (2.0 / tau)) * mass;
                total_force += -gravity * mass;
            }

            let mut total_torque = persistent_torque;
            for (t, push) in &pending_torques {
                if !*push {
                    total_torque += *t;
                }
            }

            if total_force != Vec3::ZERO {
                body.add_force(to_rapier(total_force), true);
            }
            if total_torque != Vec3::ZERO {
                body.add_torque(to_rapier(total_torque), true);
            }

            if let Some(actor) = self.actors.get_mut(&id) {
                actor.pending_forces.retain(|(_, push)| *push);
                actor.pending_torques.retain(|(_, push)| *push);
            }
        }
    }

    fn sync_from_rapier(&mut self) {
        for actor in self.actors.values_mut() {
            actor.colliding = false;
            actor.colliding_ground = false;
            if let Some(body) = self.bodies.get(actor.body_handle) {
                actor.position = from_rapier(body.translation());
                actor.velocity = from_rapier(body.linvel());
                actor.orientation = from_rapier_quat(*body.rotation());
            }
        }

        // Build collider→actor lookup for contact processing
        let coll_to_actor: HashMap<ColliderHandle, u32> = self
            .actors
            .values()
            .map(|a| (a.collider_handle, a.local_id))
            .collect();

        for pair in self.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact() {
                continue;
            }
            let id1 = coll_to_actor.get(&pair.collider1).copied();
            let id2 = coll_to_actor.get(&pair.collider2).copied();

            if let Some(id) = id1
                && let Some(a) = self.actors.get_mut(&id)
            {
                a.colliding = true;
                a.collision_score += 1.0;
                if id2.is_none() {
                    a.colliding_ground = true;
                }
            }
            if let Some(id) = id2
                && let Some(a) = self.actors.get_mut(&id)
            {
                a.colliding = true;
                a.collision_score += 1.0;
                if id1.is_none() {
                    a.colliding_ground = true;
                }
            }
        }
    }

    fn insert_actor(
        &mut self,
        local_id: u32,
        name: &str,
        actor_type: ActorType,
        position: Vec3,
        velocity: Vec3,
        size: Vec3,
        rotation: Quat,
        is_physical: bool,
    ) -> u32 {
        self.insert_actor_with_shape(
            local_id,
            name,
            actor_type,
            position,
            velocity,
            size,
            rotation,
            is_physical,
            None,
        )
    }

    fn insert_actor_with_shape(
        &mut self,
        local_id: u32,
        name: &str,
        actor_type: ActorType,
        position: Vec3,
        velocity: Vec3,
        size: Vec3,
        rotation: Quat,
        is_physical: bool,
        shape: Option<SharedShape>,
    ) -> u32 {
        let builder = if is_physical {
            RigidBodyBuilder::dynamic()
        } else {
            RigidBodyBuilder::fixed()
        };
        let body = builder
            .translation(to_rapier(position))
            .linvel(to_rapier(velocity))
            .build();
        let body_handle = self.bodies.insert(body);

        if let Some(body) = self.bodies.get_mut(body_handle) {
            body.set_rotation(to_rapier_quat(rotation), false);
        }

        let collider = match shape {
            Some(s) => ColliderBuilder::new(s),
            None => ColliderBuilder::cuboid(
                (size.x / 2.0).max(0.01),
                (size.y / 2.0).max(0.01),
                (size.z / 2.0).max(0.01),
            ),
        }
        .density(1000.0)
        .friction(0.5)
        .restitution(0.0)
        .build();
        let collider_handle =
            self.colliders
                .insert_with_parent(collider, body_handle, &mut self.bodies);

        let mut actor = RapierActor::new(
            local_id,
            name.to_string(),
            actor_type,
            body_handle,
            collider_handle,
            position,
            size,
            is_physical,
        );
        actor.velocity = velocity;
        actor.orientation = rotation;
        self.actors.insert(local_id, actor);
        local_id
    }

    fn remove_actor(&mut self, local_id: u32) {
        if let Some(actor) = self.actors.remove(&local_id) {
            self.colliders.remove(
                actor.collider_handle,
                &mut self.islands,
                &mut self.bodies,
                &mut self.soft_bodies,
                true,
            );
            self.bodies.remove(
                actor.body_handle,
                &mut self.islands,
                &mut self.colliders,
                &mut self.impulse_joints,
                &mut self.multibody_joints,
                &mut self.soft_bodies,
                true,
            );
        }
    }
}

impl PhysicsScene for RapierPhysicsScene {
    fn name(&self) -> &str {
        &self.scene_name
    }

    fn engine_type(&self) -> &str {
        "Rapier"
    }

    fn add_avatar(
        &mut self,
        local_id: u32,
        name: &str,
        position: Vec3,
        velocity: Vec3,
        size: Vec3,
        _is_flying: bool,
    ) -> u32 {
        self.insert_actor(
            local_id,
            name,
            ActorType::Agent,
            position,
            velocity,
            size,
            Quat::IDENTITY,
            true,
        )
    }

    fn remove_avatar(&mut self, local_id: u32) {
        self.remove_actor(local_id);
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
        self.insert_actor(
            local_id,
            name,
            ActorType::Prim,
            position,
            Vec3::ZERO,
            size,
            rotation,
            is_physical,
        )
    }

    fn add_prim_with_shape(
        &mut self,
        local_id: u32,
        name: &str,
        position: Vec3,
        size: Vec3,
        rotation: Quat,
        is_physical: bool,
        shape: Option<Box<dyn std::any::Any + Send>>,
    ) -> u32 {
        let shared_shape = shape.and_then(|s| s.downcast::<SharedShape>().ok().map(|s| *s));
        self.insert_actor_with_shape(
            local_id,
            name,
            ActorType::Prim,
            position,
            Vec3::ZERO,
            size,
            rotation,
            is_physical,
            shared_shape,
        )
    }

    fn update_prim_shape(
        &mut self,
        local_id: u32,
        shape: Option<Box<dyn std::any::Any + Send>>,
    ) -> bool {
        let actor = match self.actors.get(&local_id) {
            Some(a) => a,
            None => return false,
        };
        let body_handle = actor.body_handle;
        let old_collider_handle = actor.collider_handle;
        let density = actor.density;
        let friction = actor.friction;
        let restitution = actor.restitution;

        // Remove the old collider.
        self.colliders.remove(
            old_collider_handle,
            &mut self.islands,
            &mut self.bodies,
            &mut self.soft_bodies,
            true,
        );

        // Build the replacement collider.
        let collider = match shape {
            Some(boxed) => {
                match boxed.downcast::<SharedShape>() {
                    Ok(ss) => ColliderBuilder::new(*ss),
                    Err(_) => return false, // wrong type — can't do anything
                }
            }
            None => {
                // Phantom mode: tiny sensor collider so the body stays valid.
                ColliderBuilder::ball(0.001).sensor(true)
            }
        }
        .density(density)
        .friction(friction)
        .restitution(restitution)
        .build();

        let new_handle = self
            .colliders
            .insert_with_parent(collider, body_handle, &mut self.bodies);

        // Update the actor's stored handle.
        if let Some(a) = self.actors.get_mut(&local_id) {
            a.collider_handle = new_handle;
        }

        true
    }

    fn remove_prim(&mut self, local_id: u32) {
        self.remove_actor(local_id);
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
        self.flush_dirty();
        self.apply_actor_dynamics();

        self.pipeline.step(
            self.gravity,
            &self.params,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            &mut self.ccd,
            &(),
            &(),
        );

        self.sync_from_rapier();
        1.0
    }

    fn set_terrain(&mut self, height_map: &[f32]) {
        if let Some(ch) = self.terrain_collider_handle.take() {
            self.colliders.remove(
                ch,
                &mut self.islands,
                &mut self.bodies,
                &mut self.soft_bodies,
                true,
            );
        }
        if let Some(bh) = self.terrain_handle.take() {
            self.bodies.remove(
                bh,
                &mut self.islands,
                &mut self.colliders,
                &mut self.impulse_joints,
                &mut self.multibody_joints,
                &mut self.soft_bodies,
                true,
            );
        }

        let side = (height_map.len() as f32).sqrt() as usize;
        if side < 2 {
            return;
        }

        // Parry HeightField: rows advance along Z, cols along X, heights along Y.
        // Our world is Z-up (gravity along -Z). Rotate +90 around X so Y→Z.
        // After rotation, rows (Z) map to -Y, so reverse row order.
        let mut data = Vec::with_capacity(height_map.len());
        for row in (0..side).rev() {
            let start = row * side;
            data.extend_from_slice(&height_map[start..start + side]);
        }

        let heights = Array2::new(side, side, data);
        let shape = SharedShape::heightfield(heights, Vector::new(1.0, 1.0, 1.0));
        let collider = ColliderBuilder::new(shape)
            .friction(0.5)
            .restitution(0.0)
            .build();

        let half = side as f32 / 2.0;
        let body = RigidBodyBuilder::fixed()
            .translation(Vector::new(half, half, 0.0))
            .rotation(Vector::new(std::f32::consts::FRAC_PI_2, 0.0, 0.0))
            .build();
        let bh = self.bodies.insert(body);
        let ch = self
            .colliders
            .insert_with_parent(collider, bh, &mut self.bodies);
        self.terrain_handle = Some(bh);
        self.terrain_collider_handle = Some(ch);
    }

    fn set_water_level(&mut self, height: f32) {
        self.water_level = height;
    }

    fn get_top_colliders(&self) -> Vec<(u32, f32)> {
        let mut scored: Vec<(u32, f32)> = self
            .actors
            .values()
            .filter(|a| a.collision_score > 0.0)
            .map(|a| (a.local_id, a.collision_score))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(25);
        scored
    }

    fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        length: f32,
        _count: u32,
        flags: RayFilterFlags,
    ) -> Vec<ContactResult> {
        let ray = Ray {
            origin: to_rapier(origin),
            dir: to_rapier(direction),
        };
        let qp = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::default(),
        );
        let mut results = Vec::new();
        if let Some((collider_handle, intersection)) =
            qp.cast_ray_and_get_normal(&ray, length, true)
        {
            let is_terrain = self.terrain_collider_handle == Some(collider_handle);

            let body_handle = self.colliders.get(collider_handle).and_then(|c| c.parent());

            let actor =
                body_handle.and_then(|bh| self.actors.values().find(|a| a.body_handle == bh));

            let dominated = if is_terrain {
                (flags & RayFilterFlags::LAND).0 == 0
            } else if let Some(a) = actor {
                let dominated_agent =
                    a.actor_type == ActorType::Agent && (flags & RayFilterFlags::AGENT).0 == 0;
                let dominated_physical = a.is_physical && (flags & RayFilterFlags::PHYSICAL).0 == 0;
                let dominated_nonphysical =
                    !a.is_physical && (flags & RayFilterFlags::NONPHYSICAL).0 == 0;
                let dominated_phantom = a.phantom && (flags & RayFilterFlags::PHANTOM).0 == 0;
                dominated_agent || dominated_physical || dominated_nonphysical || dominated_phantom
            } else {
                false
            };

            if !dominated {
                let hit = ray.origin + ray.dir * intersection.time_of_impact;
                let normal = from_rapier(intersection.normal);
                let consumer_id = actor.map(|a| a.local_id).unwrap_or(0);
                results.push(ContactResult {
                    pos: Vec3::new(hit.x, hit.y, hit.z),
                    normal,
                    depth: intersection.time_of_impact,
                    consumer_id,
                });
            }
        }
        results
    }

    fn dispose(&mut self) {
        self.actors.clear();
        self.bodies = RigidBodySet::new();
        self.colliders = ColliderSet::new();
        self.terrain_handle = None;
        self.terrain_collider_handle = None;
    }
}

#[derive(Serialize, Deserialize)]
pub struct ActorSnapshot {
    pub local_id: u32,
    pub name: String,
    pub actor_type: u8,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub orientation: [f32; 4],
    pub size: [f32; 3],
    pub is_physical: bool,
    pub phantom: bool,
    pub flying: bool,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub gravity_modifier: f32,
    pub buoyancy: f32,
    pub children: Vec<u32>,
    pub shape_type: u8,
    pub pid_target: [f32; 3],
    pub pid_tau: f32,
    pub pid_active: bool,
    pub pid_hover_height: f32,
    pub pid_hover_tau: f32,
    pub pid_hover_type: u8,
    pub pid_hover_active: bool,
    pub apid_target: [f32; 4],
    pub apid_strength: f32,
    pub apid_damping: f32,
    pub apid_active: bool,
}

#[derive(Serialize, Deserialize)]
pub struct SceneSnapshot {
    pub name: String,
    pub water_level: f32,
    pub actors: Vec<ActorSnapshot>,
}

impl RapierPhysicsScene {
    pub fn snapshot(&self) -> SceneSnapshot {
        let actors = self
            .actors
            .values()
            .map(|a| ActorSnapshot {
                local_id: a.local_id,
                name: a.name.clone(),
                actor_type: a.actor_type as u8,
                position: a.position.to_array(),
                velocity: a.velocity.to_array(),
                orientation: [
                    a.orientation.x,
                    a.orientation.y,
                    a.orientation.z,
                    a.orientation.w,
                ],
                size: a.size.to_array(),
                is_physical: a.is_physical,
                phantom: a.phantom,
                flying: a.flying,
                density: a.density,
                friction: a.friction,
                restitution: a.restitution,
                gravity_modifier: a.gravity_modifier,
                buoyancy: a.buoyancy,
                children: a.children.clone(),
                shape_type: a.shape_type as u8,
                pid_target: a.pid_target.to_array(),
                pid_tau: a.pid_tau,
                pid_active: a.pid_active,
                pid_hover_height: a.pid_hover_height,
                pid_hover_tau: a.pid_hover_tau,
                pid_hover_type: a.pid_hover_type as u8,
                pid_hover_active: a.pid_hover_active,
                apid_target: [
                    a.apid_target.x,
                    a.apid_target.y,
                    a.apid_target.z,
                    a.apid_target.w,
                ],
                apid_strength: a.apid_strength,
                apid_damping: a.apid_damping,
                apid_active: a.apid_active,
            })
            .collect();
        SceneSnapshot {
            name: self.scene_name.clone(),
            water_level: self.water_level,
            actors,
        }
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.snapshot())
    }

    pub fn restore(snap: &SceneSnapshot) -> Self {
        let mut scene = Self::new(&snap.name);
        scene.water_level = snap.water_level;
        for a in &snap.actors {
            let actor_type = match a.actor_type {
                1 => ActorType::Agent,
                2 => ActorType::Prim,
                _ => ActorType::Unknown,
            };
            let pos = Vec3::from_array(a.position);
            let vel = Vec3::from_array(a.velocity);
            let size = Vec3::from_array(a.size);
            let rot = Quat::from_xyzw(
                a.orientation[0],
                a.orientation[1],
                a.orientation[2],
                a.orientation[3],
            );
            scene.insert_actor(
                a.local_id,
                &a.name,
                actor_type,
                pos,
                vel,
                size,
                rot,
                a.is_physical,
            );
            if let Some(actor) = scene.actors.get_mut(&a.local_id) {
                actor.phantom = a.phantom;
                actor.flying = a.flying;
                actor.density = a.density;
                actor.friction = a.friction;
                actor.restitution = a.restitution;
                actor.gravity_modifier = a.gravity_modifier;
                actor.buoyancy = a.buoyancy;
                actor.children = a.children.clone();
                actor.shape_type = match a.shape_type {
                    0 => PhysicsShapeType::Prim,
                    1 => PhysicsShapeType::Convex,
                    _ => PhysicsShapeType::None,
                };
                actor.pid_target = Vec3::from_array(a.pid_target);
                actor.pid_tau = a.pid_tau;
                actor.pid_active = a.pid_active;
                actor.pid_hover_height = a.pid_hover_height;
                actor.pid_hover_tau = a.pid_hover_tau;
                actor.pid_hover_type = match a.pid_hover_type {
                    0 => PidHoverType::Ground,
                    1 => PidHoverType::GroundAndWater,
                    2 => PidHoverType::Water,
                    _ => PidHoverType::Absolute,
                };
                actor.pid_hover_active = a.pid_hover_active;
                actor.apid_target = Quat::from_xyzw(
                    a.apid_target[0],
                    a.apid_target[1],
                    a.apid_target[2],
                    a.apid_target[3],
                );
                actor.apid_strength = a.apid_strength;
                actor.apid_damping = a.apid_damping;
                actor.apid_active = a.apid_active;
                actor.dirty = true;
            }
        }
        scene.flush_dirty();
        scene
    }

    pub fn restore_json(json: &str) -> Result<Self, serde_json::Error> {
        let snap: SceneSnapshot = serde_json::from_str(json)?;
        Ok(Self::restore(&snap))
    }
}
