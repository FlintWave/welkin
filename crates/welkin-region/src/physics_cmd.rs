// SPDX-License-Identifier: LGPL-3.0-or-later

//! Apply script `PhysicsCommand`s (llApplyImpulse, llSetForce, llSetBuoyancy,
//! llMoveToTarget, llSetStatus, ...) to the physics actor of the owning object.

use glam::Vec3;
use uuid::Uuid;
use welkin_scene::scene::Scene;
use welkin_script_vm::PhysicsCommand;

// LSL STATUS_* constants that reach the physics layer.
const STATUS_PHYSICS: i32 = 1;
const STATUS_PHANTOM: i32 = 16;

fn vec3(v: [f64; 3]) -> Vec3 {
    Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32)
}

/// Apply one command sent by the script running in `prim_id` (root or child prim).
/// Returns `Err` with a reason for commands the physics layer cannot honour, so the
/// caller can log them instead of dropping them silently.
pub fn apply_physics_command(
    scene: &Scene,
    prim_id: &Uuid,
    cmd: &PhysicsCommand,
) -> Result<(), String> {
    let group_id = scene
        .resolve_group_for_prim(prim_id)
        .ok_or_else(|| "unknown prim".to_string())?;
    let (local_id, rotation) = scene
        .with_object(&group_id, |g| {
            let r = g.root_part();
            (r.local_id, r.rotation_offset)
        })
        .ok_or_else(|| "object vanished".to_string())?;

    // Vectors flagged `local` are in the object's frame.
    let world = |v: [f64; 3], local: bool| {
        let v = vec3(v);
        if local { rotation * v } else { v }
    };

    scene
        .with_physics(|ph| {
            let Some(actor) = ph.get_actor_mut(local_id) else {
                return Err("object has no physics actor".to_string());
            };
            match cmd {
                PhysicsCommand::SetForce { force, local } => actor.set_force(world(*force, *local)),
                PhysicsCommand::SetTorque { torque, local } => actor.set_torque(world(*torque, *local)),
                PhysicsCommand::ApplyImpulse { impulse, local } => {
                    actor.add_force(world(*impulse, *local), true)
                }
                PhysicsCommand::ApplyRotationalImpulse { impulse, local } => {
                    actor.add_angular_force(world(*impulse, *local), true)
                }
                PhysicsCommand::SetBuoyancy { buoyancy } => actor.set_buoyancy(*buoyancy as f32),
                PhysicsCommand::MoveToTarget { target, tau } => {
                    actor.set_pid_target(vec3(*target));
                    actor.set_pid_tau(*tau as f32);
                    actor.set_pid_active(true);
                }
                PhysicsCommand::StopMoveToTarget => actor.set_pid_active(false),
                PhysicsCommand::SetStatus { status, value } => match *status {
                    STATUS_PHANTOM => actor.set_phantom(*value),
                    STATUS_PHYSICS => {
                        return Err(
                            "changing STATUS_PHYSICS at runtime is not supported by the physics backend yet"
                                .to_string(),
                        );
                    }
                    other => return Err(format!("STATUS flag {other} is not handled by physics")),
                },
                PhysicsCommand::SetHoverHeight { .. } => {
                    return Err("llSetHoverHeight is not supported by the physics backend yet".to_string());
                }
            }
            Ok(())
        })
        .ok_or_else(|| "no physics backend".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;
    use std::sync::mpsc;
    use welkin_scene::group::SceneObjectGroup;
    use welkin_scene::part::{PrimFlags, SceneObjectPart};
    use welkin_scene::scene::{RegionInfo, SceneConfig};

    fn world() -> (Scene, Uuid, u32) {
        let scene = Scene::new(RegionInfo::new("T", 1000, 1000), SceneConfig::default());
        scene.set_physics(Box::new(welkin_physics_rapier::RapierPhysicsScene::new("T")));
        let mut part = SceneObjectPart::new("Body", Uuid::new_v4());
        part.group_position = Vec3::new(100.0, 100.0, 1000.0); // far above any ground
        part.flags |= PrimFlags::PHYSICS;
        let id = scene.add_object(SceneObjectGroup::new(part));
        let local_id = scene.get_object(&id).unwrap().root_part().local_id;
        (scene, id, local_id)
    }

    fn velocity(scene: &Scene, local_id: u32) -> Vec3 {
        scene.with_physics(|p| p.get_actor(local_id).unwrap().velocity()).unwrap()
    }

    fn mass(scene: &Scene, local_id: u32) -> f32 {
        scene.with_physics(|p| p.get_actor(local_id).unwrap().mass()).unwrap()
    }

    /// The path scripts use: command goes through the std mpsc channel the
    /// region drains, then is applied to the physics body.
    fn send_and_apply(scene: &Scene, prim: Uuid, cmd: PhysicsCommand) -> Result<(), String> {
        let (tx, rx) = mpsc::channel::<(Uuid, PhysicsCommand)>();
        tx.send((prim, cmd)).unwrap();
        let (p, c) = rx.try_recv().unwrap();
        apply_physics_command(scene, &p, &c)
    }

    #[test]
    fn apply_impulse_through_the_channel_changes_the_bodys_velocity() {
        let (scene, id, lid) = world();
        let m = mass(&scene, lid);
        send_and_apply(&scene, id, PhysicsCommand::ApplyImpulse { impulse: [500.0, 0.0, 0.0], local: false })
            .unwrap();
        scene.tick();
        let v = velocity(&scene, lid);
        assert!((v.x - 500.0 / m).abs() < 0.02 * (500.0 / m), "vx={} want {}", v.x, 500.0 / m);
    }

    #[test]
    fn local_impulse_follows_the_objects_rotation() {
        let (scene, id, lid) = world();
        // Turn the object 90 degrees about Z so local +X points along world +Y.
        scene.with_object_mut(&id, |g| g.set_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)));
        scene.push_object_to_physics(&id);
        let m = mass(&scene, lid);
        send_and_apply(&scene, id, PhysicsCommand::ApplyImpulse { impulse: [400.0, 0.0, 0.0], local: true })
            .unwrap();
        scene.tick();
        let v = velocity(&scene, lid);
        assert!(v.x.abs() < 0.05 * (400.0 / m), "world x should be ~0, got {}", v.x);
        assert!((v.y - 400.0 / m).abs() < 0.02 * (400.0 / m), "vy={} want {}", v.y, 400.0 / m);
    }

    #[test]
    fn set_force_and_buoyancy_act_over_time() {
        let (scene, id, lid) = world();
        // Neutral buoyancy: no fall.
        send_and_apply(&scene, id, PhysicsCommand::SetBuoyancy { buoyancy: 1.0 }).unwrap();
        // A continuous force along +X.
        send_and_apply(&scene, id, PhysicsCommand::SetForce { force: [200.0, 0.0, 0.0], local: false }).unwrap();
        for _ in 0..30 {
            scene.tick();
        }
        let v = velocity(&scene, lid);
        assert!(v.x > 0.1, "force should have accelerated the body, vx={}", v.x);
        assert!(v.z.abs() < 0.2, "buoyancy 1.0 should cancel gravity, vz={}", v.z);
    }

    #[test]
    fn move_to_target_pulls_the_body_and_stop_releases_it() {
        let (scene, id, lid) = world();
        send_and_apply(
            &scene,
            id,
            PhysicsCommand::MoveToTarget { target: [100.0, 140.0, 1000.0], tau: 1.0 },
        )
        .unwrap();
        for _ in 0..40 {
            scene.tick();
        }
        assert!(velocity(&scene, lid).y > 1.0, "should be heading to +y");
        send_and_apply(&scene, id, PhysicsCommand::StopMoveToTarget).unwrap();
    }

    #[test]
    fn unsupported_commands_report_a_reason_instead_of_being_dropped() {
        let (scene, id, _) = world();
        let err = send_and_apply(&scene, id, PhysicsCommand::SetStatus { status: 1, value: false }).unwrap_err();
        assert!(err.contains("STATUS_PHYSICS"), "{err}");
        let err = send_and_apply(&scene, id, PhysicsCommand::SetHoverHeight { height: 5.0, water: 0, tau: 1.0 })
            .unwrap_err();
        assert!(err.contains("llSetHoverHeight"), "{err}");
        let err = send_and_apply(&scene, Uuid::new_v4(), PhysicsCommand::StopMoveToTarget).unwrap_err();
        assert!(err.contains("unknown prim"), "{err}");
    }

    /// Whole chain without a viewer: LSL source -> compiler -> Luau VM -> physics
    /// channel -> command applied -> Rapier body velocity.
    #[test]
    fn lsl_apply_impulse_moves_a_physical_prim_end_to_end() {
        let (scene, id, lid) = world();
        let m = mass(&scene, lid);

        let src = r#"default { state_entry() { llApplyImpulse(<500.0, 0.0, 0.0>, FALSE); } }"#;
        let luau = welkin_script_vm::compile_lsl(src).expect("LSL compile");
        let bytecode = welkin_script_vm::compile_source(&luau).expect("luau compile");
        let lua = welkin_script_vm::new_vm().unwrap();
        let (tx, rx) = mpsc::channel::<(Uuid, PhysicsCommand)>();
        let ctx = std::sync::Arc::new(parking_lot::RwLock::new(
            welkin_script_vm::builtins::ScriptContext {
                physics_sender: Some(tx),
                prim_id: id,
                ..Default::default()
            },
        ));
        welkin_script_vm::builtins::register_all(&lua, &ctx).unwrap();
        lua.load(&bytecode).exec().unwrap();
        lua.load(r#"__dispatch("state_entry")"#).exec().unwrap();

        while let Ok((prim, cmd)) = rx.try_recv() {
            apply_physics_command(&scene, &prim, &cmd).unwrap();
        }
        scene.tick();
        let v = velocity(&scene, lid);
        assert!((v.x - 500.0 / m).abs() < 0.02 * (500.0 / m), "vx={} want {}", v.x, 500.0 / m);
    }
}
