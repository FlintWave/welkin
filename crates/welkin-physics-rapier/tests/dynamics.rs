// SPDX-License-Identifier: LGPL-3.0-or-later
use glam::{Quat, Vec3};
use welkin_physics::PhysicsScene;
use welkin_physics_rapier::RapierPhysicsScene;

const DT: f32 = 1.0 / 54.5454;

#[test]
fn impulse_push() {
    // An impulse is applied once, changing velocity immediately, while an idle control prim stays at rest.
    let mut s = RapierPhysicsScene::new("t");
    let a = s.add_prim(1, "a", Vec3::new(0.0, 0.0, 1000.0), Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY, true);
    let b = s.add_prim(2, "b", Vec3::new(100.0, 0.0, 1000.0), Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY, true);
    let mass = s.get_actor(a).unwrap().mass();
    s.get_actor_mut(a).unwrap().add_force(Vec3::new(500.0, 0.0, 0.0), true);
    s.simulate(DT);
    let va = s.get_actor(a).unwrap().velocity();
    let vb = s.get_actor(b).unwrap().velocity();
    let expected = 500.0 / mass;
    assert!(
        (va.x - expected).abs() <= expected.abs() * 0.02 + 1.0e-4,
        "va.x={} expected={}",
        va.x,
        expected
    );
    assert!(vb.x.abs() < 1.0e-4);
}

#[test]
fn persistent_force() {
    // A persistent force increases velocity over multiple steps and stops accumulating once cleared.
    let mut s = RapierPhysicsScene::new("t");
    let a = s.add_prim(1, "a", Vec3::new(0.0, 0.0, 1000.0), Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY, true);
    s.get_actor_mut(a).unwrap().set_force(Vec3::new(100.0, 0.0, 0.0));
    s.simulate(DT);
    let v1 = s.get_actor(a).unwrap().velocity().x;
    for _ in 0..9 {
        s.simulate(DT);
    }
    let v10 = s.get_actor(a).unwrap().velocity().x;
    assert!(v10 > 0.0);
    assert!(v10 > v1);
    s.get_actor_mut(a).unwrap().set_force(Vec3::ZERO);
    let v_clear = s.get_actor(a).unwrap().velocity().x;
    for _ in 0..5 {
        s.simulate(DT);
    }
    let v_final = s.get_actor(a).unwrap().velocity().x;
    assert!(
        v_final <= v_clear * 1.01 + 1.0e-3,
        "v_final={} v_clear={}",
        v_final,
        v_clear
    );
}

#[test]
fn buoyancy_neutral() {
    // Buoyancy 1.0 cancels gravity, keeping vertical speed near zero while an unmodified prim falls.
    let mut s = RapierPhysicsScene::new("t");
    let a = s.add_prim(1, "a", Vec3::new(0.0, 0.0, 1000.0), Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY, true);
    let b = s.add_prim(2, "b", Vec3::new(100.0, 0.0, 1000.0), Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY, true);
    s.get_actor_mut(a).unwrap().set_buoyancy(1.0);
    for _ in 0..54 {
        s.simulate(DT);
    }
    let va = s.get_actor(a).unwrap().velocity();
    let vb = s.get_actor(b).unwrap().velocity();
    assert!(va.z.abs() < 0.05, "va.z={}", va.z);
    assert!(vb.z < -5.0, "vb.z={}", vb.z);
}

#[test]
fn move_to_target() {
    // The PID controller moves a prim toward its target and settles without large residual velocity.
    let mut s = RapierPhysicsScene::new("t");
    let a = s.add_prim(1, "a", Vec3::new(0.0, 0.0, 1000.0), Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY, true);
    {
        let actor = s.get_actor_mut(a).unwrap();
        actor.set_pid_target(Vec3::new(100.0, 0.0, 1000.0));
        actor.set_pid_tau(1.0);
        actor.set_pid_active(true);
    }
    for _ in 0..300 {
        s.simulate(DT);
    }
    let actor = s.get_actor(a).unwrap();
    let p = actor.position();
    let v = actor.velocity();
    assert!((p.x - 100.0).abs() < 5.0, "p.x={}", p.x);
    assert!(v.x.abs() < 5.0, "v.x={}", v.x);
    assert!((p.z - 1000.0).abs() < 5.0, "p.z={}", p.z);
}

#[test]
fn non_physical_ignores_force() {
    // A non-physical prim does not change position when force is applied.
    let mut s = RapierPhysicsScene::new("t");
    let a = s.add_prim(1, "a", Vec3::new(0.0, 0.0, 1000.0), Vec3::new(1.0, 1.0, 1.0), Quat::IDENTITY, false);
    let initial = s.get_actor(a).unwrap().position();
    s.get_actor_mut(a).unwrap().add_force(Vec3::new(1000.0, 0.0, 0.0), false);
    for _ in 0..10 {
        s.simulate(DT);
    }
    let p = s.get_actor(a).unwrap().position();
    assert!((p.x - initial.x).abs() < 1.0e-3);
    assert!((p.y - initial.y).abs() < 1.0e-3);
    assert!((p.z - initial.z).abs() < 1.0e-3);
}
