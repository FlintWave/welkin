// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use uuid::Uuid;

use crate::scene::Scene;

/// Manages multiple `Scene` instances within a single process,
/// keyed by their `region_id`.
pub struct RegionManager {
    regions: RwLock<HashMap<Uuid, Arc<Scene>>>,
}

impl RegionManager {
    pub fn new() -> Self {
        Self {
            regions: RwLock::new(HashMap::new()),
        }
    }

    /// Add a scene. Returns the region_id used as the key.
    pub fn add_region(&self, scene: Arc<Scene>) -> Uuid {
        let region_id = scene.region_info.region_id;
        self.regions.write().unwrap().insert(region_id, scene);
        tracing::info!(region_id = %region_id, "region added to manager");
        region_id
    }

    /// Remove a scene by region_id. Returns the `Scene` if found.
    pub fn remove_region(&self, region_id: &Uuid) -> Option<Arc<Scene>> {
        let removed = self.regions.write().unwrap().remove(region_id);
        if removed.is_some() {
            tracing::info!(region_id = %region_id, "region removed from manager");
        }
        removed
    }

    /// Get a scene by region_id.
    pub fn get_region(&self, region_id: &Uuid) -> Option<Arc<Scene>> {
        self.regions.read().unwrap().get(region_id).cloned()
    }

    /// Get a scene by region handle (`loc_x << 32 | loc_y`).
    pub fn get_region_by_handle(&self, handle: u64) -> Option<Arc<Scene>> {
        self.regions
            .read()
            .unwrap()
            .values()
            .find(|s| s.region_info.region_handle == handle)
            .cloned()
    }

    /// Number of regions currently managed.
    pub fn region_count(&self) -> usize {
        self.regions.read().unwrap().len()
    }

    /// Start all managed regions.
    pub fn start_all(&self) {
        let regions = self.regions.read().unwrap();
        for scene in regions.values() {
            scene.start();
        }
        tracing::info!(count = regions.len(), "all regions started");
    }

    /// Shut down all managed regions.
    pub fn shutdown_all(&self) {
        let regions = self.regions.read().unwrap();
        for scene in regions.values() {
            scene.shutdown();
        }
        tracing::info!(count = regions.len(), "all regions shut down");
    }

    /// Iterate over all managed regions.
    pub fn for_each_region<F>(&self, mut f: F)
    where
        F: FnMut(&Arc<Scene>),
    {
        let regions = self.regions.read().unwrap();
        for scene in regions.values() {
            f(scene);
        }
    }
}

impl Default for RegionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{RegionInfo, Scene, SceneConfig};

    fn make_scene(name: &str, loc_x: u32, loc_y: u32) -> Arc<Scene> {
        let info = RegionInfo::new(name, loc_x, loc_y);
        Arc::new(Scene::new(info, SceneConfig::default()))
    }

    #[test]
    fn test_add_and_get_region() {
        let mgr = RegionManager::new();
        let scene = make_scene("Alpha", 1000, 1000);
        let region_id = scene.region_info.region_id;

        let returned_id = mgr.add_region(scene);
        assert_eq!(returned_id, region_id);
        assert_eq!(mgr.region_count(), 1);

        let fetched = mgr.get_region(&region_id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().region_info.region_name, "Alpha");

        // Unknown UUID returns None
        assert!(mgr.get_region(&Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_remove_region() {
        let mgr = RegionManager::new();
        let scene = make_scene("Beta", 2000, 2000);
        let region_id = scene.region_info.region_id;

        mgr.add_region(scene);
        assert_eq!(mgr.region_count(), 1);

        let removed = mgr.remove_region(&region_id);
        assert!(removed.is_some());
        assert_eq!(mgr.region_count(), 0);
        assert!(mgr.get_region(&region_id).is_none());

        // Removing again returns None
        assert!(mgr.remove_region(&region_id).is_none());
    }

    #[test]
    fn test_get_by_handle() {
        let mgr = RegionManager::new();
        let s1 = make_scene("Region A", 1000, 2000);
        let s2 = make_scene("Region B", 3000, 4000);
        let handle_a = s1.region_info.region_handle;
        let handle_b = s2.region_info.region_handle;

        mgr.add_region(s1);
        mgr.add_region(s2);

        let found_a = mgr.get_region_by_handle(handle_a);
        assert!(found_a.is_some());
        assert_eq!(found_a.unwrap().region_info.region_name, "Region A");

        let found_b = mgr.get_region_by_handle(handle_b);
        assert!(found_b.is_some());
        assert_eq!(found_b.unwrap().region_info.region_name, "Region B");

        // Non-existent handle
        let bogus_handle = ((9999u64) << 32) | 9999u64;
        assert!(mgr.get_region_by_handle(bogus_handle).is_none());
    }

    #[test]
    fn test_multiple_regions() {
        let mgr = RegionManager::new();
        let s1 = make_scene("One", 100, 100);
        let s2 = make_scene("Two", 200, 200);
        let s3 = make_scene("Three", 300, 300);

        mgr.add_region(Arc::clone(&s1));
        mgr.add_region(Arc::clone(&s2));
        mgr.add_region(Arc::clone(&s3));
        assert_eq!(mgr.region_count(), 3);

        // Start all and verify
        mgr.start_all();
        assert!(s1.is_active());
        assert!(s2.is_active());
        assert!(s3.is_active());
        assert!(!s1.is_shutting_down());

        // Shutdown all and verify
        mgr.shutdown_all();
        assert!(!s1.is_active());
        assert!(!s2.is_active());
        assert!(!s3.is_active());
        assert!(s1.is_shutting_down());
        assert!(s2.is_shutting_down());
        assert!(s3.is_shutting_down());
    }

    #[test]
    fn test_for_each_region() {
        let mgr = RegionManager::new();
        mgr.add_region(make_scene("X", 10, 10));
        mgr.add_region(make_scene("Y", 20, 20));

        let mut names = Vec::new();
        mgr.for_each_region(|s| {
            names.push(s.region_info.region_name.clone());
        });
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"X".to_string()));
        assert!(names.contains(&"Y".to_string()));
    }
}
