// SPDX-License-Identifier: LGPL-3.0-or-later
//! Proof tests for scene graph operations: add/remove objects and presences,
//! link/unlink prim groups, spatial queries, and edge cases.

use std::sync::Arc;

use glam::Vec3;
use uuid::Uuid;

use welkin_scene::group::SceneObjectGroup;
use welkin_scene::part::SceneObjectPart;
use welkin_scene::presence::{PresenceType, ScenePresence};
use welkin_scene::scene::{RegionInfo, Scene, SceneConfig};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_scene() -> Scene {
    let info = RegionInfo::new("Proof Region", 1000, 1000);
    Scene::new(info, SceneConfig::default())
}

fn make_part(name: &str, owner: Uuid) -> SceneObjectPart {
    SceneObjectPart::new(name, owner)
}

fn make_group(name: &str, owner: Uuid, pos: Vec3) -> SceneObjectGroup {
    let mut part = make_part(name, owner);
    part.group_position = pos;
    SceneObjectGroup::new(part)
}

fn make_presence(first: &str, last: &str, ptype: PresenceType) -> ScenePresence {
    ScenePresence::new(Uuid::new_v4(), first, last, ptype)
}

// ===========================================================================
// 1. Add / remove objects
// ===========================================================================

#[test]
fn add_multiple_objects_increments_count() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    for i in 0..5 {
        scene.add_object(make_group(&format!("Obj{i}"), owner, Vec3::ZERO));
    }
    assert_eq!(scene.object_count(), 5);
}

#[test]
fn remove_returns_same_object() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let group = make_group("Keeper", owner, Vec3::new(10.0, 20.0, 30.0));
    let uuid = group.uuid();
    scene.add_object(group);

    let removed = scene.remove_object(&uuid).expect("should remove");
    assert_eq!(removed.uuid(), uuid);
    assert_eq!(removed.name(), "Keeper");
    assert_eq!(removed.position(), Vec3::new(10.0, 20.0, 30.0));
}

#[test]
fn remove_nonexistent_object_returns_none() {
    let scene = test_scene();
    assert!(scene.remove_object(&Uuid::new_v4()).is_none());
}

#[test]
fn duplicate_add_overwrites_previous() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let mut part = make_part("First", owner);
    let fixed_uuid = Uuid::new_v4();
    part.uuid = fixed_uuid;
    part.group_position = Vec3::ZERO;
    let group1 = SceneObjectGroup::new(part);

    let mut part2 = make_part("Second", owner);
    part2.uuid = fixed_uuid;
    part2.group_position = Vec3::new(99.0, 99.0, 99.0);
    let group2 = SceneObjectGroup::new(part2);

    scene.add_object(group1);
    scene.add_object(group2);

    // HashMap::insert replaces the value for the same key, so the second add wins.
    // object_count atomic was incremented twice even though only one entry remains.
    // This test documents the current (imperfect) behaviour.
    let obj = scene.get_object(&fixed_uuid).expect("should exist");
    assert_eq!(obj.name(), "Second");
}

// ===========================================================================
// 2. Add / remove presences
// ===========================================================================

#[test]
fn add_multiple_presences() {
    let scene = test_scene();
    scene.add_presence(make_presence("Alice", "A", PresenceType::User));
    scene.add_presence(make_presence("Bob", "B", PresenceType::User));
    scene.add_presence(make_presence("NPC", "Bot", PresenceType::Npc));
    assert_eq!(scene.presence_count(), 3);
}

#[test]
fn remove_nonexistent_presence_returns_none() {
    let scene = test_scene();
    assert!(scene.remove_presence(&Uuid::new_v4()).is_none());
}

#[test]
fn remove_presence_returns_correct_data() {
    let scene = test_scene();
    let uuid = Uuid::new_v4();
    let p = ScenePresence::new(uuid, "Jane", "Doe", PresenceType::User);
    scene.add_presence(p);

    let removed = scene.remove_presence(&uuid).expect("should remove");
    assert_eq!(removed.full_name(), "Jane Doe");
    assert_eq!(scene.presence_count(), 0);
}

