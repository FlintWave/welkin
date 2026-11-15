// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{Quat, Vec3, Vec4};

const PRIM_ROTATION_TOLERANCE: f32 = 0.002;
const PRIM_VELOCITY_TOLERANCE: f32 = 0.1;
const PRIM_ANGVELOCITY_TOLERANCE: f32 = 0.002;
const PRIM_POSITION_TOLERANCE: f32 = 0.05;
const PRIM_TIME_MS_TOLERANCE: f64 = 250.0;

const AVATAR_ROTATION_TOLERANCE: f32 = 0.01;
const AVATAR_VELOCITY_TOLERANCE: f32 = 0.1;
const AVATAR_LOW_VELOCITY_SQ: f32 = 0.1;
const AVATAR_POSITION_LARGE_TOLERANCE: f32 = 5.0;
const AVATAR_POSITION_SMALL_TOLERANCE: f32 = 0.05;

fn clamp_vector(v: Vec3, max: f32) -> Vec3 {
    let a = v.x.abs().max(v.y.abs()).max(v.z.abs());
    if a > max { v * (max / a) } else { v }
}

fn approx_equals_vec3(a: Vec3, b: Vec3, tolerance: f32) -> bool {
    (a.x - b.x).abs() <= tolerance
        && (a.y - b.y).abs() <= tolerance
        && (a.z - b.z).abs() <= tolerance
}

fn approx_zero_vec3(v: Vec3, tolerance: f32) -> bool {
    v.x.abs() <= tolerance && v.y.abs() <= tolerance && v.z.abs() <= tolerance
}

fn is_zero_vec3(v: Vec3) -> bool {
    v.x == 0.0 && v.y == 0.0 && v.z == 0.0
}

pub struct PrimUpdateState {
    pub last_position: Vec3,
    pub last_rotation: Quat,
    pub last_velocity: Vec3,
    pub last_acceleration: Vec3,
    pub last_angular_velocity: Vec3,
    pub last_update_time_ms: f64,
}

impl Default for PrimUpdateState {
    fn default() -> Self {
        Self {
            last_position: Vec3::ZERO,
            last_rotation: Quat::IDENTITY,
            last_velocity: Vec3::ZERO,
            last_acceleration: Vec3::ZERO,
            last_angular_velocity: Vec3::ZERO,
            last_update_time_ms: 0.0,
        }
    }
}

impl PrimUpdateState {
    pub fn should_send_terse(
        &self,
        now_ms: f64,
        position: Vec3,
        rotation: Quat,
        velocity: Vec3,
        acceleration: Vec3,
        angular_velocity: Vec3,
    ) -> bool {
        let elapsed = now_ms - self.last_update_time_ms;
        if elapsed > PRIM_TIME_MS_TOLERANCE {
            return true;
        }

        if !approx_equals_vec3(
            acceleration,
            self.last_acceleration,
            PRIM_VELOCITY_TOLERANCE,
        ) {
            return true;
        }

        let cur_vel = clamp_vector(velocity, 128.0);
        let last_vel = clamp_vector(self.last_velocity, 128.0);
        if !approx_equals_vec3(cur_vel, last_vel, PRIM_VELOCITY_TOLERANCE) {
            return true;
        }

        if cur_vel.x.abs() < 1e-4 && cur_vel.y.abs() < 1e-4 && cur_vel.z.abs() < 1e-4 {
            if !approx_equals_vec3(position, self.last_position, PRIM_POSITION_TOLERANCE) {
                return true;
            }
            if self.last_velocity.x.abs() > 1e-4
                || self.last_velocity.y.abs() > 1e-4
                || self.last_velocity.z.abs() > 1e-4
            {
                return true;
            }
        }

        let ang_vel = clamp_vector(angular_velocity, 64.0);
        let last_ang = clamp_vector(self.last_angular_velocity, 64.0);
        if !approx_equals_vec3(ang_vel, last_ang, PRIM_ANGVELOCITY_TOLERANCE) {
            return true;
        }

        if angular_velocity.x.abs() < 1e-4
            && angular_velocity.y.abs() < 1e-4
            && angular_velocity.z.abs() < 1e-4
        {
            let rot_diff = rotation * self.last_rotation.inverse();
            let (_, angle) = rot_diff.to_axis_angle();
            if angle.abs() > PRIM_ROTATION_TOLERANCE {
                return true;
            }
        }

        false
    }

