// SPDX-License-Identifier: GPL-3.0-or-later

//! Proof tests for prim meshing colliders (tasks 6.1, 6.2, 6.3).
//!
//! 6.1 -- Collider shape verification: reference prims produce shapes with
//!         expected geometric properties (AABB half-extents, vertex counts).
//! 6.2 -- Drop test: objects with real colliders land correctly on terrain
//!         and on other objects.
//! 6.3 -- Performance: meshing 1 000 mixed prims within 5 seconds.

use std::time::Instant;

use glam::{Quat, Vec3};
use uuid::Uuid;

use welkin_prim::meshing::{PhysicsShapeType, ShapeCache, create_physics_shape, shape_for_box};
use welkin_prim::{
    LL_PCODE_PATH_CIRCLE, LL_PCODE_PROFILE_CIRCLE, LL_PCODE_PROFILE_SQUARE, LL_SCULPT_TYPE_SPHERE,
    PathParams, ProfileParams, SculptParams, VolumeParams,
};

use welkin_physics::PhysicsScene;
use welkin_physics_rapier::RapierPhysicsScene;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn box_params() -> VolumeParams {
    VolumeParams {
        profile: ProfileParams {
            curve_type: LL_PCODE_PROFILE_SQUARE,
            begin: 0.0,
            end: 1.0,
            hollow: 0.0,
        },
        path: PathParams::default(),
        sculpt: None,
    }
}

fn sphere_params() -> VolumeParams {
    VolumeParams {
        profile: ProfileParams {
            curve_type: LL_PCODE_PROFILE_CIRCLE,
            begin: 0.0,
            end: 1.0,
            hollow: 0.0,
        },
        path: PathParams::default(),
        sculpt: None,
    }
}

/// Circle profile + circle path -- ring-like shape that forces meshing
/// (neither analytic box nor analytic sphere).
fn cylinder_params() -> VolumeParams {
    VolumeParams {
        profile: ProfileParams {
            curve_type: LL_PCODE_PROFILE_CIRCLE,
            begin: 0.0,
            end: 1.0,
            hollow: 0.0,
        },
        path: PathParams {
            curve_type: LL_PCODE_PATH_CIRCLE,
            ..PathParams::default()
        },
        sculpt: None,
    }
}

/// Circle profile + circle path with reduced cross-section scale -- torus.
fn torus_params() -> VolumeParams {
    VolumeParams {
        profile: ProfileParams {
            curve_type: LL_PCODE_PROFILE_CIRCLE,
            begin: 0.0,
            end: 1.0,
            hollow: 0.0,
        },
        path: PathParams {
            curve_type: LL_PCODE_PATH_CIRCLE,
            scale: [0.5, 0.5],
            ..PathParams::default()
        },
        sculpt: None,
    }
}

fn sculpt_params_sphere() -> VolumeParams {
    VolumeParams {
        profile: ProfileParams::default(),
        path: PathParams::default(),
        sculpt: Some(SculptParams {
            texture: Uuid::from_u128(0xdead_beef),
            sculpt_type: LL_SCULPT_TYPE_SPHERE,
        }),
    }
}

/// Approximate equality with absolute tolerance.
fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() < tol
}

/// Flat square heightmap at constant `height`.
fn flat_terrain(side: usize, height: f32) -> Vec<f32> {
    vec![height; side * side]
}

// ===========================================================================
// 6.1 -- Collider shape verification
// ===========================================================================

/// Box(2x4x6) should produce an analytic cuboid whose AABB half-extents are
/// approximately (1, 2, 3).
#[test]
fn test_box_collider_aabb() {
    let params = box_params();
    let size = Vec3::new(2.0, 4.0, 6.0);
    let cache = ShapeCache::new();

    let result = create_physics_shape(&params, size, PhysicsShapeType::Prim, &cache, 0)
        .expect("box should produce a shape");

    // Analytic box has zero vertex/triangle counts.
    assert_eq!(
        result.vertex_count, 0,
        "analytic box should have zero vertices"
    );

    let aabb = result.shape.compute_local_aabb();
    let hx = (aabb.maxs.x - aabb.mins.x) * 0.5;
    let hy = (aabb.maxs.y - aabb.mins.y) * 0.5;
    let hz = (aabb.maxs.z - aabb.mins.z) * 0.5;

    assert!(
        approx_eq(hx, 1.0, 0.01),
        "x half-extent: expected ~1.0, got {}",
        hx
    );
    assert!(
        approx_eq(hy, 2.0, 0.01),
        "y half-extent: expected ~2.0, got {}",
        hy
    );
    assert!(
        approx_eq(hz, 3.0, 0.01),
        "z half-extent: expected ~3.0, got {}",
        hz
    );
}