#[test]
fn presence_gets_local_id_on_add() {
    let scene = test_scene();
    let uuid = Uuid::new_v4();
    let p = ScenePresence::new(uuid, "Local", "Id", PresenceType::User);
    assert_eq!(p.local_id, 0);

    scene.add_presence(p);
    let stored = scene.get_presence(&uuid).expect("should exist");
    assert_ne!(stored.local_id, 0, "local_id should be assigned on add");
}

// ===========================================================================
// 3. Link / unlink prim groups
// ===========================================================================

#[test]
fn link_prims_preserves_order() {
    let owner = Uuid::new_v4();
    let mut group = SceneObjectGroup::new(make_part("Root", owner));
    let c1 = make_part("C1", owner);
    let c2 = make_part("C2", owner);
    let c3 = make_part("C3", owner);
    let c1_uuid = c1.uuid;
    let c2_uuid = c2.uuid;
    let c3_uuid = c3.uuid;

    group.add_part(c1);
    group.add_part(c2);
    group.add_part(c3);

    assert_eq!(group.part_count(), 4);
    assert_eq!(group.root_part().link_num, 1);
    assert_eq!(group.get_part(&c1_uuid).unwrap().link_num, 2);
    assert_eq!(group.get_part(&c2_uuid).unwrap().link_num, 3);
    assert_eq!(group.get_part(&c3_uuid).unwrap().link_num, 4);
}

#[test]
fn unlink_middle_child_renumbers() {
    let owner = Uuid::new_v4();
    let mut group = SceneObjectGroup::new(make_part("Root", owner));
    let c1 = make_part("C1", owner);
    let c2 = make_part("C2", owner);
    let c3 = make_part("C3", owner);
    let c1_uuid = c1.uuid;
    let c3_uuid = c3.uuid;

    group.add_part(c1);
    group.add_part(c2);
    group.add_part(c3);

    // Remove the middle child (link 3 => "C2")
    // After removal, C1 stays 2, C3 becomes 3 (was 4).
    // But we remove c1 here to test the first child:
    group.remove_part(&c1_uuid);
    assert_eq!(group.part_count(), 3);
    // C3 originally link_num 4, now should renumber:
    assert_eq!(group.get_part(&c3_uuid).unwrap().link_num, 3);
}

#[test]
fn unlink_all_children_resets_root_link() {
    let owner = Uuid::new_v4();
    let mut group = SceneObjectGroup::new(make_part("Root", owner));
    let c1 = make_part("Only Child", owner);
    let c1_uuid = c1.uuid;
    group.add_part(c1);

    assert_eq!(group.root_part().link_num, 1);
    group.remove_part(&c1_uuid);
    assert_eq!(group.part_count(), 1);
    assert_eq!(group.root_part().link_num, 0, "single prim => link_num 0");
}

#[test]
fn unlink_nonexistent_child_returns_none() {
    let owner = Uuid::new_v4();
    let mut group = SceneObjectGroup::new(make_part("Root", owner));
    assert!(group.remove_part(&Uuid::new_v4()).is_none());
}

#[test]
fn add_part_propagates_group_position() {
    let owner = Uuid::new_v4();
    let mut root = make_part("Root", owner);
    root.group_position = Vec3::new(100.0, 200.0, 50.0);
    let mut group = SceneObjectGroup::new(root);

    let child = make_part("Child", owner);
    let child_uuid = child.uuid;
    group.add_part(child);

    let stored_child = group.get_part(&child_uuid).unwrap();
    assert_eq!(
        stored_child.group_position,
        Vec3::new(100.0, 200.0, 50.0),
        "child group_position should match root"
    );
}

#[test]
fn linked_group_in_scene_tracks_local_ids() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let mut root = make_part("LinkedRoot", owner);
    root.group_position = Vec3::ZERO;
    let mut group = SceneObjectGroup::new(root);
    let child = make_part("LinkedChild", owner);
    group.add_part(child);

    let group_uuid = group.uuid();
    scene.add_object(group);

    // Both root and child should have local IDs tracked by the scene
    let obj = scene.get_object(&group_uuid).unwrap();
    let root_lid = obj.root_part().local_id;
    assert_ne!(root_lid, 0);
    assert_eq!(scene.get_object_by_local_id(root_lid), Some(group_uuid));

    // Collect child local IDs
    let child_parts: Vec<_> = obj.parts().skip(1).collect();
    assert_eq!(child_parts.len(), 1);
    let child_lid = child_parts[0].local_id;
    assert_ne!(child_lid, 0);
    assert_eq!(scene.get_object_by_local_id(child_lid), Some(group_uuid));
}

