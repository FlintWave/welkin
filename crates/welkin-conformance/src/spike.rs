// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::path::Path;

use crate::trajectory::TrajectoryRecording;
use rapier3d::prelude::*;

pub struct SpikeResult {
    pub actor_id: u32,
    pub max_position_error: f32,
    pub mean_position_error: f32,
    pub frame_count: u32,
}

pub struct SpikeReport {
    pub results: Vec<SpikeResult>,
    pub overall_pass: bool,
    pub tolerance: f32,
}

pub fn run_spike(path: &Path, tolerance: f32) -> Result<SpikeReport, Box<dyn std::error::Error>> {
    let recording = TrajectoryRecording::load(path)?;
    let frames = &recording.frames;

    let gravity = Vector::new(0.0, 0.0, -9.8);
    let params = IntegrationParameters {
        dt: 1.0 / 54.5454,
        ..IntegrationParameters::default()
    };

    let mut bodies = RigidBodySet::new();
    let mut colliders = ColliderSet::new();
    let mut islands = IslandManager::new();
    let mut broad_phase = BroadPhaseBvh::new();
    let mut narrow_phase = NarrowPhase::new();
    let mut impulse_joints = ImpulseJointSet::new();
    let mut multibody_joints = MultibodyJointSet::new();
    let mut soft_bodies = SoftBodySet::new();
    let mut ccd = CCDSolver::new();
    let mut pipeline = PhysicsPipeline::new();

    let ground_body = bodies.insert(
        RigidBodyBuilder::fixed()
            .translation(Vector::new(128.0, 128.0, 12.5))
            .build(),
    );
    colliders.insert_with_parent(
        ColliderBuilder::cuboid(128.0, 128.0, 12.5).build(),
        ground_body,
        &mut bodies,
    );

    let mut actor_ids = Vec::new();
    let mut actor_index: HashMap<u32, usize> = HashMap::new();

    for frame in frames {
        if frame.step == 0 && !actor_index.contains_key(&frame.local_id) {
            actor_index.insert(frame.local_id, actor_ids.len());
            actor_ids.push(frame.local_id);
        }
    }

    let mut body_handles = Vec::with_capacity(actor_ids.len());
    for &id in &actor_ids {
        let initial = frames
            .iter()
            .find(|f| f.step == 0 && f.local_id == id)
            .expect("actor must have step-0 frame");

        let p = initial.position;
        let v = initial.velocity;

        let handle = bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(p.x, p.y, p.z))
                .linvel(Vector::new(v.x, v.y, v.z))
                .build(),
        );
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(0.5, 0.5, 0.5).build(),
            handle,
            &mut bodies,
        );
        body_handles.push(handle);
    }

    let mut sum_error = vec![0.0_f32; actor_ids.len()];
    let mut max_error = vec![0.0_f32; actor_ids.len()];
    let mut frame_counts = vec![0_u32; actor_ids.len()];

    let max_step = frames.iter().map(|f| f.step).max().unwrap_or(0);

    for _current_step in 1..=max_step {
        pipeline.step(
            gravity,
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

        for frame in frames {
            if frame.step != _current_step {
                continue;
            }
            let Some(&idx) = actor_index.get(&frame.local_id) else {
                continue;
            };

            let sim = bodies[body_handles[idx]].translation();
            let rec = frame.position;

            let dx = sim.x - rec.x;
            let dy = sim.y - rec.y;
            let dz = sim.z - rec.z;
            let error = (dx * dx + dy * dy + dz * dz).sqrt();

            sum_error[idx] += error;
            if error > max_error[idx] {
                max_error[idx] = error;
            }
            frame_counts[idx] += 1;
        }
    }

    let mut results = Vec::with_capacity(actor_ids.len());
    for i in 0..actor_ids.len() {
        let mean = if frame_counts[i] > 0 {
            sum_error[i] / frame_counts[i] as f32
        } else {
            0.0
        };
        results.push(SpikeResult {
            actor_id: actor_ids[i],
            max_position_error: max_error[i],
            mean_position_error: mean,
            frame_count: frame_counts[i],
        });
    }

    let overall_pass = results.iter().all(|r| r.max_position_error <= tolerance);

    Ok(SpikeReport {
        results,
        overall_pass,
        tolerance,
    })
}