/// Sphere(d=5) should produce an analytic ball whose AABB is approximately
/// cubic with half-extent ~2.5 on every axis.
#[test]
fn test_sphere_collider_radius() {
    let params = sphere_params();
    let diameter = 5.0_f32;
    let size = Vec3::splat(diameter);
    let cache = ShapeCache::new();

    let result = create_physics_shape(&params, size, PhysicsShapeType::Prim, &cache, 0)
        .expect("sphere should produce a shape");

    assert_eq!(
        result.vertex_count, 0,
        "analytic sphere should have zero vertices"
    );

    let aabb = result.shape.compute_local_aabb();
    let expected = diameter * 0.5; // 2.5

    for (axis, lo, hi) in [
        ("x", aabb.mins.x, aabb.maxs.x),
        ("y", aabb.mins.y, aabb.maxs.y),
        ("z", aabb.mins.z, aabb.maxs.z),
    ] {
        let half = (hi - lo) * 0.5;
        assert!(
            approx_eq(half, expected, 0.05),
            "{} half-extent: expected ~{}, got {}",
            axis,
            expected,
            half
        );
    }
}

/// Cylinder-like params (circle profile, circle path -- NOT analytic) should
/// produce a trimesh with real geometry.
#[test]
fn test_cylinder_collider_produces_mesh() {
    let params = cylinder_params();
    let size = Vec3::new(2.0, 2.0, 4.0);
    let cache = ShapeCache::new();

    let result = create_physics_shape(&params, size, PhysicsShapeType::Prim, &cache, 0)
        .expect("cylinder should produce a shape");

    assert!(
        result.vertex_count > 0,
        "cylinder collider should have vertices (meshed, not analytic)"
    );
}

/// Torus-like params should produce a meshed shape with real geometry.
#[test]
fn test_torus_collider_produces_mesh() {
    let params = torus_params();
    let size = Vec3::splat(3.0);
    let cache = ShapeCache::new();

    let result = create_physics_shape(&params, size, PhysicsShapeType::Prim, &cache, 0)
        .expect("torus should produce a shape");

    assert!(
        result.vertex_count > 0,
        "torus collider should have vertices (meshed shape)"
    );
}

/// A sculpt prim without texture data should still produce a shape using the
/// placeholder geometry path.
#[test]
fn test_sculpt_placeholder_collider() {
    let params = sculpt_params_sphere();
    let size = Vec3::splat(2.0);
    let cache = ShapeCache::new();

    let result = create_physics_shape(&params, size, PhysicsShapeType::Prim, &cache, 0)
        .expect("sculpt prim should produce a shape even without texture data");

    assert!(
        result.vertex_count > 0,
        "sculpt placeholder should produce geometry"
    );

    // AABB must be non-degenerate.
    let aabb = result.shape.compute_local_aabb();
    let ex = aabb.maxs.x - aabb.mins.x;
    let ey = aabb.maxs.y - aabb.mins.y;
    let ez = aabb.maxs.z - aabb.mins.z;
    assert!(ex > 0.01, "sculpt AABB x extent is degenerate: {}", ex);
    assert!(ey > 0.01, "sculpt AABB y extent is degenerate: {}", ey);
    assert!(ez > 0.01, "sculpt AABB z extent is degenerate: {}", ez);
}

// ===========================================================================
// 6.2 -- Drop tests
// ===========================================================================

/// A physical box dropped above flat terrain must land on it, not fall through.
#[test]
fn test_box_drops_on_terrain() {
    let mut scene = RapierPhysicsScene::new("drop_test");

    // Flat terrain at Z = 20.
    let terrain_height = 20.0_f32;
    let side: usize = 64;
    let heightmap = flat_terrain(side, terrain_height);
    scene.set_terrain(&heightmap);

    // Real box collider: 1x1x1 (half-extents 0.5).
    let box_size = Vec3::splat(1.0);
    let box_shape = shape_for_box(box_size * 0.5);

    // Place the box at Z = 30, well above terrain.
    let start_pos = Vec3::new(32.0, 32.0, 30.0);
    let local_id: u32 = 1;
    scene.add_prim_with_shape(
        local_id,
        "drop_box",
        start_pos,
        box_size,
        Quat::IDENTITY,
        true, // physical
        Some(Box::new(box_shape)),
    );

    // Simulate 200 steps (physics uses its internal dt ~ 1/54.5 s per step).
    for _ in 0..200 {
        scene.simulate(0.01);
    }

    let actor = scene.get_actor(local_id).expect("actor should still exist");
    let final_pos = actor.position();
    let half_box = box_size.z * 0.5;

    // The box must not have fallen through the terrain.
    assert!(
        final_pos.z > terrain_height - 1.0,
        "box fell through terrain: final z = {:.3}, terrain = {}",
        final_pos.z,
        terrain_height
    );

    // The box should be near the expected resting position
    // (terrain_height + half_box_height).
    let expected_z = terrain_height + half_box;
    assert!(
        final_pos.z < expected_z + 3.0,
        "box too far above terrain: final z = {:.3}, expected ~{:.1}",
        final_pos.z,
        expected_z
    );

    // Velocity should be small (box has settled).
    let vel = actor.velocity();
    assert!(
        vel.length() < 3.0,
        "box should be mostly at rest, velocity magnitude = {:.3}",
        vel.length()
    );
}

