// SPDX-License-Identifier: GPL-3.0-or-later

use glam::{Quat, Vec3};
use rapier3d::parry::shape::SharedShape;

use welkin_physics::PhysicsScene;
use welkin_physics_rapier::RapierPhysicsScene;

#[test]
fn add_prim_with_none_falls_back_to_cuboid() {
    let mut scene = RapierPhysicsScene::new("test");
    let id = scene.add_prim_with_shape(
        1,
        "box",
        Vec3::new(128.0, 128.0, 25.0),
        Vec3::new(1.0, 1.0, 1.0),
        Quat::IDENTITY,
        false,
        None,
    );
    assert_eq!(id, 1);
    let actor = scene.get_actor(1).expect("actor must exist");
    assert_eq!(actor.name(), "box");
    assert!(!actor.is_physical());
}

#[test]
fn add_prim_with_custom_shape() {
    let mut scene = RapierPhysicsScene::new("test");
    let ball: Box<dyn std::any::Any + Send> = Box::new(SharedShape::ball(1.0));
    let id = scene.add_prim_with_shape(
        42,
        "sphere",
        Vec3::new(10.0, 20.0, 30.0),
        Vec3::new(2.0, 2.0, 2.0),
        Quat::IDENTITY,
        true,
        Some(ball),
    );
    assert_eq!(id, 42);
    let actor = scene.get_actor(42).expect("actor must exist");
    assert_eq!(actor.name(), "sphere");
    assert!(actor.is_physical());
}

#[test]
fn add_prim_with_wrong_type_falls_back_to_cuboid() {
    // Passing a type that is not SharedShape should gracefully fall back to cuboid.
    let mut scene = RapierPhysicsScene::new("test");
    let wrong: Box<dyn std::any::Any + Send> = Box::new(42_u64);
    let id = scene.add_prim_with_shape(
        7,
        "wrong_type",
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 1.0, 1.0),
        Quat::IDENTITY,
        false,
        Some(wrong),
    );
    assert_eq!(id, 7);
    let actor = scene
        .get_actor(7)
        .expect("actor must exist after bad downcast");
    assert_eq!(actor.name(), "wrong_type");
}

/// Verify the default trait implementation delegates to add_prim.
#[test]
fn default_trait_impl_delegates_to_add_prim() {
    use welkin_physics::null::NullPhysicsScene;

    let mut scene = NullPhysicsScene::new("null-test");
    let ball: Box<dyn std::any::Any + Send> = Box::new(SharedShape::ball(0.5));
    let id = scene.add_prim_with_shape(
        99,
        "null_prim",
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::new(0.5, 0.5, 0.5),
        Quat::IDENTITY,
        false,
        Some(ball),
    );
    assert_eq!(id, 99);
    let actor = scene
        .get_actor(99)
        .expect("NullPhysicsScene must create actor via default impl");
    assert_eq!(actor.name(), "null_prim");
}
