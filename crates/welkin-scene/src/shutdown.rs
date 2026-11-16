// SPDX-License-Identifier: LGPL-3.0-or-later

use std::time::{Duration, Instant};

use crate::scene::Scene;

/// Orchestrates a graceful shutdown sequence for a single [`Scene`].
///
/// The sequence persists all dirty objects, counts presences that would be
/// disconnected, and reports statistics.  Actual database writes and client
/// disconnects will be wired in later; for now persistence is simulated by
/// calling [`Scene::mark_backed_up`] on every dirty object.
pub struct GracefulShutdown {
    persist_timeout: Duration,
}

/// Statistics produced by [`GracefulShutdown::execute`].
#[derive(Debug, Clone)]
pub struct ShutdownReport {
    /// Number of objects whose dirty state was flushed (simulated persistence).
    pub objects_persisted: usize,
    /// Number of presences that were present at shutdown time.
    pub presences_removed: usize,
    /// Wall-clock time the shutdown sequence took.
    pub elapsed: Duration,
    /// `true` when the sequence exceeded [`GracefulShutdown::persist_timeout`].
    pub timed_out: bool,
}

impl GracefulShutdown {
    pub fn new(persist_timeout: Duration) -> Self {
        Self { persist_timeout }
    }

    /// Execute the full shutdown sequence for a single scene.
    ///
    /// 1. Transition the scene to *shutting-down* state.
    /// 2. Force-collect every dirty object and mark each as backed up.
    /// 3. Record the presence count (real disconnect is future work).
    /// 4. Return a [`ShutdownReport`] with timing and counts.
    pub async fn execute(&self, scene: &Scene) -> ShutdownReport {
        let start = Instant::now();

        // 1. Signal the scene that shutdown has begun.
        scene.shutdown();

        // 2. Collect and "persist" all dirty objects.
        let dirty = scene.force_backup_all();
        let objects_persisted = dirty.len();
        for uuid in &dirty {
            scene.mark_backed_up(uuid);
        }

        // 3. Count presences (would be disconnected in a real implementation).
        let presences_removed = scene.presence_count();

        // 4. Build report.
        let elapsed = start.elapsed();
        let timed_out = elapsed > self.persist_timeout;

        ShutdownReport {
            objects_persisted,
            presences_removed,
            elapsed,
            timed_out,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::SceneObjectGroup;
    use crate::part::SceneObjectPart;
    use crate::presence::{PresenceType, ScenePresence};
    use crate::scene::{RegionInfo, SceneConfig};
    use glam::Vec3;
    use uuid::Uuid;

    fn test_scene() -> Scene {
        let info = RegionInfo::new("Test Region", 1000, 1000);
        Scene::new(info, SceneConfig::default())
    }

    fn test_group(name: &str, owner: Uuid, position: Vec3) -> SceneObjectGroup {
        let mut part = SceneObjectPart::new(name, owner);
        part.group_position = position;
        SceneObjectGroup::new(part)
    }

    #[tokio::test]
    async fn test_graceful_shutdown_empty_scene() {
        let scene = test_scene();
        scene.start();

        let shutdown = GracefulShutdown::new(Duration::from_secs(30));
        let report = shutdown.execute(&scene).await;

        assert_eq!(report.objects_persisted, 0);
        assert_eq!(report.presences_removed, 0);
        assert!(!report.timed_out);
        assert!(scene.is_shutting_down());
        assert!(!scene.is_active());
    }

    #[tokio::test]
    async fn test_graceful_shutdown_with_dirty_objects() {
        let scene = test_scene();
        scene.start();

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
        // New objects count as unsaved; start from a clean scene.
        for id in [u1, u2, u3] {
            scene.mark_backed_up(&id);
        }

        // Mark two objects dirty.
        scene.with_object_mut(&u1, |g| g.mark_changed());
        scene.with_object_mut(&u2, |g| g.mark_changed());

        let shutdown = GracefulShutdown::new(Duration::from_secs(30));
        let report = shutdown.execute(&scene).await;

        assert_eq!(report.objects_persisted, 2);
        assert!(!report.timed_out);

        // All dirty flags should have been cleared.
        assert!(scene.force_backup_all().is_empty());
    }

    #[tokio::test]
    async fn test_shutdown_report_fields() {
        let scene = test_scene();
        scene.start();

        let owner = Uuid::new_v4();
        let g = test_group("Obj", owner, Vec3::ZERO);
        let uuid = g.uuid();
        scene.add_object(g);
        scene.with_object_mut(&uuid, |g| g.mark_changed());

        let presence = ScenePresence::new(Uuid::new_v4(), "Test", "User", PresenceType::User);
        scene.add_presence(presence);

        let shutdown = GracefulShutdown::new(Duration::from_secs(30));
        let report = shutdown.execute(&scene).await;

        assert_eq!(report.objects_persisted, 1);
        assert_eq!(report.presences_removed, 1);
        assert!(report.elapsed > Duration::ZERO);
        assert!(!report.timed_out);
    }
}