/// A physical box dropped from height should land on a static box below it,
/// not pass through.
#[test]
fn test_object_lands_on_object() {
    let mut scene = RapierPhysicsScene::new("stacking_test");

    // Flat terrain at Z = 0.
    let side: usize = 64;
    let heightmap = flat_terrain(side, 0.0);
    scene.set_terrain(&heightmap);

    // Static platform box: 4x4x2, center at Z = 1.0 (rests on terrain).
    let static_size = Vec3::new(4.0, 4.0, 2.0);
    let static_shape = shape_for_box(static_size * 0.5);
    let static_pos = Vec3::new(32.0, 32.0, 1.0);
    scene.add_prim_with_shape(
        10,
        "static_box",
        static_pos,
        static_size,
        Quat::IDENTITY,
        false, // fixed / static
        Some(Box::new(static_shape)),
    );

    // Dynamic box: 1x1x1, dropped from Z = 10.
    let drop_size = Vec3::splat(1.0);
    let drop_shape = shape_for_box(drop_size * 0.5);
    let drop_start = Vec3::new(32.0, 32.0, 10.0);
    scene.add_prim_with_shape(
        20,
        "drop_box",
        drop_start,
        drop_size,
        Quat::IDENTITY,
        true, // physical
        Some(Box::new(drop_shape)),
    );

    // Simulate long enough for the drop box to settle.
    for _ in 0..400 {
        scene.simulate(0.01);
    }

    let actor = scene.get_actor(20).expect("drop box should still exist");
    let final_pos = actor.position();

    // The static box top surface is at Z = static_pos.z + static_half_z = 1.0 + 1.0 = 2.0.
    // The drop box should rest on top: center at Z ~ 2.0 + 0.5 = 2.5.
    let static_top = static_pos.z + static_size.z * 0.5;

    assert!(
        final_pos.z > static_top - 1.0,
        "drop box fell through static box: final z = {:.3}, static top = {:.1}",
        final_pos.z,
        static_top
    );
    assert!(
        final_pos.z < static_top + drop_size.z + 3.0,
        "drop box too far above static box: final z = {:.3}, expected ~{:.1}",
        final_pos.z,
        static_top + drop_size.z * 0.5
    );
}

// ===========================================================================
// 6.3 -- Performance
// ===========================================================================

/// Meshing 1 000 mixed prims (boxes, spheres, hollowed boxes, cylinders)
/// must complete within 5 seconds.
#[test]
fn test_mesh_1000_prims_under_5_seconds() {
    let cache = ShapeCache::new();
    let start = Instant::now();

    for i in 0..1000_u64 {
        let (params, size) = match i % 4 {
            0 => {
                // Plain box (analytic path).
                (box_params(), Vec3::new(1.0 + (i as f32 % 5.0), 2.0, 3.0))
            }
            1 => {
                // Sphere (analytic path, uniform scale).
                (sphere_params(), Vec3::splat(1.0 + (i as f32 % 3.0)))
            }
            2 => {
                // Hollowed box -- forces trimesh generation.
                let params = VolumeParams {
                    profile: ProfileParams {
                        curve_type: LL_PCODE_PROFILE_SQUARE,
                        begin: 0.0,
                        end: 1.0,
                        hollow: 0.25 + (i as f32 % 3.0) * 0.1,
                    },
                    path: PathParams::default(),
                    sculpt: None,
                };
                (params, Vec3::splat(2.0))
            }
            3 => {
                // Cylinder-like (circle profile + circle path) -- forces meshing.
                (
                    cylinder_params(),
                    Vec3::new(1.0, 1.0, 2.0 + (i as f32 % 4.0)),
                )
            }
            _ => unreachable!(),
        };

        // Unique cache keys: each prim is meshed from scratch (worst case).
        let _ = create_physics_shape(&params, size, PhysicsShapeType::Prim, &cache, i + 1);
    }

    let elapsed = start.elapsed();

    assert!(
        elapsed.as_secs_f64() < 5.0,
        "meshing 1000 prims took {:.2}s, budget is 5.0s",
        elapsed.as_secs_f64()
    );

    eprintln!(
        "Performance: meshed 1000 mixed prims in {:.3}s ({:.0} prims/sec)",
        elapsed.as_secs_f64(),
        1000.0 / elapsed.as_secs_f64()
    );
}
