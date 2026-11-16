// SPDX-License-Identifier: LGPL-3.0-or-later
//! The terrain collider must cover the whole 256 m region with the right axes.
//!
//! History: it was once a 1 m x 1 m field (everything fell through the ground),
//! and after that the height map was transposed and rotated 180 degrees because
//! parry's `Array2` is column-major.

use glam::{Quat, Vec3};
use welkin_physics::{PhysicsScene, RayFilterFlags};
use welkin_physics_rapier::RapierPhysicsScene;

const SIDE: usize = 256;

/// Build a scene whose height at (x, y) is `height_at(x, y)` (map index = y * 256 + x).
fn scene_with(height_at: impl Fn(usize, usize) -> f32) -> RapierPhysicsScene {
    let mut heights = Vec::with_capacity(SIDE * SIDE);
    for y in 0..SIDE {
        for x in 0..SIDE {
            heights.push(height_at(x, y));
        }
    }
    let mut s = RapierPhysicsScene::new("terrain");
    s.set_terrain(&heights);
    s.simulate(0.02); // update the broad phase so raycasts can see the terrain
    s
}

/// Terrain height straight below (x, y), read back through a raycast.
fn ground(s: &RapierPhysicsScene, x: f32, y: f32) -> f32 {
    s.raycast(
        Vec3::new(x, y, 500.0),
        Vec3::new(0.0, 0.0, -1.0),
        1000.0,
        1,
        RayFilterFlags(0xFFFF),
    )
    .first()
    .unwrap_or_else(|| panic!("no terrain under ({x},{y})"))
    .pos
    .z
}

#[test]
fn height_x_slope_maps_to_world_x() {
    let s = scene_with(|x, _| x as f32 * 0.1);
    // Sample inside cells (not on grid vertices, where a ray can slip between triangles).
    for (x, y) in [(10.3, 10.6), (200.3, 10.6), (10.3, 200.6), (200.3, 200.6), (128.3, 77.6)] {
        let g = ground(&s, x, y);
        assert!((g - x * 0.1).abs() < 0.05, "at ({x},{y}) ground={g}, want {}", x * 0.1);
    }
}

#[test]
fn height_y_slope_maps_to_world_y() {
    let s = scene_with(|_, y| y as f32 * 0.1);
    for (x, y) in [(10.3, 10.6), (200.3, 10.6), (10.3, 200.6), (200.3, 200.6), (77.3, 128.6)] {
        let g = ground(&s, x, y);
        assert!((g - y * 0.1).abs() < 0.05, "at ({x},{y}) ground={g}, want {}", y * 0.1);
    }
}

#[test]
fn a_single_raised_cell_lands_at_its_own_coordinates() {
    // A plateau around (x=77, y=33) only; everything else at 5 m.
    let s = scene_with(|x, y| {
        if (75..=79).contains(&x) && (31..=35).contains(&y) { 50.0 } else { 5.0 }
    });
    assert!((ground(&s, 77.3, 33.6) - 50.0).abs() < 0.05, "plateau missing at its own cell");
    assert!((ground(&s, 33.3, 77.6) - 5.0).abs() < 0.05, "transposed cell must be flat");
    assert!((ground(&s, 178.3, 222.6) - 5.0).abs() < 0.05, "mirrored cell must be flat");
}

#[test]
fn flat_terrain_holds_a_prim_everywhere_in_the_region() {
    // Flat ground at 21 m: a 1 m cube must come to rest with its centre ~0.5 m above it.
    for (x, y) in [(128.0, 128.0), (2.0, 2.0), (250.0, 250.0), (2.0, 250.0), (250.0, 2.0)] {
        let mut s = scene_with(|_, _| 21.0);
        s.add_prim(1, "cube", Vec3::new(x, y, 80.0), Vec3::ONE, Quat::IDENTITY, true);
        for _ in 0..800 {
            s.simulate(1.0 / 54.5454);
        }
        let z = s.get_actor(1).unwrap().position().z;
        assert!((z - 21.5).abs() < 0.1, "at ({x},{y}) the cube rests at z={z}, expected ~21.5");
    }
}