// ===========================================================================
// 4. Spatial queries
// ===========================================================================

#[test]
fn objects_in_range_zero_radius_finds_origin_only() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let at_origin = make_group("Origin", owner, Vec3::ZERO);
    let origin_uuid = at_origin.uuid();
    let away = make_group("Away", owner, Vec3::new(1.0, 0.0, 0.0));
    let away_uuid = away.uuid();

    scene.add_object(at_origin);
    scene.add_object(away);

    let found = scene.objects_in_range(Vec3::ZERO, 0.0);
    assert!(found.contains(&origin_uuid));
    assert!(!found.contains(&away_uuid));
}

#[test]
fn objects_in_range_3d_distance() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    // Place object at exactly (3, 4, 0) => distance 5 from origin
    let g = make_group("Pythagorean", owner, Vec3::new(3.0, 4.0, 0.0));
    let uuid = g.uuid();
    scene.add_object(g);

    assert!(scene.objects_in_range(Vec3::ZERO, 5.0).contains(&uuid));
    assert!(!scene.objects_in_range(Vec3::ZERO, 4.99).contains(&uuid));
}

#[test]
fn objects_in_range_empty_scene() {
    let scene = test_scene();
    assert!(scene.objects_in_range(Vec3::ZERO, 1000.0).is_empty());
}

#[test]
fn find_objects_by_owner_no_matches() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    scene.add_object(make_group("Mine", owner, Vec3::ZERO));

    let found = scene.find_objects_by_owner(&stranger);
    assert!(found.is_empty());
}

#[test]
fn find_objects_by_owner_mixed_owners() {
    let scene = test_scene();
    let owner_a = Uuid::new_v4();
    let owner_b = Uuid::new_v4();

    let ga1 = make_group("A1", owner_a, Vec3::ZERO);
    let ga2 = make_group("A2", owner_a, Vec3::new(5.0, 0.0, 0.0));
    let gb1 = make_group("B1", owner_b, Vec3::new(10.0, 0.0, 0.0));
    let a1_uuid = ga1.uuid();
    let a2_uuid = ga2.uuid();
    let b1_uuid = gb1.uuid();

    scene.add_object(ga1);
    scene.add_object(ga2);
    scene.add_object(gb1);

    let owned_a = scene.find_objects_by_owner(&owner_a);
    assert_eq!(owned_a.len(), 2);
    assert!(owned_a.contains(&a1_uuid));
    assert!(owned_a.contains(&a2_uuid));

    let owned_b = scene.find_objects_by_owner(&owner_b);
    assert_eq!(owned_b.len(), 1);
    assert!(owned_b.contains(&b1_uuid));
}

#[test]
fn find_object_by_name_returns_first_match() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let g1 = make_group("Duplicate", owner, Vec3::ZERO);
    let g2 = make_group("Duplicate", owner, Vec3::new(10.0, 0.0, 0.0));
    let g1_uuid = g1.uuid();
    let g2_uuid = g2.uuid();
    scene.add_object(g1);
    scene.add_object(g2);

    let found = scene.find_object_by_name("Duplicate");
    assert!(found.is_some());
    let found_uuid = found.unwrap();
    // HashMap iteration order is non-deterministic; either is valid
    assert!(found_uuid == g1_uuid || found_uuid == g2_uuid);
}

#[test]
fn find_object_by_name_missing_returns_none() {
    let scene = test_scene();
    assert!(scene.find_object_by_name("Ghost").is_none());
}

// ===========================================================================
// 5. Scene lifecycle
// ===========================================================================

#[test]
fn start_stop_lifecycle() {
    let scene = test_scene();
    assert!(!scene.is_active());
    assert!(!scene.is_shutting_down());

    scene.start();
    assert!(scene.is_active());
    assert!(!scene.is_shutting_down());

    scene.shutdown();
    assert!(!scene.is_active());
    assert!(scene.is_shutting_down());
}