    pub fn record_sent(
        &mut self,
        now_ms: f64,
        position: Vec3,
        rotation: Quat,
        velocity: Vec3,
        acceleration: Vec3,
        angular_velocity: Vec3,
    ) {
        self.last_position = position;
        self.last_rotation = rotation;
        self.last_velocity = velocity;
        self.last_acceleration = acceleration;
        self.last_angular_velocity = angular_velocity;
        self.last_update_time_ms = now_ms;
    }
}

pub struct AvatarUpdateState {
    pub last_position: Vec3,
    pub last_rotation: Quat,
    pub last_velocity: Vec3,
    pub last_collision_plane: Vec4,
    pub last_state: u8,
}

impl Default for AvatarUpdateState {
    fn default() -> Self {
        Self {
            last_position: Vec3::ZERO,
            last_rotation: Quat::IDENTITY,
            last_velocity: Vec3::ZERO,
            last_collision_plane: Vec4::ZERO,
            last_state: 0,
        }
    }
}

impl AvatarUpdateState {
    pub fn should_send_terse(
        &self,
        state: u8,
        position: Vec3,
        rotation: Quat,
        velocity: Vec3,
        collision_plane: Vec4,
    ) -> bool {
        if state != self.last_state {
            return true;
        }

        let rd = rotation * self.last_rotation.inverse();
        let (_, angle) = rd.to_axis_angle();
        if angle.abs() > AVATAR_ROTATION_TOLERANCE {
            return true;
        }

        if (collision_plane.x - self.last_collision_plane.x).abs() > AVATAR_POSITION_SMALL_TOLERANCE
            || (collision_plane.y - self.last_collision_plane.y).abs()
                > AVATAR_POSITION_SMALL_TOLERANCE
            || (collision_plane.w - self.last_collision_plane.w).abs()
                > AVATAR_POSITION_SMALL_TOLERANCE
        {
            return true;
        }

        if !approx_equals_vec3(velocity, self.last_velocity, AVATAR_VELOCITY_TOLERANCE) {
            return true;
        }

        if is_zero_vec3(velocity) && !is_zero_vec3(self.last_velocity) {
            return true;
        }

        let dpos = position - self.last_position;
        if !approx_zero_vec3(dpos, AVATAR_POSITION_LARGE_TOLERANCE) {
            return true;
        }

        if !approx_zero_vec3(dpos, AVATAR_POSITION_SMALL_TOLERANCE)
            && velocity.length_squared() < AVATAR_LOW_VELOCITY_SQ
        {
            return true;
        }

        false
    }

