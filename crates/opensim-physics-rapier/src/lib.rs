// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use opensim_physics::{ActorType, ContactResult, PhysicsActor, PhysicsScene, RayFilterFlags};

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
    pub shape_type: PhysicsShapeType,
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
            shape_type: PhysicsShapeType::Prim,
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
        Vec3::ZERO
    }
    fn set_force(&mut self, _force: Vec3) {
        self.dirty = true;
    }
    fn torque(&self) -> Vec3 {
        Vec3::ZERO
    }
    fn set_torque(&mut self, _torque: Vec3) {
        self.dirty = true;
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
        }
    }

    fn flush_dirty(&mut self) {
        let ids: Vec<u32> = self.actors.keys().copied().collect();
        for id in ids {
            let actor = match self.actors.get(&id) {
                Some(a) if a.dirty || !a.pending_forces.is_empty() || !a.pending_torques.is_empty() => a,
                _ => continue,
            };
            let handle = actor.body_handle;
            let pos = actor.position;
            let vel = actor.velocity;
            let ori = actor.orientation;
            let forces: Vec<(Vec3, bool)> = actor.pending_forces.clone();
            let torques: Vec<(Vec3, bool)> = actor.pending_torques.clone();
            let was_dirty = actor.dirty;

            if let Some(body) = self.bodies.get_mut(handle) {
                if was_dirty {
                    body.set_translation(to_rapier(pos), true);
                    body.set_linvel(to_rapier(vel), true);
                    body.set_rotation(to_rapier_quat(ori), true);
                }
                for (f, _) in &forces {
                    body.add_force(to_rapier(*f), true);
                }
                for (t, _) in &torques {
                    body.add_torque(to_rapier(*t), true);
                }
            }

            if let Some(a) = self.actors.get_mut(&id) {
                a.dirty = false;
                a.pending_forces.clear();
                a.pending_torques.clear();
            }
        }
    }

    fn sync_from_rapier(&mut self) {
        for actor in self.actors.values_mut() {
            if let Some(body) = self.bodies.get(actor.body_handle) {
                actor.position = from_rapier(body.translation());
                actor.velocity = from_rapier(body.linvel());
                actor.orientation = from_rapier_quat(*body.rotation());
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

        let collider = ColliderBuilder::cuboid(
            (size.x / 2.0).max(0.01),
            (size.y / 2.0).max(0.01),
            (size.z / 2.0).max(0.01),
        )
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

    fn set_terrain(&mut self, _height_map: &[f32]) {
        if let Some(old) = self.terrain_handle.take() {
            self.bodies.remove(
                old,
                &mut self.islands,
                &mut self.colliders,
                &mut self.impulse_joints,
                &mut self.multibody_joints,
                &mut self.soft_bodies,
                true,
            );
        }
        let body = RigidBodyBuilder::fixed()
            .translation(Vector::new(128.0, 128.0, 12.5))
            .build();
        let handle = self.bodies.insert(body);
        let collider = ColliderBuilder::cuboid(128.0, 128.0, 12.5).build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
        self.terrain_handle = Some(handle);
    }

    fn set_water_level(&mut self, height: f32) {
        self.water_level = height;
    }

    fn get_top_colliders(&self) -> Vec<(u32, f32)> {
        Vec::new()
    }

    fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        length: f32,
        _count: u32,
        _flags: RayFilterFlags,
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
        if let Some((collider_handle, toi)) = qp.cast_ray(&ray, length, true) {
            let hit = ray.origin + ray.dir * toi;
            let consumer_id = self
                .colliders
                .get(collider_handle)
                .and_then(|c| c.parent())
                .and_then(|bh| {
                    self.actors
                        .values()
                        .find(|a| a.body_handle == bh)
                        .map(|a| a.local_id)
                })
                .unwrap_or(0);
            results.push(ContactResult {
                pos: Vec3::new(hit.x, hit.y, hit.z),
                normal: Vec3::Z,
                depth: toi,
                consumer_id,
            });
        }
        results
    }

    fn dispose(&mut self) {
        self.actors.clear();
        self.bodies = RigidBodySet::new();
        self.colliders = ColliderSet::new();
        self.terrain_handle = None;
    }
}