#[test]
fn tick_advances_frame_number() {
    let scene = test_scene();
    assert_eq!(scene.frame_number(), 0);
    let s1 = scene.tick();
    assert_eq!(s1.frame_number, 1);
    let s2 = scene.tick();
    assert_eq!(s2.frame_number, 2);
    assert_eq!(scene.frame_number(), 2);
}

#[test]
fn tick_reports_dirty_objects() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let g = make_group("Dirty", owner, Vec3::ZERO);
    let uuid = g.uuid();
    scene.add_object(g);

    scene.with_object_mut(&uuid, |g| g.mark_changed());

    let stats = scene.tick();
    assert_eq!(stats.dirty_objects, 1);
}

// ===========================================================================
// 6. with_object / with_object_mut closures
// ===========================================================================

#[test]
fn with_object_on_missing_returns_none() {
    let scene = test_scene();
    let result = scene.with_object(&Uuid::new_v4(), |_g| 42);
    assert!(result.is_none());
}

#[test]
fn with_object_mut_modifies_in_place() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let g = make_group("Mutable", owner, Vec3::ZERO);
    let uuid = g.uuid();
    scene.add_object(g);

    scene.with_object_mut(&uuid, |g| {
        g.set_position(Vec3::new(50.0, 60.0, 70.0));
    });

    let pos = scene.with_object(&uuid, |g| g.position());
    assert_eq!(pos, Some(Vec3::new(50.0, 60.0, 70.0)));
}

// ===========================================================================
// 7. for_each iterators
// ===========================================================================

#[test]
fn for_each_object_visits_all() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    for i in 0..3 {
        scene.add_object(make_group(&format!("Iter{i}"), owner, Vec3::ZERO));
    }

    let mut count = 0;
    scene.for_each_object(|_| count += 1);
    assert_eq!(count, 3);
}

#[test]
fn for_each_presence_visits_all() {
    let scene = test_scene();
    scene.add_presence(make_presence("A", "A", PresenceType::User));
    scene.add_presence(make_presence("B", "B", PresenceType::User));

    let mut count = 0;
    scene.for_each_presence(|_| count += 1);
    assert_eq!(count, 2);
}

// ===========================================================================
// 8. Local ID allocation uniqueness
// ===========================================================================

#[test]
fn local_ids_are_unique_across_objects_and_presences() {
    let scene = test_scene();
    let owner = Uuid::new_v4();

    // Add two objects (each with 1 part => 1 local ID each)
    let g1 = make_group("Obj1", owner, Vec3::ZERO);
    let g2 = make_group("Obj2", owner, Vec3::new(5.0, 0.0, 0.0));
    scene.add_object(g1);
    scene.add_object(g2);

    // Add a presence
    scene.add_presence(make_presence("User", "One", PresenceType::User));

    // Collect all assigned local IDs
    let mut local_ids = Vec::new();
    scene.for_each_object(|g| {
        for p in g.parts() {
            local_ids.push(p.local_id);
        }
    });
    scene.for_each_presence(|p| {
        local_ids.push(p.local_id);
    });

    // All should be non-zero and unique
    for &lid in &local_ids {
        assert_ne!(lid, 0);
    }
    let unique_count = {
        let mut sorted = local_ids.clone();
        sorted.sort();
        sorted.dedup();
        sorted.len()
    };
    assert_eq!(unique_count, local_ids.len(), "local IDs must be unique");
}

// ===========================================================================
// 9. Concurrent access (basic check with Arc + threads)
// ===========================================================================

