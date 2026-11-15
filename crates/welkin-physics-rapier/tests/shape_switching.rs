// SPDX-License-Identifier: GPL-3.0-or-later

use glam::{Quat, Vec3};
use rapier3d::parry::shape::SharedShape;

use welkin_physics::PhysicsScene;
use welkin_physics_rapier::RapierPhysicsScene;

/// Helper: create a scene with one cuboid prim at local_id 1.
fn scene_with_cuboid() -> RapierPhysicsScene {
    let mut scene = RapierPhysicsScene::new("test");
    scene.add_prim(
        1,
        "box",
        Vec3::new(128.0, 128.0, 25.0),
        Vec3::new(1.0, 1.0, 1.0),
        Quat::IDENTITY,
        false,
    );
    scene
}

#[test]
fn test_update_shape_replaces_collider() {
    let mut scene = scene_with_cuboid();

    // Replace the cuboid with a ball.
    let ball: Box<dyn std::any::Any + Send> = Box::new(SharedShape::ball(2.0));
    let ok = scene.update_prim_shape(1, Some(ball));
    assert!(ok, "update_prim_shape should succeed");

    // Actor still exists and retains its properties.
    let actor = scene.get_actor(1).expect("actor must still exist after shape update");
    assert_eq!(actor.name(), "box");
    assert!(!actor.is_physical());
}

#[test]
fn test_update_shape_to_phantom() {
    let mut scene = scene_with_cuboid();

    // Go phantom (None shape).
    let ok = scene.update_prim_shape(1, None);
    assert!(ok, "update to phantom should succeed");

    // Actor still exists.
    let actor = scene.get_actor(1).expect("actor must survive phantom transition");
    assert_eq!(actor.name(), "box");
}

#[test]
fn test_update_shape_nonexistent_returns_false() {
    let mut scene = RapierPhysicsScene::new("test");

    let ball: Box<dyn std::any::Any + Send> = Box::new(SharedShape::ball(1.0));
    let ok = scene.update_prim_shape(999, Some(ball));
    assert!(!ok, "updating a nonexistent actor must return false");
}

#[test]
fn test_update_shape_restores_from_phantom() {
    let mut scene = scene_with_cuboid();

    // Go phantom.
    assert!(scene.update_prim_shape(1, None));

    // Restore with a cylinder shape.
    let cylinder: Box<dyn std::any::Any + Send> =
        Box::new(SharedShape::cylinder(1.0, 0.5));
    assert!(
        scene.update_prim_shape(1, Some(cylinder)),
        "restoring from phantom must succeed"
    );

    // Actor is still there and a simulation step doesn't panic.
    let actor = scene.get_actor(1).expect("actor must survive phantom round-trip");
    assert_eq!(actor.name(), "box");
    scene.simulate(1.0 / 54.5454);
}
