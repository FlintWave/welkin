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
    }

    pub fn has_physics(&self) -> bool {
        self.physics.lock().unwrap().is_some()
    }

    pub fn set_terrain(&self, height_map: &[f32]) {
        if let Ok(mut guard) = self.physics.lock()
            && let Some(ref mut physics) = *guard
        {
            physics.set_terrain(height_map);
        }
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
