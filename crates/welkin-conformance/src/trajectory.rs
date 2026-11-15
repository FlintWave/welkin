// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{Quat, Vec3};
use serde::Deserialize;
use std::io::BufRead;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
struct RawRecord {
    s: u32,
    id: u32,
    t: i32,
    p: [f32; 3],
    v: [f32; 3],
    o: [f32; 4],
    av: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrajectoryFrame {
    pub step: u32,
    pub local_id: u32,
    pub actor_type: i32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub orientation: Quat,
    pub angular_velocity: Vec3,
}

#[derive(Debug)]
pub struct TrajectoryRecording {
    pub frames: Vec<TrajectoryFrame>,
    pub step_count: u32,
    pub actor_ids: Vec<u32>,
}

impl TrajectoryRecording {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut frames = Vec::new();
        let mut max_step = 0u32;
        let mut ids = std::collections::BTreeSet::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let raw: RawRecord = serde_json::from_str(line.trim())?;
            max_step = max_step.max(raw.s);
            ids.insert(raw.id);
            frames.push(TrajectoryFrame {
                step: raw.s,
                local_id: raw.id,
                actor_type: raw.t,
                position: Vec3::new(raw.p[0], raw.p[1], raw.p[2]),
                velocity: Vec3::new(raw.v[0], raw.v[1], raw.v[2]),
                orientation: Quat::from_xyzw(raw.o[0], raw.o[1], raw.o[2], raw.o[3]),
                angular_velocity: Vec3::new(raw.av[0], raw.av[1], raw.av[2]),
            });
        }

        let step_count = if frames.is_empty() { 0 } else { max_step + 1 };
        Ok(TrajectoryRecording {
            frames,
            step_count,
            actor_ids: ids.into_iter().collect(),
        })
    }

    pub fn frames_for_actor(&self, local_id: u32) -> Vec<&TrajectoryFrame> {
        self.frames
            .iter()
            .filter(|f| f.local_id == local_id)
            .collect()
    }

    pub fn frames_at_step(&self, step: u32) -> Vec<&TrajectoryFrame> {
        self.frames.iter().filter(|f| f.step == step).collect()
    }
}
