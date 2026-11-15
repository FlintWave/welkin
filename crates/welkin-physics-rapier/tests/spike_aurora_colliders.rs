// SPDX-License-Identifier: GPL-3.0-or-later

use aurora_prim::params::{
    LL_PCODE_PATH_CIRCLE, LL_PCODE_PATH_LINE, LL_PCODE_PROFILE_CIRCLE_HALF,
    LL_PCODE_PROFILE_SQUARE, PathParams, ProfileParams, VolumeParams,
};
use rapier3d::prelude::*;

fn box_params() -> VolumeParams {
    VolumeParams {
        profile: ProfileParams {
            curve_type: LL_PCODE_PROFILE_SQUARE,
            begin: 0.0,
            end: 1.0,
            hollow: 0.0,
        },
        path: PathParams {
            curve_type: LL_PCODE_PATH_LINE,
            begin: 0.0,
            end: 1.0,
            scale: [1.0, 1.0],
            shear: [0.0, 0.0],
            twist_begin: 0.0,
            twist_end: 0.0,
            radius_offset: 0.0,
            taper: [0.0, 0.0],
            revolutions: 1.0,
            skew: 0.0,
        },
        sculpt: None,
    }
}

fn sphere_params() -> VolumeParams {
    VolumeParams {
        profile: ProfileParams {
            curve_type: LL_PCODE_PROFILE_CIRCLE_HALF,
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

fn combined_geometry(params: &VolumeParams, detail: f32) -> (Vec<Vector>, Vec<[u32; 3]>) {
    let mesh = aurora_prim::volume::generate_volume(params, detail);
    assert!(!mesh.faces.is_empty());

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for face in &mesh.faces {
        assert!(!face.positions.is_empty());
        assert!(!face.indices.is_empty());
        // Each face has its own vertex buffer, so indices are offset into the combined one.
        let offset = vertices.len() as u32;
        vertices.extend(face.positions.iter().map(|p| Vector::new(p[0], p[1], p[2])));
        indices.extend(face.indices.chunks_exact(3).map(|t| {
            [
                offset + t[0] as u32,
                offset + t[1] as u32,
                offset + t[2] as u32,
            ]
        }));
    }
    assert!(!vertices.is_empty());
    assert!(!indices.is_empty());
    (vertices, indices)
}

#[test]
fn prim_box_collider() {
    let (vertices, indices) = combined_geometry(&box_params(), 1.0);

    let collider = ColliderBuilder::trimesh(vertices.clone(), indices)
        .expect("trimesh builds from box prim")
        .build();
    assert!(collider.shape().as_trimesh().is_some());

    let hull = ColliderBuilder::convex_hull(&vertices).expect("convex hull builds from box prim");
    assert!(hull.build().shape().as_convex_polyhedron().is_some());
}

#[test]
fn prim_sphere_collider() {
    let (vertices, indices) = combined_geometry(&sphere_params(), 2.5);

    let collider = ColliderBuilder::trimesh(vertices.clone(), indices)
        .expect("trimesh builds from sphere prim")
        .build();
    assert!(collider.shape().as_trimesh().is_some());

    assert!(ColliderBuilder::convex_hull(&vertices).is_some());
}

#[test]
fn rapier_physics_step() {
    let (vertices, _) = combined_geometry(&box_params(), 1.0);

    let mut bodies = RigidBodySet::new();
    let mut colliders = ColliderSet::new();

    colliders.insert(ColliderBuilder::halfspace(nalgebra::Unit::new_unchecked(
        Vector::Y,
    )));

    let body = bodies.insert(RigidBodyBuilder::dynamic().translation(Vector::new(0.0, 5.0, 0.0)));
    // A trimesh has no volume, so the dynamic body uses the convex hull of the same positions.
    colliders.insert_with_parent(
        ColliderBuilder::convex_hull(&vertices).expect("convex hull"),
        body,
        &mut bodies,
    );

    let mut pipeline = PhysicsPipeline::new();
    let params = IntegrationParameters {
        dt: 1.0 / 60.0,
        ..IntegrationParameters::default()
    };
    let mut islands = IslandManager::new();
    let mut broad_phase = BroadPhaseBvh::new();
    let mut narrow_phase = NarrowPhase::new();
    let mut impulse_joints = ImpulseJointSet::new();
    let mut multibody_joints = MultibodyJointSet::new();
    let mut soft_bodies = SoftBodySet::new();
    let mut ccd = CCDSolver::new();

    for _ in 0..60 {
        pipeline.step(
            Vector::new(0.0, -9.81, 0.0),
            &params,
            &mut islands,
            &mut broad_phase,
            &mut narrow_phase,
            &mut bodies,
            &mut colliders,
            &mut impulse_joints,
            &mut multibody_joints,
            &mut soft_bodies,
            &mut ccd,
            &(),
            &(),
        );
    }

    let y = bodies[body].translation().y;
    assert!(y < 5.0, "box did not fall: y = {y}");
    assert!(y > -1.0, "box fell through the ground: y = {y}");
}