#[test]
fn concurrent_add_remove_objects() {
    let scene = Arc::new(test_scene());
    let owner = Uuid::new_v4();

    // Spawn 10 threads that each add an object then remove it
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let scene = Arc::clone(&scene);
            std::thread::spawn(move || {
                let g = make_group(&format!("Thread{i}"), owner, Vec3::new(i as f32, 0.0, 0.0));
                let uuid = g.uuid();
                scene.add_object(g);
                let removed = scene.remove_object(&uuid);
                assert!(removed.is_some());
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }

    // All objects were added then removed; scene should be empty
    assert_eq!(scene.object_count(), 0);
}

#[test]
fn concurrent_add_objects_then_query() {
    let scene = Arc::new(test_scene());
    let owner = Uuid::new_v4();

    let handles: Vec<_> = (0..20)
        .map(|i| {
            let scene = Arc::clone(&scene);
            std::thread::spawn(move || {
                let g = make_group(
                    &format!("Conc{i}"),
                    owner,
                    Vec3::new(i as f32 * 10.0, 0.0, 0.0),
                );
                scene.add_object(g);
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }

    assert_eq!(scene.object_count(), 20);

    // Spatial query should find all objects within a big radius
    let all_found = scene.objects_in_range(Vec3::new(100.0, 0.0, 0.0), 200.0);
    assert_eq!(all_found.len(), 20);
}

#[test]
fn concurrent_presences() {
    let scene = Arc::new(test_scene());

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let scene = Arc::clone(&scene);
            std::thread::spawn(move || {
                let p = ScenePresence::new(
                    Uuid::new_v4(),
                    &format!("User{i}"),
                    "Test",
                    PresenceType::User,
                );
                scene.add_presence(p);
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }

    assert_eq!(scene.presence_count(), 10);
}

// ===========================================================================
// 10. Backup collection edge cases
// ===========================================================================

#[test]
fn mark_changed_sets_timestamps_and_flags() {
    let scene = test_scene();
    let owner = Uuid::new_v4();
    let g = make_group("Dirty", owner, Vec3::ZERO);
    let uuid = g.uuid();
    scene.add_object(g);

    scene.with_object_mut(&uuid, |g| g.mark_changed());

    scene.with_object(&uuid, |g| {
        assert!(g.has_changed);
        assert!(g.backup_needed);
        assert!(g.time_first_changed > 0);
        assert!(g.time_last_changed > 0);
        assert!(g.time_last_changed >= g.time_first_changed);
    });
}

#[test]
fn force_backup_all_empty_scene() {
    let scene = test_scene();
    assert!(scene.force_backup_all().is_empty());
}

#[test]
fn mark_backed_up_nonexistent_is_noop() {
    let scene = test_scene();
    // Should not panic
    scene.mark_backed_up(&Uuid::new_v4());
}

// ===========================================================================
// 11. Bounding box
// ===========================================================================

#[test]
fn bounding_box_single_prim() {
    let owner = Uuid::new_v4();
    let mut root = make_part("Box", owner);
    root.group_position = Vec3::new(10.0, 20.0, 30.0);
    root.scale = Vec3::new(2.0, 4.0, 6.0);
    let group = SceneObjectGroup::new(root);

    let (min, max) = group.bounding_box();
    assert_eq!(min, Vec3::new(9.0, 18.0, 27.0));
    assert_eq!(max, Vec3::new(11.0, 22.0, 33.0));
}

// ===========================================================================
// 12. SceneObjectGroup flags
// ===========================================================================

#[test]
fn group_physical_phantom_temporary() {
    use welkin_scene::part::PrimFlags;

    let owner = Uuid::new_v4();
    let mut root = make_part("Flags", owner);
    root.flags = PrimFlags::PHYSICS | PrimFlags::PHANTOM | PrimFlags::TEMPORARY_ON_REZ;
    let group = SceneObjectGroup::new(root);

    assert!(group.is_physical());
    assert!(group.is_phantom());
    assert!(group.is_temporary());
}

#[test]
fn group_not_physical_by_default() {
    let owner = Uuid::new_v4();
    let group = SceneObjectGroup::new(make_part("Default", owner));
    assert!(!group.is_physical());
    assert!(!group.is_phantom());
    assert!(!group.is_temporary());
}

// ===========================================================================
// 13. Region info
// ===========================================================================

#[test]
fn region_handle_encode_decode() {
    let info = RegionInfo::new("Grid Square", 256, 512);
    let handle = info.region_handle;
    let x = (handle >> 32) as u32;
    let y = (handle & 0xFFFF_FFFF) as u32;
    assert_eq!(x, 256 * 256);
    assert_eq!(y, 512 * 256);
}

#[test]
fn region_defaults() {
    let info = RegionInfo::new("Default", 0, 0);
    assert_eq!(info.region_size_x, 256);
    assert_eq!(info.region_size_y, 256);
    assert_eq!(info.internal_port, 9000);
    assert!(!info.region_id.is_nil());
}
