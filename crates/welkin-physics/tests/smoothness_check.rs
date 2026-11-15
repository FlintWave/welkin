// SPDX-License-Identifier: LGPL-3.0-or-later
//
// Aurora Viewer smoothness check: verify that physics outputs produce
// smooth motion suitable for viewer interpolation (no position snapping).

use glam::{Quat, Vec3, Vec4};

use welkin_physics::avatar::AvatarController;
use welkin_physics::terse_update::{AvatarUpdateState, PrimUpdateState};
use welkin_physics::vehicle::{VehicleState, VehicleType};

const DT: f32 = 1.0 / 45.0;
const GRAVITY_Z: f32 = -9.8;
const STEPS: usize = 200;

// Avatar PID controllers apply strong corrective forces initially.
// The viewer handles gaps via interpolation. Thresholds here verify
// that per-update jumps stay within what viewer interpolation can mask.

#[test]
fn walking_avatar_smooth() {
    let mut ctrl = AvatarController::new(80.0);
    ctrl.target_velocity = Vec3::new(3.0, 0.0, 0.0);
    ctrl.colliding = true;
    ctrl.colliding_ground = true;

    let mut pos = Vec3::new(128.0, 128.0, 25.0);
    let mut vel = Vec3::ZERO;
    let mut update_state = AvatarUpdateState::default();
    let mut last_sent_pos = pos;
    let mut max_jump: f32 = 0.0;

    for _ in 0..STEPS {
        let force = ctrl.step(pos, vel, 1.0, 25.0, 20.0, GRAVITY_Z * 80.0, DT);
        vel += force / 80.0 * DT;
        pos += vel * DT;

        if update_state.should_send_terse(0, pos, Quat::IDENTITY, vel, Vec4::ZERO) {
            let jump = (pos - last_sent_pos).length();
            if jump > max_jump {
                max_jump = jump;
            }
            last_sent_pos = pos;
            update_state.record_sent(0, pos, Quat::IDENTITY, vel, Vec4::ZERO);
        }
    }
    assert!(
        max_jump < 10.0,
        "walking avatar max jump {max_jump} exceeds 10m snap threshold"
    );
}

#[test]
fn flying_avatar_smooth() {
    let mut ctrl = AvatarController::new(80.0);
    ctrl.target_velocity = Vec3::new(5.0, 0.0, 2.0);
    ctrl.flying = true;

    let mut pos = Vec3::new(128.0, 128.0, 50.0);
    let mut vel = Vec3::ZERO;
    let mut update_state = AvatarUpdateState::default();
    let mut last_sent_pos = pos;
    let mut max_jump: f32 = 0.0;

    for _ in 0..STEPS {
        let force = ctrl.step(pos, vel, 1.0, 25.0, 20.0, GRAVITY_Z * 80.0, DT);
        vel += force / 80.0 * DT;
        pos += vel * DT;

        if update_state.should_send_terse(0, pos, Quat::IDENTITY, vel, Vec4::ZERO) {
            let jump = (pos - last_sent_pos).length();
            if jump > max_jump {
                max_jump = jump;
            }
            last_sent_pos = pos;
            update_state.record_sent(0, pos, Quat::IDENTITY, vel, Vec4::ZERO);
        }
    }

    assert!(
        max_jump < 10.0,
        "flying avatar max jump {max_jump} exceeds 10m snap threshold"
    );
}

#[test]
fn vehicle_car_smooth() {
    let mut vs = VehicleState::default();
    vs.set_type(VehicleType::Car, 1.0 / DT);
    vs.linear_motor_direction = Vec3::new(10.0, 0.0, 0.0);

    let mut pos = Vec3::new(128.0, 128.0, 25.0);
    let mut vel = Vec3::ZERO;
    let mut angvel = Vec3::ZERO;
    let rot = Quat::IDENTITY;
    let mut prim_state = PrimUpdateState::default();
    let mut time_ms: f64 = 0.0;
    let mut last_sent_pos = pos;
    let mut max_jump: f32 = 0.0;

    for _ in 0..STEPS {
        let (force, torque) = vs.step(pos, vel, angvel, rot, 100.0, 25.0, 20.0, GRAVITY_Z, DT);
        vel += force / 100.0 * DT;
        angvel += torque / 100.0 * DT;
        pos += vel * DT;
        time_ms += DT as f64 * 1000.0;

        if prim_state.should_send_terse(time_ms, pos, rot, vel, Vec3::ZERO, angvel) {
            let jump = (pos - last_sent_pos).length();
            if jump > max_jump {
                max_jump = jump;
            }
            last_sent_pos = pos;
            prim_state.record_sent(time_ms, pos, rot, vel, Vec3::ZERO, angvel);
        }
    }

    assert!(
        max_jump < 5.0,
        "vehicle car max jump {max_jump} exceeds 5m snap threshold"
    );
}

#[test]
fn falling_object_smooth() {
    let mut pos = Vec3::new(128.0, 128.0, 100.0);
    let mut vel = Vec3::ZERO;
    let rot = Quat::IDENTITY;
    let mut prim_state = PrimUpdateState::default();
    let mut time_ms: f64 = 0.0;
    let mut last_sent_pos = pos;
    let mut max_jump: f32 = 0.0;

    for _ in 0..STEPS {
        vel.z += GRAVITY_Z * DT;
        pos += vel * DT;
        time_ms += DT as f64 * 1000.0;

        if prim_state.should_send_terse(time_ms, pos, rot, vel, Vec3::ZERO, Vec3::ZERO) {
            let jump = (pos - last_sent_pos).length();
            if jump > max_jump {
                max_jump = jump;
            }
            last_sent_pos = pos;
            prim_state.record_sent(time_ms, pos, rot, vel, Vec3::ZERO, Vec3::ZERO);
        }
    }

    assert!(
        max_jump < 3.0,
        "falling object max jump {max_jump} exceeds 3m snap threshold"
    );
}

#[test]
fn terse_updates_not_starved() {
    let mut pos = Vec3::new(128.0, 128.0, 25.0);
    let vel = Vec3::new(3.0, 0.0, 0.0);
    let rot = Quat::IDENTITY;
    let mut prim_state = PrimUpdateState::default();
    let mut time_ms: f64 = 0.0;
    let mut update_count = 0;

    for _ in 0..STEPS {
        pos += vel * DT;
        time_ms += DT as f64 * 1000.0;

        if prim_state.should_send_terse(time_ms, pos, rot, vel, Vec3::ZERO, Vec3::ZERO) {
            update_count += 1;
            prim_state.record_sent(time_ms, pos, rot, vel, Vec3::ZERO, Vec3::ZERO);
        }
    }

    assert!(
        update_count >= 3,
        "only {update_count} terse updates in {STEPS} steps — viewer would stutter"
    );
}
