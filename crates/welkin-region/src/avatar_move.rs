// SPDX-License-Identifier: LGPL-3.0-or-later

//! Vertical placement of avatars relative to the terrain.
//!
//! Horizontal movement is integrated by the `AgentUpdate` handler; this module
//! decides where an avatar's centre sits vertically.

use glam::Vec3;

/// Height of a standing avatar's centre above the ground (metres). With the
/// default 21 m terrain this reproduces the former fixed 22.0 spawn height.
pub const AVATAR_STAND_OFFSET: f32 = 1.0;

/// Speed at which a non-flying avatar above the ground descends (metres/second).
pub const FALL_SPEED: f32 = 20.0;

/// Region edge length in metres.
const REGION_SIZE: f32 = 256.0;

/// Place an avatar after a horizontal move.
///
/// * x/y are kept inside the region.
/// * A walking avatar stays on the ground (follows slopes up and down); if it was
///   above the ground (for example after it stopped flying) it falls at `FALL_SPEED`.
/// * A flying avatar may fly anywhere above the ground but not below it.
pub fn place_on_ground(mut pos: Vec3, flying: bool, ground_z: f32, dt: f32) -> Vec3 {
    pos.x = pos.x.clamp(0.0, REGION_SIZE);
    pos.y = pos.y.clamp(0.0, REGION_SIZE);

    let floor = ground_z + AVATAR_STAND_OFFSET;
    if flying {
        if pos.z < floor {
            pos.z = floor;
        }
    } else if pos.z > floor {
        pos.z = (pos.z - FALL_SPEED * dt.max(0.0)).max(floor);
    } else {
        pos.z = floor;
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stands_at_ground_plus_offset_not_a_fixed_height() {
        // Hilltop at 47.3 m: the avatar's centre is 1 m above it, not at 22.0.
        let p = place_on_ground(Vec3::new(10.0, 10.0, 22.0), false, 47.3, 0.02);
        assert!((p.z - 48.3).abs() < 1e-5, "z={}", p.z);
    }

    #[test]
    fn default_terrain_keeps_the_old_spawn_height() {
        let p = place_on_ground(Vec3::new(128.0, 128.0, 22.0), false, 21.0, 0.02);
        assert_eq!(p.z, 22.0);
    }

    #[test]
    fn walking_follows_a_downhill_slope_immediately() {
        // Ground drops from 30 to 28: a walking avatar that was ~1 m up (at the
        // old floor) is within fall range and settles without floating.
        let p = place_on_ground(Vec3::new(10.0, 10.0, 31.0), false, 28.0, 0.5);
        assert!((p.z - 29.0).abs() < 1e-5, "z={}", p.z);
    }

    #[test]
    fn a_non_flying_avatar_above_the_ground_falls_at_fall_speed() {
        // 40 m above the floor, dt = 0.1 s -> drops 2 m.
        let p = place_on_ground(Vec3::new(10.0, 10.0, 62.0), false, 21.0, 0.1);
        assert!((p.z - 60.0).abs() < 1e-4, "z={}", p.z);
        // Never falls through the floor.
        let p = place_on_ground(Vec3::new(10.0, 10.0, 22.5), false, 21.0, 10.0);
        assert_eq!(p.z, 22.0);
    }

    #[test]
    fn flying_is_free_above_ground_but_cannot_go_under_it() {
        assert_eq!(place_on_ground(Vec3::new(10.0, 10.0, 90.0), true, 21.0, 0.02).z, 90.0);
        assert_eq!(place_on_ground(Vec3::new(10.0, 10.0, 5.0), true, 21.0, 0.02).z, 22.0);
    }

    #[test]
    fn horizontal_position_is_kept_inside_the_region() {
        let p = place_on_ground(Vec3::new(-4.0, 300.0, 30.0), true, 21.0, 0.02);
        assert_eq!((p.x, p.y), (0.0, 256.0));
    }
}
