// SPDX-License-Identifier: LGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use glam::{Quat, Vec3};
use uuid::Uuid;

use welkin_physics::PhysicsScene;

use crate::group::SceneObjectGroup;
use crate::presence::ScenePresence;
use crate::terrain::TerrainChannel;

#[derive(Debug, Clone)]
pub struct RegionInfo {
    pub region_id: Uuid,
    pub region_name: String,
    pub region_loc_x: u32,
    pub region_loc_y: u32,
    pub region_size_x: u32,
    pub region_size_y: u32,
    pub region_handle: u64,
    pub external_host_name: String,
    pub internal_port: u16,
}

impl RegionInfo {
    pub fn new(name: &str, loc_x: u32, loc_y: u32) -> Self {
        let region_handle = (((loc_x as u64) * 256) << 32) | ((loc_y as u64) * 256);
        Self {
            region_id: Uuid::new_v4(),
            region_name: name.to_string(),
            region_loc_x: loc_x,
            region_loc_y: loc_y,
            region_size_x: 256,
            region_size_y: 256,
            region_handle,
            external_host_name: String::new(),
            internal_port: 9000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SceneConfig {
    pub physics_enabled: bool,
    pub scripts_enabled: bool,
    pub physical_prims: bool,
    pub collidable_prims: bool,
    pub min_nonphys_size: f32,
    pub max_nonphys_size: f32,
    pub min_phys_size: f32,
    pub max_phys_size: f32,
    pub max_prims_per_frame: i32,
    pub linkset_capacity: i32,
    pub default_draw_distance: f32,
    pub max_draw_distance: f32,
    pub frame_time: f32,
    pub persist_min_seconds: i64,
    pub persist_max_seconds: i64,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            physics_enabled: true,
            scripts_enabled: true,
            physical_prims: true,
            collidable_prims: true,
            min_nonphys_size: 0.001,
            max_nonphys_size: 256.0,
            min_phys_size: 0.01,
            max_phys_size: 64.0,
            max_prims_per_frame: 500,
            linkset_capacity: 0,
            default_draw_distance: 256.0,
            max_draw_distance: 512.0,
            frame_time: 1.0 / 11.0,
            persist_min_seconds: 60,
            persist_max_seconds: 600,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameStats {
    pub frame_number: u64,
    pub objects_updated: u32,
    pub presences_updated: u32,
    pub dirty_objects: u32,
    pub physics_ms: f32,
}

/// A physical object whose simulated state changed, for update broadcasting.
#[derive(Debug, Clone)]
pub struct MovedObject {
    pub uuid: Uuid,
    pub local_id: u32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    pub angular_velocity: Vec3,
}

pub struct Scene {
    pub region_info: RegionInfo,
    pub config: SceneConfig,

    objects: RwLock<HashMap<Uuid, SceneObjectGroup>>,
    objects_by_local_id: RwLock<HashMap<u32, Uuid>>,
    presences: RwLock<HashMap<Uuid, ScenePresence>>,
    presences_by_local_id: RwLock<HashMap<u32, Uuid>>,

    next_local_id: AtomicU32,
    frame: AtomicU64,
    active: AtomicBool,
    shutting_down: AtomicBool,

    pub object_count: AtomicU32,
    pub presence_count: AtomicU32,

    started_at: RwLock<Option<Instant>>,
    physics: Mutex<Option<Box<dyn PhysicsScene + Send>>>,
    /// The region's height field: the single source for physics and ground queries.
    terrain: RwLock<TerrainChannel>,
    /// The terrain as last loaded or baked; the `Revert` brush restores from it.
    baked_terrain: RwLock<Vec<f32>>,
    /// Set when terrain is edited; cleared when it has been saved.
    terrain_modified: AtomicBool,
}

/// Highest attachment point number (SL has 55 including HUD points).
pub const MAX_ATTACHMENT_POINT: u8 = 55;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AttachError {
    #[error("invalid attachment point {0}")]
    BadPoint(u8),
    #[error("the avatar is not in the region")]
    NoAvatar,
    #[error("the object is not in the scene")]
    NoObject,
    #[error("the object is already attached")]
    AlreadyAttached,
}

impl Scene {
    pub fn new(region_info: RegionInfo, config: SceneConfig) -> Self {
        Self {
            region_info,
            config,
            objects: RwLock::new(HashMap::new()),
            objects_by_local_id: RwLock::new(HashMap::new()),
            presences: RwLock::new(HashMap::new()),
            presences_by_local_id: RwLock::new(HashMap::new()),
            next_local_id: AtomicU32::new(1),
            frame: AtomicU64::new(0),
            active: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            object_count: AtomicU32::new(0),
            presence_count: AtomicU32::new(0),
            started_at: RwLock::new(None),
            physics: Mutex::new(None),
            terrain: RwLock::new(TerrainChannel::new_default()),
            baked_terrain: RwLock::new(TerrainChannel::new_default().heights().to_vec()),
            terrain_modified: AtomicBool::new(false),
        }
    }

    pub fn allocate_local_id(&self) -> u32 {
        self.next_local_id.fetch_add(1, Ordering::Relaxed)
    }

    // ---- Object management ----

    pub fn add_object(&self, mut group: SceneObjectGroup) -> Uuid {
        let uuid = group.uuid();

        let mut local_ids: Vec<u32> = Vec::new();
        for part in group.parts_mut() {
            if part.local_id == 0 {
                part.local_id = self.allocate_local_id();
            }
            local_ids.push(part.local_id);
        }

        {
            let mut by_local = self.objects_by_local_id.write().unwrap();
            for lid in &local_ids {
                by_local.insert(*lid, uuid);
            }
        }

        // Register with physics before the group moves into the map (the objects
        // lock is not held while the physics lock is taken).
        self.register_group_with_physics(&group);

        self.objects.write().unwrap().insert(uuid, group);
        self.object_count.fetch_add(1, Ordering::Relaxed);

        tracing::debug!(region = %self.region_info.region_name, object = %uuid, "object added to scene");
        uuid
    }

    pub fn remove_object(&self, uuid: &Uuid) -> Option<SceneObjectGroup> {
        let group = self.objects.write().unwrap().remove(uuid)?;

        {
            let mut by_local = self.objects_by_local_id.write().unwrap();
            by_local.retain(|_, v| v != uuid);
        }

        self.object_count.fetch_sub(1, Ordering::Relaxed);
        self.unregister_root_from_physics(group.root_part().local_id);
        tracing::debug!(region = %self.region_info.region_name, object = %uuid, "object removed from scene");
        Some(group)
    }

    pub fn get_object(&self, uuid: &Uuid) -> Option<SceneObjectGroup> {
        self.objects.read().unwrap().get(uuid).cloned()
    }

    /// Resolve a prim UUID (root or child) to the UUID of the object group that owns it.
    pub fn resolve_group_for_prim(&self, prim_id: &Uuid) -> Option<Uuid> {
        let objects = self.objects.read().unwrap();
        if objects.contains_key(prim_id) {
            return Some(*prim_id);
        }
        objects
            .iter()
            .find(|(_, g)| g.get_part(prim_id).is_some())
            .map(|(id, _)| *id)
    }

    pub fn get_object_by_local_id(&self, local_id: u32) -> Option<Uuid> {
        self.objects_by_local_id
            .read()
            .unwrap()
            .get(&local_id)
            .copied()
    }

    pub fn with_object<F, R>(&self, uuid: &Uuid, f: F) -> Option<R>
    where
        F: FnOnce(&SceneObjectGroup) -> R,
    {
        let objects = self.objects.read().unwrap();
        objects.get(uuid).map(f)
    }

    pub fn with_object_mut<F, R>(&self, uuid: &Uuid, f: F) -> Option<R>
    where
        F: FnOnce(&mut SceneObjectGroup) -> R,
    {
        let mut objects = self.objects.write().unwrap();
        objects.get_mut(uuid).map(f)
    }

    pub fn for_each_object<F>(&self, mut f: F)
    where
        F: FnMut(&SceneObjectGroup),
    {
        let objects = self.objects.read().unwrap();
        for group in objects.values() {
            f(group);
        }
    }

    pub fn object_count(&self) -> usize {
        self.objects.read().unwrap().len()
    }

    // ---- Presence management ----

    pub fn add_presence(&self, mut presence: ScenePresence) -> Uuid {
        let uuid = presence.uuid;

        if presence.local_id == 0 {
            presence.local_id = self.allocate_local_id();
        }

        let mut by_local = self.presences_by_local_id.write().unwrap();
        let mut presences = self.presences.write().unwrap();

        if presences.remove(&uuid).is_some() {
            by_local.retain(|_, v| *v != uuid);
            tracing::debug!(region = %self.region_info.region_name, avatar = %uuid, "evicted stale presence before re-add");
        } else {
            self.presence_count.fetch_add(1, Ordering::Relaxed);
        }

        by_local.insert(presence.local_id, uuid);
        presences.insert(uuid, presence);

        tracing::debug!(region = %self.region_info.region_name, avatar = %uuid, "presence added to scene");
        uuid
    }

    pub fn remove_presence(&self, uuid: &Uuid) -> Option<ScenePresence> {
        let presence = self.presences.write().unwrap().remove(uuid)?;

        self.presences_by_local_id
            .write()
            .unwrap()
            .retain(|_, v| v != uuid);

        self.presence_count.fetch_sub(1, Ordering::Relaxed);
        tracing::debug!(region = %self.region_info.region_name, avatar = %uuid, "presence removed from scene");
        Some(presence)
    }

    pub fn get_presence(&self, uuid: &Uuid) -> Option<ScenePresence> {
        self.presences.read().unwrap().get(uuid).cloned()
    }

    pub fn for_each_presence<F>(&self, mut f: F)
    where
        F: FnMut(&ScenePresence),
    {
        let presences = self.presences.read().unwrap();
        for p in presences.values() {
            f(p);
        }
    }

    pub fn presence_count(&self) -> usize {
        self.presences.read().unwrap().len()
    }

    pub fn update_presence_position(&self, uuid: &Uuid, position: Vec3, rotation: Quat) -> bool {
        if let Some(p) = self.presences.write().unwrap().get_mut(uuid) {
            p.position = position;
            p.body_rotation = rotation;
            true
        } else {
            false
        }
    }

    pub fn update_presence_appearance(
        &self,
        uuid: &Uuid,
        texture_entry: Vec<u8>,
        visual_params: Vec<u8>,
    ) -> bool {
        if let Some(p) = self.presences.write().unwrap().get_mut(uuid) {
            p.texture_entry = texture_entry;
            p.visual_params = visual_params;
            p.appearance_serial += 1;
            true
        } else {
            false
        }
    }

    // ---- Spatial queries ----

    pub fn objects_in_range(&self, center: Vec3, range: f32) -> Vec<Uuid> {
        let range_sq = range * range;
        let objects = self.objects.read().unwrap();
        objects
            .iter()
            .filter(|(_, g)| g.position().distance_squared(center) <= range_sq)
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn find_objects_by_owner(&self, owner_id: &Uuid) -> Vec<Uuid> {
        let objects = self.objects.read().unwrap();
        objects
            .iter()
            .filter(|(_, g)| g.root_part().owner_id == *owner_id)
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn find_object_by_name(&self, name: &str) -> Option<Uuid> {
        let objects = self.objects.read().unwrap();
        objects
            .iter()
            .find(|(_, g)| g.name() == name)
            .map(|(id, _)| *id)
    }

    // ---- Physics ----

    pub fn set_physics(&self, physics: Box<dyn PhysicsScene + Send>) {
        *self.physics.lock().unwrap() = Some(physics);
        // Objects added before the backend existed (e.g. restored from storage) must
        // also become physics actors.
        let existing: Vec<SceneObjectGroup> =
            self.objects.read().unwrap().values().cloned().collect();
        for group in &existing {
            self.register_group_with_physics(group);
        }
    }

    /// Run `f` against the physics backend, if one is installed and enabled.
    pub fn with_physics<R>(&self, f: impl FnOnce(&mut dyn PhysicsScene) -> R) -> Option<R> {
        let mut guard = self.physics.lock().ok()?;
        let physics = guard.as_mut()?;
        Some(f(&mut **physics))
    }

    /// Add the group's root prim to the physics world. Non-physical prims become
    /// fixed colliders; phantom prims are registered as sensors. (Linkset children
    /// are not yet separate colliders.)
    fn register_group_with_physics(&self, group: &SceneObjectGroup) {
        if !self.config.physics_enabled {
            return;
        }
        let root = group.root_part();
        if root.local_id == 0 {
            return;
        }
        let physical = group.is_physical();
        let phantom = group.is_phantom();
        let _ = self.with_physics(|ph| {
            ph.add_prim(
                root.local_id,
                &root.name,
                root.group_position,
                root.scale,
                root.rotation_offset,
                physical,
            );
            if phantom && let Some(actor) = ph.get_actor_mut(root.local_id) {
                actor.set_phantom(true);
            }
        });
    }

    /// Push an object's current root transform into its physics actor. Call after
    /// code (scripts, editing) moves an object directly so its collider follows.
    pub fn push_object_to_physics(&self, uuid: &Uuid) {
        let Some((local_id, pos, rot)) = self.with_object(uuid, |g| {
            let r = g.root_part();
            (r.local_id, r.group_position, r.rotation_offset)
        }) else {
            return;
        };
        let _ = self.with_physics(|ph| {
            if let Some(actor) = ph.get_actor_mut(local_id) {
                actor.set_position(pos);
                actor.set_orientation(rot);
            }
        });
    }

    /// Copy simulated state of physical prims back into the scene. Returns the
    /// objects whose transform or motion state changed (including coming to rest),
    /// so the caller can send updates to viewers.
    pub fn sync_physics_to_objects(&self) -> Vec<MovedObject> {
        if !self.config.physics_enabled {
            return Vec::new();
        }
        let physical: Vec<(Uuid, u32)> = self
            .objects
            .read()
            .unwrap()
            .iter()
            .filter(|(_, g)| g.is_physical() && !g.is_phantom())
            .map(|(id, g)| (*id, g.root_part().local_id))
            .collect();
        if physical.is_empty() {
            return Vec::new();
        }
        let states: Vec<(Uuid, u32, Vec3, Vec3, Quat, Vec3)> = self
            .with_physics(|ph| {
                physical
                    .iter()
                    .filter_map(|(id, lid)| {
                        ph.get_actor(*lid).map(|a| {
                            (*id, *lid, a.position(), a.velocity(), a.orientation(), a.rotational_velocity())
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut moved = Vec::new();
        let mut objects = self.objects.write().unwrap();
        for (id, local_id, pos, vel, rot, ang) in states {
            let Some(group) = objects.get_mut(&id) else { continue };
            let old = group.root_part();
            let pos_changed = (old.group_position - pos).length() > 0.005;
            let rot_changed = old.rotation_offset.dot(rot).abs() < 0.99999;
            let at_rest = vel.length() < 0.01 && ang.length() < 0.01;
            let settled = at_rest && (old.velocity != Vec3::ZERO || old.angular_velocity != Vec3::ZERO);
            if !(pos_changed || rot_changed || settled) {
                continue;
            }
            let (vel, ang) = if at_rest { (Vec3::ZERO, Vec3::ZERO) } else { (vel, ang) };
            group.set_position(pos);
            group.set_rotation(rot);
            let root = group.root_part_mut();
            root.velocity = vel;
            root.angular_velocity = ang;
            group.mark_changed();
            moved.push(MovedObject {
                uuid: id,
                local_id,
                position: pos,
                velocity: vel,
                rotation: rot,
                angular_velocity: ang,
            });
        }
        moved
    }

    fn unregister_root_from_physics(&self, local_id: u32) {
        if local_id == 0 {
            return;
        }
        let _ = self.with_physics(|ph| ph.remove_prim(local_id));
    }

    pub fn has_physics(&self) -> bool {
        self.physics.lock().unwrap().is_some()
    }

    /// Terrain height in metres at a region-local position (bilinear between the
    /// four surrounding vertices; positions outside the region clamp to the edge).
    pub fn terrain_height_at(&self, x: f32, y: f32) -> f32 {
        let terrain = self.terrain.read().unwrap();
        let max_x = (terrain.size_x() - 1) as f32;
        let max_y = (terrain.size_y() - 1) as f32;
        let x = x.clamp(0.0, max_x);
        let y = y.clamp(0.0, max_y);
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(terrain.size_x() - 1), (y0 + 1).min(terrain.size_y() - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let h00 = terrain.get(x0, y0);
        let h10 = terrain.get(x1, y0);
        let h01 = terrain.get(x0, y1);
        let h11 = terrain.get(x1, y1);
        let bottom = h00 + (h10 - h00) * fx;
        let top = h01 + (h11 - h01) * fx;
        bottom + (top - bottom) * fy
    }

    pub fn set_terrain(&self, height_map: &[f32]) {
        {
            let mut terrain = self.terrain.write().unwrap();
            let expected = (terrain.size_x() * terrain.size_y()) as usize;
            if height_map.len() == expected {
                terrain.heights_mut().copy_from_slice(height_map);
                terrain.mark_all_dirty();
                *self.baked_terrain.write().unwrap() = height_map.to_vec();
            } else {
                tracing::warn!(
                    got = height_map.len(),
                    expected,
                    "set_terrain: height map size does not match the region; scene terrain unchanged"
                );
            }
        }
        if let Ok(mut guard) = self.physics.lock()
            && let Some(ref mut physics) = *guard
        {
            physics.set_terrain(height_map);
        }
    }

    /// Run a terrain edit; returns the patches that changed (and clears their
    /// dirty flags). The physics height field follows the change.
    fn edit_terrain(&self, edit: impl FnOnce(&mut TerrainChannel, &[f32])) -> Vec<(u32, u32)> {
        let (changed, heights) = {
            let mut terrain = self.terrain.write().unwrap();
            let baked = self.baked_terrain.read().unwrap();
            edit(&mut terrain, baked.as_slice());
            let changed = terrain.dirty_patches();
            terrain.clear_dirty();
            let heights = if changed.is_empty() { Vec::new() } else { terrain.heights().to_vec() };
            (changed, heights)
        };
        if !changed.is_empty() {
            self.terrain_modified.store(true, Ordering::SeqCst);
        }
        if !changed.is_empty()
            && let Ok(mut guard) = self.physics.lock()
            && let Some(ref mut physics) = *guard
        {
            physics.set_terrain(&heights);
        }
        changed
    }

    /// The terrain heights if they were edited since the last save (and marks them
    /// as saved). Call `mark_terrain_modified` again if the save fails.
    pub fn take_terrain_for_backup(&self) -> Option<Vec<f32>> {
        if self.terrain_modified.swap(false, Ordering::SeqCst) {
            Some(self.terrain.read().unwrap().heights().to_vec())
        } else {
            None
        }
    }

    pub fn mark_terrain_modified(&self) {
        self.terrain_modified.store(true, Ordering::SeqCst);
    }

    /// Apply a terraform brush (a viewer `ModifyLand` click). Returns the changed
    /// patches as (patch_x, patch_y).
    pub fn modify_terrain(
        &self,
        action: crate::terrain_paint::PaintAction,
        x: f32,
        y: f32,
        height: f32,
        brush_size: f32,
        strength: f32,
    ) -> Vec<(u32, u32)> {
        self.edit_terrain(|terrain, baked| {
            crate::terrain_paint::paint_effect(terrain, Some(baked), action, x, y, height, brush_size, strength)
        })
    }

    /// Apply an area terraform over a rectangle (west, south, east, north in region metres).
    #[allow(clippy::too_many_arguments)]
    pub fn flood_terrain(
        &self,
        action: crate::terrain_paint::PaintAction,
        height: f32,
        strength: f32,
        west: f32,
        south: f32,
        east: f32,
        north: f32,
    ) -> Vec<(u32, u32)> {
        self.edit_terrain(|terrain, baked| {
            crate::terrain_paint::flood_effect(terrain, Some(baked), action, height, strength, west, south, east, north)
        })
    }

    /// LayerData payloads (the `LayerData` block bytes) for the given patches, split
    /// so every payload fits in one UDP packet.
    pub fn terrain_layer_payloads(&self, patches: &[(u32, u32)]) -> Vec<Vec<u8>> {
        const MAX_PAYLOAD: usize = 1000;
        let terrain = self.terrain.read().unwrap();
        let mut out = Vec::new();
        let mut rest = patches;
        while !rest.is_empty() {
            let mut take = rest.len().min(8);
            loop {
                let data = crate::terrain_layer::encode_land_layer(&terrain, &rest[..take]);
                if data.is_empty() {
                    return out; // region size the encoder does not support
                }
                if data.len() <= MAX_PAYLOAD || take == 1 {
                    out.push(data);
                    break;
                }
                take = take.div_ceil(2);
            }
            rest = &rest[take..];
        }
        out
    }

    // ---- Attachments ----

    /// Attach an object already in the scene to an avatar's attachment point. The
    /// object leaves the physics world (it now follows the avatar) and its root part
    /// takes the avatar as parent. `root.attached_pos` / `rotation_offset` hold the
    /// offset from the attachment point and are left as set by the caller.
    pub fn attach_object(
        &self,
        group_id: &Uuid,
        agent_id: &Uuid,
        point: u8,
        item_id: Uuid,
    ) -> Result<(), AttachError> {
        if point == 0 || point > MAX_ATTACHMENT_POINT {
            return Err(AttachError::BadPoint(point));
        }
        let avatar_local_id = self
            .presences
            .read()
            .unwrap()
            .get(agent_id)
            .map(|p| p.local_id)
            .ok_or(AttachError::NoAvatar)?;
        let root_local_id = {
            let mut objects = self.objects.write().unwrap();
            let group = objects.get_mut(group_id).ok_or(AttachError::NoObject)?;
            if group.is_attachment {
                return Err(AttachError::AlreadyAttached);
            }
            group.is_attachment = true;
            group.attachment_point = point;
            group.attached_avatar = *agent_id;
            group.attachment_item_id = item_id;
            group.root_part_mut().parent_id = avatar_local_id;
            group.root_part_mut().shape.last_attach_point = point;
            group.mark_changed();
            group.root_part().local_id
        };
        self.unregister_root_from_physics(root_local_id);
        if let Some(p) = self.presences.write().unwrap().get_mut(agent_id) {
            p.attachments.retain(|(_, g)| g != group_id);
            p.attachments.push((point, *group_id));
        }
        Ok(())
    }

    /// Detach an attachment and leave it in the world at `position`. Returns false if
    /// the object is not an attachment.
    pub fn detach_object(&self, group_id: &Uuid, position: Vec3) -> bool {
        let agent = {
            let mut objects = self.objects.write().unwrap();
            let Some(group) = objects.get_mut(group_id) else { return false };
            if !group.is_attachment {
                return false;
            }
            let agent = group.attached_avatar;
            group.is_attachment = false;
            group.attachment_point = 0;
            group.attached_avatar = Uuid::nil();
            group.attachment_item_id = Uuid::nil();
            group.root_part_mut().parent_id = 0;
            group.root_part_mut().attached_pos = Vec3::ZERO;
            group.set_position(position);
            group.mark_changed();
            agent
        };
        if let Some(p) = self.presences.write().unwrap().get_mut(&agent) {
            p.attachments.retain(|(_, g)| g != group_id);
        }
        if let Some(group) = self.get_object(group_id) {
            self.register_group_with_physics(&group);
        }
        true
    }

    /// Object ids of the attachments an avatar wears, with their attachment points.
    pub fn attachments_of(&self, agent_id: &Uuid) -> Vec<(u8, Uuid)> {
        let objects = self.objects.read().unwrap();
        let mut out: Vec<(u8, Uuid)> = objects
            .iter()
            .filter(|(_, g)| g.is_attachment && g.attached_avatar == *agent_id)
            .map(|(id, g)| (g.attachment_point, *id))
            .collect();
        out.sort();
        out
    }

    /// Remove every attachment an avatar wears from the scene (logout) and return them,
    /// with their attachment points still set so callers can save them.
    pub fn take_attachments_of(&self, agent_id: &Uuid) -> Vec<SceneObjectGroup> {
        let ids: Vec<Uuid> = self.attachments_of(agent_id).into_iter().map(|(_, id)| id).collect();
        let removed: Vec<SceneObjectGroup> = ids.iter().filter_map(|id| self.remove_object(id)).collect();
        if let Some(p) = self.presences.write().unwrap().get_mut(agent_id) {
            p.attachments.clear();
        }
        removed
    }

    pub fn get_top_colliders(&self) -> Vec<(u32, f32)> {
        if let Ok(guard) = self.physics.lock()
            && let Some(ref physics) = *guard
        {
            physics.get_top_colliders()
        } else {
            Vec::new()
        }
    }

    // ---- Scene loop ----

    pub fn frame_number(&self) -> u64 {
        self.frame.load(Ordering::Relaxed)
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Relaxed)
    }

    pub fn start(&self) {
        self.active.store(true, Ordering::Relaxed);
        self.shutting_down.store(false, Ordering::Relaxed);
        *self.started_at.write().unwrap() = Some(Instant::now());
        tracing::info!(region = %self.region_info.region_name, "scene started");
    }

    pub fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::Relaxed);
        self.active.store(false, Ordering::Relaxed);
        tracing::info!(region = %self.region_info.region_name, "scene shutting down");
    }

    pub fn tick(&self) -> FrameStats {
        let frame_number = self.frame.fetch_add(1, Ordering::Relaxed) + 1;
        let mut physics_ms = 0.0f32;
        if self.config.physics_enabled
            && let Ok(mut guard) = self.physics.lock()
            && let Some(ref mut physics) = *guard
        {
            let start = std::time::Instant::now();
            physics.simulate(self.config.frame_time);
            physics_ms = start.elapsed().as_secs_f32() * 1000.0;
        }
        let mut dirty_objects = 0u32;
        {
            let objects = self.objects.read().unwrap();
            for group in objects.values() {
                if group.backup_needed {
                    dirty_objects += 1;
                }
            }
        }
        FrameStats {
            frame_number,
            objects_updated: 0,
            presences_updated: self.presence_count.load(Ordering::Relaxed),
            dirty_objects,
            physics_ms,
        }
    }

    pub fn prim_stats(&self) -> (u32, u32) {
        let objects = self.objects.read().unwrap();
        let mut total = 0u32;
        let mut active = 0u32;
        for group in objects.values() {
            total += group.part_count() as u32;
            if group.is_physical() {
                active += 1;
            }
        }
        (total, active)
    }

    // ---- Periodic backup ----

    /// Returns UUIDs of objects eligible for backup based on C#-style persistence timing.
    ///
    /// An object is collected when `backup_needed` is true AND either:
    /// - it has been idle (unchanged) for at least `persist_min_seconds`, OR
    /// - it has been dirty for at least `persist_max_seconds` (forced backup).
    pub fn collect_for_backup(&self) -> Vec<Uuid> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let min_s = self.config.persist_min_seconds;
        let max_s = self.config.persist_max_seconds;

        let objects = self.objects.read().unwrap();
        objects
            .iter()
            .filter(|(_, g)| {
                g.backup_needed
                    && ((now - g.time_last_changed >= min_s)
                        || (now - g.time_first_changed >= max_s))
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Clears dirty/backup flags on an object after it has been persisted.
    pub fn mark_backed_up(&self, uuid: &Uuid) {
        let mut objects = self.objects.write().unwrap();
        if let Some(g) = objects.get_mut(uuid) {
            g.backup_needed = false;
            g.has_changed = false;
            g.time_first_changed = 0;
            g.time_last_changed = 0;
        }
    }

    /// Returns UUIDs of ALL objects with `backup_needed == true`, ignoring timing.
    /// Used during shutdown to flush every pending change.
    pub fn force_backup_all(&self) -> Vec<Uuid> {
        let objects = self.objects.read().unwrap();
        objects
            .iter()
            .filter(|(_, g)| g.backup_needed)
            .map(|(id, _)| *id)
            .collect()
    }
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("region", &self.region_info.region_name)
            .field("frame", &self.frame.load(Ordering::Relaxed))
            .field("objects", &self.object_count.load(Ordering::Relaxed))
            .field("presences", &self.presence_count.load(Ordering::Relaxed))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::SceneObjectPart;
    use crate::presence::PresenceType;

    fn test_scene() -> Scene {
        let info = RegionInfo::new("Test Region", 1000, 1000);
        Scene::new(info, SceneConfig::default())
    }

    fn test_part(name: &str, owner: Uuid) -> SceneObjectPart {
        SceneObjectPart::new(name, owner)
    }

    fn test_group(name: &str, owner: Uuid, position: Vec3) -> SceneObjectGroup {
        let mut part = test_part(name, owner);
        part.group_position = position;
        SceneObjectGroup::new(part)
    }

    #[test]
    fn new_scene_is_empty() {
        let scene = test_scene();
        assert_eq!(scene.object_count(), 0);
        assert_eq!(scene.presence_count(), 0);
        assert_eq!(scene.frame_number(), 0);
        assert!(!scene.is_active());
        assert!(!scene.is_shutting_down());
    }

    fn physical_group(name: &str, position: Vec3) -> SceneObjectGroup {
        let mut part = test_part(name, Uuid::new_v4());
        part.group_position = position;
        part.flags |= crate::part::PrimFlags::PHYSICS;
        SceneObjectGroup::new(part)
    }

    #[test]
    fn sync_copies_physical_motion_back_and_reports_rest() {
        let scene = test_scene();
        scene.set_physics(Box::new(welkin_physics::null::NullPhysicsScene::new("t")));
        let uuid = scene.add_object(physical_group("Ball", Vec3::new(5.0, 5.0, 50.0)));
        let local_id = scene.get_object(&uuid).unwrap().root_part().local_id;

        // Nothing moved yet.
        assert!(scene.sync_physics_to_objects().is_empty());

        // Physics moves the actor.
        scene.with_physics(|p| {
            let a = p.get_actor_mut(local_id).unwrap();
            a.set_position(Vec3::new(5.0, 5.0, 40.0));
            a.set_velocity(Vec3::new(0.0, 0.0, -5.0));
        });
        let moved = scene.sync_physics_to_objects();
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].uuid, uuid);
        assert_eq!(moved[0].position, Vec3::new(5.0, 5.0, 40.0));
        let part = scene.get_object(&uuid).unwrap();
        assert_eq!(part.root_part().group_position, Vec3::new(5.0, 5.0, 40.0));
        assert_eq!(part.root_part().velocity, Vec3::new(0.0, 0.0, -5.0));

        // Same state again: nothing to report.
        assert!(scene.sync_physics_to_objects().is_empty());

        // Comes to rest at the same spot: one final update with zero velocity.
        scene.with_physics(|p| p.get_actor_mut(local_id).unwrap().set_velocity(Vec3::ZERO));
        let rest = scene.sync_physics_to_objects();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].velocity, Vec3::ZERO);
        assert!(scene.sync_physics_to_objects().is_empty());
    }

    #[test]
    fn non_physical_objects_are_not_synced_but_moves_reach_physics() {
        let scene = test_scene();
        scene.set_physics(Box::new(welkin_physics::null::NullPhysicsScene::new("t")));
        let uuid = scene.add_object(test_group("Wall", Uuid::new_v4(), Vec3::new(1.0, 1.0, 1.0)));
        let local_id = scene.get_object(&uuid).unwrap().root_part().local_id;
        assert!(scene.sync_physics_to_objects().is_empty());

        scene.with_object_mut(&uuid, |g| g.set_position(Vec3::new(9.0, 9.0, 9.0)));
        scene.push_object_to_physics(&uuid);
        let p = scene.with_physics(|ph| ph.get_actor(local_id).unwrap().position()).unwrap();
        assert_eq!(p, Vec3::new(9.0, 9.0, 9.0));
    }

    #[test]
    fn modify_terrain_raises_ground_and_reports_changed_patches() {
        use crate::terrain_paint::PaintAction;
        let scene = test_scene();
        let before = scene.terrain_height_at(100.0, 100.0);
        let changed = scene.modify_terrain(PaintAction::Raise, 100.0, 100.0, 0.0, 4.0, 2.0);
        assert!((scene.terrain_height_at(100.0, 100.0) - (before + 2.0)).abs() < 1e-3);
        assert!(!changed.is_empty());
        assert!(changed.iter().all(|&(px, py)| px < 16 && py < 16));
        // Dirty flags were consumed: a brush that changes nothing reports nothing.
        assert!(scene.modify_terrain(PaintAction::Raise, 100.0, 100.0, 0.0, 4.0, 0.0).iter().all(|p| changed.contains(p)));
        // Revert restores the baked (loaded) height.
        scene.modify_terrain(PaintAction::Revert, 100.0, 100.0, 0.0, 4.0, 1.0);
        assert!((scene.terrain_height_at(100.0, 100.0) - before).abs() < 1e-3);
    }

    fn scene_with_avatar() -> (Scene, Uuid, u32) {
        use crate::presence::{PresenceType, ScenePresence};
        let scene = test_scene();
        let agent = Uuid::new_v4();
        scene.add_presence(ScenePresence::new(agent, "Test", "Avatar", PresenceType::User));
        let local = scene.get_presence(&agent).unwrap().local_id;
        (scene, agent, local)
    }

    #[test]
    fn attach_parents_the_object_to_the_avatar_and_detach_puts_it_back() {
        let (scene, agent, avatar_local) = scene_with_avatar();
        let obj = scene.add_object(test_group("Hat", agent, Vec3::new(10.0, 10.0, 30.0)));
        let item = Uuid::new_v4();
        scene.attach_object(&obj, &agent, 2, item).unwrap();

        let g = scene.get_object(&obj).unwrap();
        assert!(g.is_attachment);
        assert_eq!((g.attachment_point, g.attached_avatar, g.attachment_item_id), (2, agent, item));
        assert_eq!(g.root_part().parent_id, avatar_local);
        assert_eq!(scene.attachments_of(&agent), vec![(2, obj)]);
        assert_eq!(scene.get_presence(&agent).unwrap().attachments, vec![(2, obj)]);

        assert!(scene.detach_object(&obj, Vec3::new(50.0, 60.0, 25.0)));
        let g = scene.get_object(&obj).unwrap();
        assert!(!g.is_attachment);
        assert_eq!(g.root_part().parent_id, 0);
        assert_eq!(g.position(), Vec3::new(50.0, 60.0, 25.0));
        assert!(scene.attachments_of(&agent).is_empty());
        assert!(scene.get_presence(&agent).unwrap().attachments.is_empty());
        assert!(!scene.detach_object(&obj, Vec3::ZERO), "already detached");
    }

    #[test]
    fn attach_rejects_bad_requests() {
        let (scene, agent, _) = scene_with_avatar();
        let obj = scene.add_object(test_group("Hat", agent, Vec3::ZERO));
        assert_eq!(scene.attach_object(&obj, &agent, 0, Uuid::nil()), Err(AttachError::BadPoint(0)));
        assert_eq!(scene.attach_object(&obj, &agent, 56, Uuid::nil()), Err(AttachError::BadPoint(56)));
        assert_eq!(scene.attach_object(&obj, &Uuid::new_v4(), 2, Uuid::nil()), Err(AttachError::NoAvatar));
        assert_eq!(scene.attach_object(&Uuid::new_v4(), &agent, 2, Uuid::nil()), Err(AttachError::NoObject));
        scene.attach_object(&obj, &agent, 2, Uuid::nil()).unwrap();
        assert_eq!(scene.attach_object(&obj, &agent, 3, Uuid::nil()), Err(AttachError::AlreadyAttached));
    }

    #[test]
    fn logout_takes_all_of_an_avatars_attachments_out_of_the_scene() {
        let (scene, agent, _) = scene_with_avatar();
        let a = scene.add_object(test_group("A", agent, Vec3::ZERO));
        let b = scene.add_object(test_group("B", agent, Vec3::ZERO));
        let ground = scene.add_object(test_group("Ground", agent, Vec3::ZERO));
        scene.attach_object(&a, &agent, 2, Uuid::new_v4()).unwrap();
        scene.attach_object(&b, &agent, 6, Uuid::new_v4()).unwrap();
        let taken = scene.take_attachments_of(&agent);
        assert_eq!(taken.len(), 2);
        assert!(taken.iter().all(|g| g.is_attachment), "still marked so callers can save them");
        assert!(scene.get_object(&a).is_none() && scene.get_object(&b).is_none());
        assert!(scene.get_object(&ground).is_some());
        assert!(scene.get_presence(&agent).unwrap().attachments.is_empty());
    }

    #[test]
    fn attached_objects_leave_the_physics_world() {
        let (scene, agent, _) = scene_with_avatar();
        scene.set_physics(Box::new(welkin_physics::null::NullPhysicsScene::new("t")));
        let obj = scene.add_object(test_group("Hat", agent, Vec3::new(5.0, 5.0, 30.0)));
        let local = scene.get_object(&obj).unwrap().root_part().local_id;
        assert!(scene.with_physics(|ph| ph.get_actor_mut(local).is_some()).unwrap());
        scene.attach_object(&obj, &agent, 2, Uuid::nil()).unwrap();
        assert!(!scene.with_physics(|ph| ph.get_actor_mut(local).is_some()).unwrap());
        scene.detach_object(&obj, Vec3::new(5.0, 5.0, 30.0));
        assert!(scene.with_physics(|ph| ph.get_actor_mut(local).is_some()).unwrap(), "back in the world");
    }

    #[test]
    fn edited_terrain_is_offered_for_backup_once() {
        use crate::terrain_paint::PaintAction;
        let scene = test_scene();
        assert!(scene.take_terrain_for_backup().is_none(), "untouched terrain needs no save");
        scene.set_terrain(&vec![5.0; 256 * 256]);
        assert!(scene.take_terrain_for_backup().is_none(), "loading terrain is not an edit");
        scene.modify_terrain(PaintAction::Raise, 50.0, 50.0, 0.0, 3.0, 1.0);
        let heights = scene.take_terrain_for_backup().expect("edited");
        assert_eq!(heights.len(), 256 * 256);
        assert!(scene.take_terrain_for_backup().is_none());
        scene.mark_terrain_modified();
        assert!(scene.take_terrain_for_backup().is_some());
    }

    #[test]
    fn terrain_layer_payloads_cover_all_patches_and_fit_a_packet() {
        let scene = test_scene();
        let all: Vec<(u32, u32)> = (0..16).flat_map(|py| (0..16).map(move |px| (px, py))).collect();
        let payloads = scene.terrain_layer_payloads(&all);
        let decoded: usize = payloads
            .iter()
            .map(|p| crate::terrain_layer::decode_land_layer(p).unwrap().len())
            .sum();
        assert_eq!(decoded, 256);
        assert!(payloads.iter().all(|p| p.len() <= 1000));
        // Rough terrain still fits: many patches, each possibly large.
        let mut map = vec![0.0f32; 256 * 256];
        for (i, h) in map.iter_mut().enumerate() {
            *h = ((i * 7919) % 97) as f32;
        }
        scene.set_terrain(&map);
        let payloads = scene.terrain_layer_payloads(&all);
        assert!(payloads.iter().all(|p| p.len() <= 1000 || crate::terrain_layer::decode_land_layer(p).unwrap().len() == 1));
        let decoded: usize = payloads.iter().map(|p| crate::terrain_layer::decode_land_layer(p).unwrap().len()).sum();
        assert_eq!(decoded, 256);
    }

    #[test]
    fn terrain_height_is_bilinear_clamped_and_follows_set_terrain() {
        let scene = test_scene();
        // Default region terrain is flat at 21 m.
        assert_eq!(scene.terrain_height_at(100.5, 40.25), 21.0);

        // h = 0.1 * x  (map index = y * 256 + x)
        let mut map = vec![0.0f32; 256 * 256];
        for y in 0..256 {
            for x in 0..256 {
                map[y * 256 + x] = x as f32 * 0.1;
            }
        }
        scene.set_terrain(&map);
        assert!((scene.terrain_height_at(10.0, 5.0) - 1.0).abs() < 1e-4);
        assert!((scene.terrain_height_at(10.5, 5.0) - 1.05).abs() < 1e-4, "interpolates between vertices");
        assert!((scene.terrain_height_at(-20.0, 5.0) - 0.0).abs() < 1e-4, "clamped at the west edge");
        assert!((scene.terrain_height_at(400.0, 5.0) - 25.5).abs() < 1e-3, "clamped at the east edge");

        // A map of the wrong size is rejected and leaves the terrain alone.
        scene.set_terrain(&[5.0; 10]);
        assert!((scene.terrain_height_at(10.0, 5.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn objects_register_with_physics_on_add_and_unregister_on_remove() {
        let scene = test_scene();
        scene.set_physics(Box::new(welkin_physics::null::NullPhysicsScene::new("t")));
        let owner = Uuid::new_v4();
        let uuid = scene.add_object(test_group("Box", owner, Vec3::new(10.0, 10.0, 30.0)));
        let local_id = scene.get_object(&uuid).unwrap().root_part().local_id;

        assert!(scene.with_physics(|p| p.get_actor(local_id).is_some()).unwrap());
        let pos = scene.with_physics(|p| p.get_actor(local_id).unwrap().position()).unwrap();
        assert_eq!(pos, Vec3::new(10.0, 10.0, 30.0));

        scene.remove_object(&uuid);
        assert!(!scene.with_physics(|p| p.get_actor(local_id).is_some()).unwrap());
    }

    #[test]
    fn objects_added_before_physics_are_registered_when_it_arrives() {
        let scene = test_scene();
        let uuid = scene.add_object(test_group("Early", Uuid::new_v4(), Vec3::ZERO));
        let local_id = scene.get_object(&uuid).unwrap().root_part().local_id;

        scene.set_physics(Box::new(welkin_physics::null::NullPhysicsScene::new("t")));
        assert!(scene.with_physics(|p| p.get_actor(local_id).is_some()).unwrap());
    }

    #[test]
    fn resolve_group_for_prim_handles_root_child_and_unknown() {
        let scene = test_scene();
        let owner = Uuid::new_v4();
        let mut group = test_group("Linkset", owner, Vec3::ZERO);
        let group_id = group.uuid();
        let child = test_part("Child", owner);
        let child_id = child.uuid;
        group.add_part(child);
        scene.add_object(group);

        assert_eq!(scene.resolve_group_for_prim(&group_id), Some(group_id));
        assert_eq!(scene.resolve_group_for_prim(&child_id), Some(group_id));
        assert_eq!(scene.resolve_group_for_prim(&Uuid::new_v4()), None);
    }

    #[test]
    fn add_remove_object() {
        let scene = test_scene();
        let owner = Uuid::new_v4();
        let group = test_group("Box", owner, Vec3::ZERO);
        let uuid = group.uuid();

        let returned_uuid = scene.add_object(group);
        assert_eq!(returned_uuid, uuid);
        assert_eq!(scene.object_count(), 1);

        let obj = scene.get_object(&uuid);
        assert!(obj.is_some());
        assert_eq!(obj.unwrap().name(), "Box");

        let removed = scene.remove_object(&uuid);
        assert!(removed.is_some());
        assert_eq!(scene.object_count(), 0);
        assert!(scene.get_object(&uuid).is_none());
    }

    #[test]
    fn add_remove_presence() {
        let scene = test_scene();
        let avatar_uuid = Uuid::new_v4();
        let presence = ScenePresence::new(avatar_uuid, "Test", "User", PresenceType::User);

        let returned = scene.add_presence(presence);
        assert_eq!(returned, avatar_uuid);
        assert_eq!(scene.presence_count(), 1);

        let p = scene.get_presence(&avatar_uuid);
        assert!(p.is_some());
        assert_eq!(p.unwrap().full_name(), "Test User");

        let removed = scene.remove_presence(&avatar_uuid);
        assert!(removed.is_some());
        assert_eq!(scene.presence_count(), 0);
    }

    #[test]
    fn spatial_query_objects_in_range() {
        let scene = test_scene();
        let owner = Uuid::new_v4();

        let near = test_group("Near", owner, Vec3::new(10.0, 10.0, 0.0));
        let far = test_group("Far", owner, Vec3::new(200.0, 200.0, 0.0));
        let near_uuid = near.uuid();
        let far_uuid = far.uuid();

        scene.add_object(near);
        scene.add_object(far);

        let within_50 = scene.objects_in_range(Vec3::ZERO, 50.0);
        assert!(within_50.contains(&near_uuid));
        assert!(!within_50.contains(&far_uuid));

        let within_500 = scene.objects_in_range(Vec3::ZERO, 500.0);
        assert!(within_500.contains(&near_uuid));
        assert!(within_500.contains(&far_uuid));
    }

    #[test]
    fn find_objects_by_owner() {
        let scene = test_scene();
        let owner_a = Uuid::new_v4();
        let owner_b = Uuid::new_v4();

        let g1 = test_group("A1", owner_a, Vec3::ZERO);
        let g2 = test_group("A2", owner_a, Vec3::new(10.0, 0.0, 0.0));
        let g3 = test_group("B1", owner_b, Vec3::new(20.0, 0.0, 0.0));
        let g1_uuid = g1.uuid();
        let g2_uuid = g2.uuid();

        scene.add_object(g1);
        scene.add_object(g2);
        scene.add_object(g3);

        let owned = scene.find_objects_by_owner(&owner_a);
        assert_eq!(owned.len(), 2);
        assert!(owned.contains(&g1_uuid));
        assert!(owned.contains(&g2_uuid));

        let owned_b = scene.find_objects_by_owner(&owner_b);
        assert_eq!(owned_b.len(), 1);
    }

    #[test]
    fn find_object_by_name() {
        let scene = test_scene();
        let owner = Uuid::new_v4();

        let g = test_group("Unique Name", owner, Vec3::ZERO);
        let uuid = g.uuid();
        scene.add_object(g);

        assert_eq!(scene.find_object_by_name("Unique Name"), Some(uuid));
        assert_eq!(scene.find_object_by_name("Missing"), None);
    }

    #[test]
    fn tick_increments_frame() {
        let scene = test_scene();
        scene.start();
        assert!(scene.is_active());

        let stats1 = scene.tick();
        assert_eq!(stats1.frame_number, 1);

        let stats2 = scene.tick();
        assert_eq!(stats2.frame_number, 2);

        scene.shutdown();
        assert!(!scene.is_active());
        assert!(scene.is_shutting_down());
    }

    #[test]
    fn with_object_closure() {
        let scene = test_scene();
        let owner = Uuid::new_v4();
        let group = test_group("Test", owner, Vec3::new(5.0, 10.0, 15.0));
        let uuid = group.uuid();
        scene.add_object(group);

        let pos = scene.with_object(&uuid, |g| g.position());
        assert_eq!(pos, Some(Vec3::new(5.0, 10.0, 15.0)));

        scene.with_object_mut(&uuid, |g| {
            g.set_position(Vec3::new(99.0, 99.0, 99.0));
        });

        let new_pos = scene.with_object(&uuid, |g| g.position());
        assert_eq!(new_pos, Some(Vec3::new(99.0, 99.0, 99.0)));
    }

    #[test]
    fn local_id_allocation() {
        let scene = test_scene();
        let id1 = scene.allocate_local_id();
        let id2 = scene.allocate_local_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn object_by_local_id() {
        let scene = test_scene();
        let owner = Uuid::new_v4();
        let group = test_group("LidTest", owner, Vec3::ZERO);
        let uuid = group.uuid();
        scene.add_object(group);

        let obj = scene.get_object(&uuid).unwrap();
        let root_lid = obj.root_part().local_id;
        assert_ne!(root_lid, 0);

        let found = scene.get_object_by_local_id(root_lid);
        assert_eq!(found, Some(uuid));
    }

    #[test]
    fn region_handle_encoding() {
        let info = RegionInfo::new("Encode", 1000, 2000);
        let expected = ((1000u64 * 256) << 32) | (2000u64 * 256);
        assert_eq!(info.region_handle, expected);
    }

    // ---- Periodic backup tests ----

    #[test]
    fn test_collect_for_backup_empty_scene() {
        let scene = test_scene();
        assert!(scene.collect_for_backup().is_empty());
    }

    #[test]
    fn test_collect_for_backup_clean_objects() {
        let scene = test_scene();
        let owner = Uuid::new_v4();
        // Add objects but never mark them dirty
        scene.add_object(test_group("Clean1", owner, Vec3::ZERO));
        scene.add_object(test_group("Clean2", owner, Vec3::new(10.0, 0.0, 0.0)));
        assert!(scene.collect_for_backup().is_empty());
    }

    #[test]
    fn test_collect_for_backup_recently_changed() {
        let scene = test_scene();
        let owner = Uuid::new_v4();
        let group = test_group("Recent", owner, Vec3::ZERO);
        let uuid = group.uuid();
        scene.add_object(group);

        // Mark dirty with timestamps set to "just now" — not eligible yet
        scene.with_object_mut(&uuid, |g| {
            g.mark_changed();
        });

        // persist_min_seconds is 60, so a just-changed object should NOT be collected
        assert!(scene.collect_for_backup().is_empty());
    }

    #[test]
    fn test_collect_for_backup_idle_long_enough() {
        let scene = test_scene();
        let owner = Uuid::new_v4();
        let group = test_group("Idle", owner, Vec3::ZERO);
        let uuid = group.uuid();
        scene.add_object(group);

        // Simulate: object was changed persist_min_seconds + 1 seconds ago
        let past = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - scene.config.persist_min_seconds
            - 1;

        scene.with_object_mut(&uuid, |g| {
            g.backup_needed = true;
            g.has_changed = true;
            g.time_first_changed = past;
            g.time_last_changed = past;
        });

        let collected = scene.collect_for_backup();
        assert_eq!(collected.len(), 1);
        assert!(collected.contains(&uuid));
    }

    #[test]
    fn test_collect_for_backup_dirty_too_long() {
        // Object keeps being changed (time_last_changed is recent) but
        // time_first_changed exceeds persist_max_seconds — force backup.
        let scene = test_scene();
        let owner = Uuid::new_v4();
        let group = test_group("ForceDirty", owner, Vec3::ZERO);
        let uuid = group.uuid();
        scene.add_object(group);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let long_ago = now - scene.config.persist_max_seconds - 1;

        scene.with_object_mut(&uuid, |g| {
            g.backup_needed = true;
            g.has_changed = true;
            g.time_first_changed = long_ago;
            g.time_last_changed = now; // just changed — still active
        });

        let collected = scene.collect_for_backup();
        assert_eq!(collected.len(), 1);
        assert!(collected.contains(&uuid));
    }

    #[test]
    fn test_force_backup_all() {
        let scene = test_scene();
        let owner = Uuid::new_v4();

        let g1 = test_group("Dirty1", owner, Vec3::ZERO);
        let g2 = test_group("Dirty2", owner, Vec3::new(5.0, 0.0, 0.0));
        let g3 = test_group("Clean", owner, Vec3::new(10.0, 0.0, 0.0));
        let u1 = g1.uuid();
        let u2 = g2.uuid();
        let u3 = g3.uuid();

        scene.add_object(g1);
        scene.add_object(g2);
        scene.add_object(g3);

        // Mark two as needing backup (recently — would NOT pass timed collection)
        scene.with_object_mut(&u1, |g| g.mark_changed());
        scene.with_object_mut(&u2, |g| g.mark_changed());

        let forced = scene.force_backup_all();
        assert_eq!(forced.len(), 2);
        assert!(forced.contains(&u1));
        assert!(forced.contains(&u2));
        assert!(!forced.contains(&u3));
    }

    #[test]
    fn test_mark_backed_up() {
        let scene = test_scene();
        let owner = Uuid::new_v4();
        let group = test_group("BackedUp", owner, Vec3::ZERO);
        let uuid = group.uuid();
        scene.add_object(group);

        // Simulate old dirty object eligible for backup
        let past = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - scene.config.persist_min_seconds
            - 1;
        scene.with_object_mut(&uuid, |g| {
            g.backup_needed = true;
            g.has_changed = true;
            g.time_first_changed = past;
            g.time_last_changed = past;
        });

        // Verify it would be collected
        assert_eq!(scene.collect_for_backup().len(), 1);

        // Mark backed up
        scene.mark_backed_up(&uuid);

        // Should no longer be collected
        assert!(scene.collect_for_backup().is_empty());
        assert!(scene.force_backup_all().is_empty());

        // Verify the flags are fully reset
        scene.with_object(&uuid, |g| {
            assert!(!g.backup_needed);
            assert!(!g.has_changed);
            assert_eq!(g.time_first_changed, 0);
            assert_eq!(g.time_last_changed, 0);
        });
    }
}
