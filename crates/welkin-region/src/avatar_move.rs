// SPDX-License-Identifier: LGPL-3.0-or-later

//! Vertical placement of avatars relative to the terrain.
//!
//! Horizontal movement is integrated by the `AgentUpdate` handler; this module
//! decides where an avatar's centre sits vertically.

use glam::Vec3;

/// Height of a standing avatar's centre above the ground (metres). With the
/// default 21 m terrain this reproduces the former fixed 22.0 spawn height.
pub const AVATAR_STAND_OFFSET: f32 = 1.0;

/// Gravity applied to a non-flying avatar in the air (m/s^2).
pub const GRAVITY: f32 = 9.8;

/// Upward speed given by a jump (about 1.3 m of height).
pub const JUMP_SPEED: f32 = 5.0;

/// Falling speed cap (m/s).
pub const TERMINAL_VELOCITY: f32 = 54.0;

/// Region edge length in metres.
const REGION_SIZE: f32 = 256.0;

/// An avatar this close to the floor (and not moving up) counts as standing.
const GROUNDED_EPSILON: f32 = 0.05;

/// Place an avatar after a horizontal move and advance its vertical motion.
///
/// * x/y are kept inside the region.
/// * A walking avatar on the ground follows the terrain; `jump` gives it an upward
///   speed of `JUMP_SPEED`. In the air it is pulled down by gravity (so an avatar
///   that stops flying falls) and lands on the floor.
/// * A flying avatar may fly anywhere above the ground but not below it; its
///   vertical speed is reset because climbing is part of its horizontal velocity.
///
/// `vz` is the avatar's persistent vertical velocity.
pub fn place_on_ground(
    mut pos: Vec3,
    flying: bool,
    jump: bool,
    ground_z: f32,
    dt: f32,
    vz: &mut f32,
) -> Vec3 {
    pos.x = pos.x.clamp(0.0, REGION_SIZE);
    pos.y = pos.y.clamp(0.0, REGION_SIZE);
    let dt = dt.max(0.0);
    let floor = ground_z + AVATAR_STAND_OFFSET;

    if flying {
        *vz = 0.0;
        if pos.z < floor {
            pos.z = floor;
        }
        return pos;
    }

    let grounded = pos.z <= floor + GROUNDED_EPSILON && *vz <= 0.0;
    if grounded {
        pos.z = floor;
        *vz = if jump { JUMP_SPEED } else { 0.0 };
        if !jump {
            return pos;
        }
    }

    // Airborne (or just jumped): ballistic motion.
    *vz = (*vz - GRAVITY * dt).max(-TERMINAL_VELOCITY);
    pos.z += *vz * dt;
    if pos.z <= floor {
        pos.z = floor;
        *vz = 0.0;
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 0.02;

    fn at(z: f32) -> Vec3 {
        Vec3::new(10.0, 10.0, z)
    }

    #[test]
    fn stands_at_ground_plus_offset_not_a_fixed_height() {
        // Hilltop at 47.3 m: the avatar's centre is 1 m above it, not at 22.0.
        let mut vz = 0.0;
        let p = place_on_ground(at(22.0), false, false, 47.3, DT, &mut vz);
        assert!((p.z - 48.3).abs() < 1e-5, "z={}", p.z);
    }

    #[test]
    fn default_terrain_keeps_the_old_spawn_height() {
        let mut vz = 0.0;
        assert_eq!(place_on_ground(at(22.0), false, false, 21.0, DT, &mut vz).z, 22.0);
    }

    #[test]
    fn walking_follows_slopes_up_and_down() {
        let mut vz = 0.0;
        // Ground rises: avatar is lifted onto it.
        assert_eq!(place_on_ground(at(22.0), false, false, 23.0, DT, &mut vz).z, 24.0);
        // Ground falls slightly (within the grounded tolerance): avatar sticks to it.
        assert_eq!(place_on_ground(at(24.0), false, false, 23.98, DT, &mut vz).z, 24.98);
    }

    #[test]
    fn jump_from_the_ground_goes_up_then_comes_back_down() {
        let mut vz = 0.0;
        let mut pos = at(22.0);
        // Jump pressed once.
        pos = place_on_ground(pos, false, true, 21.0, DT, &mut vz);
        assert!(pos.z > 22.0, "should leave the ground, z={}", pos.z);
        assert!(vz > 0.0);
        // Rises, peaks around v^2 / 2g above the floor, then lands.
        let mut peak = pos.z;
        for _ in 0..200 {
            pos = place_on_ground(pos, false, false, 21.0, DT, &mut vz);
            peak = peak.max(pos.z);
        }
        let expected_peak = 22.0 + JUMP_SPEED * JUMP_SPEED / (2.0 * GRAVITY);
        assert!((peak - expected_peak).abs() < 0.1, "peak {peak}, expected ~{expected_peak}");
        assert_eq!(pos.z, 22.0, "must be back on the ground");
        assert_eq!(vz, 0.0);
    }

    #[test]
    fn cannot_jump_again_in_mid_air() {
        let mut vz = 0.0;
        let mut pos = place_on_ground(at(22.0), false, true, 21.0, DT, &mut vz);
        for _ in 0..10 {
            pos = place_on_ground(pos, false, false, 21.0, DT, &mut vz);
        }
        let (z_before, vz_before) = (pos.z, vz);
        // Holding jump in the air must not add speed.
        pos = place_on_ground(pos, false, true, 21.0, DT, &mut vz);
        assert!(vz <= vz_before, "no extra upward speed in the air");
        assert!(pos.z <= z_before + JUMP_SPEED * DT);
    }

    #[test]
    fn a_non_flying_avatar_in_the_air_falls_under_gravity_and_lands() {
        let mut vz = 0.0;
        let mut pos = at(62.0); // 40 m above the floor
        let mut last = pos.z;
        for _ in 0..1000 {
            pos = place_on_ground(pos, false, false, 21.0, DT, &mut vz);
            assert!(pos.z <= last + 1e-6, "must never rise while falling");
            last = pos.z;
            if pos.z == 22.0 {
                break;
            }
        }
        assert_eq!(pos.z, 22.0, "should have landed");
        assert_eq!(vz, 0.0);
    }

    #[test]
    fn flying_is_free_above_ground_but_cannot_go_under_it() {
        let mut vz = 3.0;
        assert_eq!(place_on_ground(at(90.0), true, false, 21.0, DT, &mut vz).z, 90.0);
        assert_eq!(vz, 0.0, "vertical speed is reset while flying");
        assert_eq!(place_on_ground(at(5.0), true, false, 21.0, DT, &mut vz).z, 22.0);
    }

    #[test]
    fn jump_has_no_effect_while_flying() {
        let mut vz = 0.0;
        assert_eq!(place_on_ground(at(40.0), true, true, 21.0, DT, &mut vz).z, 40.0);
    }

    #[test]
    fn horizontal_position_is_kept_inside_the_region() {
        let mut vz = 0.0;
        let p = place_on_ground(Vec3::new(-4.0, 300.0, 30.0), true, false, 21.0, DT, &mut vz);
        assert_eq!((p.x, p.y), (0.0, 256.0));
    }
}