    pub fn record_sent(
        &mut self,
        state: u8,
        position: Vec3,
        rotation: Quat,
        velocity: Vec3,
        collision_plane: Vec4,
    ) {
        self.last_state = state;
        self.last_position = position;
        self.last_rotation = rotation;
        self.last_velocity = velocity;
        self.last_collision_plane = collision_plane;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prim_elapsed_over_tolerance_sends() {
        let state = PrimUpdateState::default();
        assert!(state.should_send_terse(
            300.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        ));
    }

    #[test]
    fn prim_no_change_suppresses() {
        let mut state = PrimUpdateState::default();
        state.record_sent(
            100.0,
            Vec3::new(10.0, 20.0, 30.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        assert!(!state.should_send_terse(
            200.0,
            Vec3::new(10.0, 20.0, 30.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        ));
    }

    #[test]
    fn prim_velocity_change_sends() {
        let mut state = PrimUpdateState::default();
        state.record_sent(
            100.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
        );
        assert!(state.should_send_terse(
            200.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::new(1.5, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
        ));
    }

    #[test]
    fn prim_velocity_just_stopped_sends() {
        let mut state = PrimUpdateState::default();
        state.record_sent(
            100.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
        );
        assert!(state.should_send_terse(
            200.0,
            Vec3::new(0.05, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        ));
    }

    #[test]
    fn prim_position_drift_at_rest_sends() {
        let mut state = PrimUpdateState::default();
        state.record_sent(
            100.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        assert!(state.should_send_terse(
            200.0,
            Vec3::new(0.1, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        ));
    }

    #[test]
    fn prim_rotation_at_rest_sends() {
        let mut state = PrimUpdateState::default();
        state.record_sent(
            100.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        );
        let rot = Quat::from_axis_angle(Vec3::Z, 0.1);
        assert!(state.should_send_terse(
            200.0,
            Vec3::ZERO,
            rot,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        ));
    }

    #[test]
    fn prim_angular_velocity_change_sends() {
        let mut state = PrimUpdateState::default();
        state.record_sent(
            100.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
        );
        assert!(state.should_send_terse(
            200.0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::new(1.5, 0.0, 0.0),
        ));
    }

    #[test]
    fn prim_clamp_vector_works() {
        let v = Vec3::new(200.0, -100.0, 50.0);
        let clamped = clamp_vector(v, 128.0);
        assert!(clamped.x.abs() <= 128.01);
        assert!(clamped.y.abs() <= 128.01);
        assert!(clamped.z.abs() <= 128.01);
        let ratio = clamped.x / clamped.y;
        let original_ratio = v.x / v.y;
        assert!((ratio - original_ratio).abs() < 0.001);
    }

    #[test]
    fn avatar_state_change_sends() {
        let mut state = AvatarUpdateState::default();
        state.record_sent(0, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec4::ZERO);
        assert!(state.should_send_terse(1, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec4::ZERO,));
    }

    #[test]
    fn avatar_no_change_suppresses() {
        let mut state = AvatarUpdateState::default();
        state.record_sent(0, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec4::ZERO);
        assert!(!state.should_send_terse(0, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec4::ZERO,));
    }

    #[test]
    fn avatar_rotation_change_sends() {
        let mut state = AvatarUpdateState::default();
        state.record_sent(0, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec4::ZERO);
        let rot = Quat::from_axis_angle(Vec3::Z, 0.5);
        assert!(state.should_send_terse(0, Vec3::ZERO, rot, Vec3::ZERO, Vec4::ZERO));
    }

    #[test]
    fn avatar_velocity_stop_sends() {
        let mut state = AvatarUpdateState::default();
        state.record_sent(
            0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::new(1.0, 0.0, 0.0),
            Vec4::ZERO,
        );
        assert!(state.should_send_terse(
            0,
            Vec3::new(1.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec4::ZERO,
        ));
    }

    #[test]
    fn avatar_large_position_change_sends() {
        let mut state = AvatarUpdateState::default();
        state.record_sent(0, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec4::ZERO);
        assert!(state.should_send_terse(
            0,
            Vec3::new(10.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec4::ZERO,
        ));
    }

    #[test]
    fn avatar_small_drift_at_low_velocity_sends() {
        let mut state = AvatarUpdateState::default();
        state.record_sent(0, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec4::ZERO);
        assert!(state.should_send_terse(
            0,
            Vec3::new(0.1, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::new(0.1, 0.0, 0.0),
            Vec4::ZERO,
        ));
    }

    #[test]
    fn avatar_collision_plane_change_sends() {
        let mut state = AvatarUpdateState::default();
        state.record_sent(0, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Vec4::ZERO);
        assert!(state.should_send_terse(
            0,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec4::new(0.0, 0.0, 1.0, 1.0),
        ));
    }
}
