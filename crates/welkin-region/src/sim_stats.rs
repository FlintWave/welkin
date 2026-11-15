// SPDX-License-Identifier: LGPL-3.0-or-later

use std::time::Instant;
use welkin_msg::msgs::{SimStats, sim_stats};

#[derive(Debug, Clone)]
pub struct IntervalAverages {
    pub interval_secs: f32,
    pub sim_fps: f32,
    pub physics_fps: f32,
    pub frame_ms: f32,
    pub physics_ms: f32,
    pub script_ms: f32,
}

#[derive(Debug, Clone)]
pub struct FrameMetrics {
    frames: u32,
    physics_steps: u32,
    frame_ms_sum: f32,
    physics_ms_sum: f32,
    script_ms_sum: f32,
    last_take: Instant,
}

impl FrameMetrics {
    pub fn new() -> Self {
        Self {
            frames: 0,
            physics_steps: 0,
            frame_ms_sum: 0.0,
            physics_ms_sum: 0.0,
            script_ms_sum: 0.0,
            last_take: Instant::now(),
        }
    }

    pub fn record(&mut self, frame_ms: f32, physics_ms: f32, script_ms: f32) {
        self.frames += 1;
        if physics_ms > 0.0 {
            self.physics_steps += 1;
        }
        self.frame_ms_sum += frame_ms;
        self.physics_ms_sum += physics_ms;
        self.script_ms_sum += script_ms;
    }

    pub fn take(&mut self) -> IntervalAverages {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_take).as_secs_f32();
        let interval_secs = if elapsed > 0.0 { elapsed } else { 1.0 };
        let sim_fps = if self.frames == 0 {
            0.0
        } else {
            self.frames as f32 / interval_secs
        };
        let physics_fps = if self.physics_steps == 0 {
            0.0
        } else {
            self.physics_steps as f32 / interval_secs
        };
        let physics_fps = if physics_fps > sim_fps && sim_fps > 0.0 {
            sim_fps
        } else {
            physics_fps
        };
        let frame_ms = if self.frames == 0 {
            0.0
        } else {
            self.frame_ms_sum / self.frames as f32
        };
        let physics_ms = if self.frames == 0 {
            0.0
        } else {
            self.physics_ms_sum / self.frames as f32
        };
        let script_ms = if self.frames == 0 {
            0.0
        } else {
            self.script_ms_sum / self.frames as f32
        };

        self.frames = 0;
        self.physics_steps = 0;
        self.frame_ms_sum = 0.0;
        self.physics_ms_sum = 0.0;
        self.script_ms_sum = 0.0;
        self.last_take = now;

        IntervalAverages {
            interval_secs,
            sim_fps,
            physics_fps,
            frame_ms,
            physics_ms,
            script_ms,
        }
    }
}

impl Default for FrameMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SimStatsInput {
    pub region_x: u32,
    pub region_y: u32,
    pub region_flags: u32,
    pub object_capacity: u32,
    pub root_agents: u32,
    pub child_agents: u32,
    pub total_prims: u32,
    pub active_prims: u32,
    pub active_scripts: u32,
    pub averages: IntervalAverages,
}

const STAT_IDS: [u32; 41] = [
    0, 1, 2, 3, 13, 14, 11, 12, 4, 5, 7, 9, 6, 17, 18, 24, 8, 19, 20, 15,
    33, 32, 27, 21, 22, 23, 25, 26, 31, 38, 34, 35, 36, 37, 39, 40, 30,
    10, 16, 28, 29,
];

pub fn build_sim_stats(input: &SimStatsInput) -> SimStats {
    let mins = input.averages.frame_ms;
    let physics_ms = input.averages.physics_ms;
    let agent_ms = 0.0f32;
    let sim_fps = input.averages.sim_fps;
    let time_dilation = if input.averages.interval_secs <= 0.0 || sim_fps == 0.0 {
        0.0
    } else {
        let value = (sim_fps / 45.0).clamp(0.0, 1.0);
        (value * 1000.0).round() / 1000.0
    };
    let other_ms = (mins - physics_ms - agent_ms).max(0.0);
    let target_frame_ms = 1000.0 / 45.0;
    let sim_spare_ms = (target_frame_ms - (physics_ms + agent_ms + other_ms)).clamp(0.0, mins);

    let mut values = [0.0f32; 41];
    values[0] = time_dilation;
    values[1] = sim_fps;
    values[2] = input.averages.physics_fps;
    values[3] = 0.0;
    values[4] = input.root_agents as f32;
    values[5] = input.child_agents as f32;
    values[6] = input.total_prims as f32;
    values[7] = input.active_prims as f32;
    values[8] = mins;
    values[9] = 0.0;
    values[10] = physics_ms;
    values[11] = 0.0;
    values[12] = other_ms;
    values[13] = 0.0;
    values[14] = 0.0;
    values[15] = 0.0;
    values[16] = agent_ms;
    values[17] = 0.0;
    values[18] = 0.0;
    values[19] = input.active_scripts as f32;
    values[20] = 0.0;
    values[21] = sim_spare_ms;
    values[22] = 20.0;
    values[23] = 0.0;
    values[24] = 0.0;
    values[25] = 0.0;
    values[26] = 0.0;
    values[27] = 0.0;
    values[28] = 0.0;
    values[29] = 0.0;
    values[30] = 0.0;
    values[31] = 0.0;
    values[32] = 0.0;
    values[33] = 0.0;
    values[34] = 0.0;
    values[35] = 0.0;
    values[36] = 0.0;
    values[37] = input.averages.script_ms;
    values[38] = 0.0;
    values[39] = 0.0;
    values[40] = 0.0;

    let stat = STAT_IDS
        .iter()
        .zip(values.iter())
        .map(|(&stat_id, &stat_value)| sim_stats::Stat {
            stat_id,
            stat_value,
        })
        .collect();

    SimStats {
        region: sim_stats::Region {
            region_x: input.region_x,
            region_y: input.region_y,
            region_flags: input.region_flags,
            object_capacity: input.object_capacity,
        },
        stat,
        pid_stat: sim_stats::PidStat { pid: 0 },
        region_info: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_41_stats_in_wire_order() {
        let input = SimStatsInput {
            region_x: 0,
            region_y: 0,
            region_flags: 0,
            object_capacity: 45000,
            root_agents: 7,
            child_agents: 0,
            total_prims: 0,
            active_prims: 0,
            active_scripts: 0,
            averages: IntervalAverages {
                interval_secs: 1.0,
                sim_fps: 45.0,
                physics_fps: 45.0,
                frame_ms: 22.222,
                physics_ms: 10.0,
                script_ms: 2.0,
            },
        };
        let stats = build_sim_stats(&input);
        assert_eq!(stats.stat.len(), 41);
        for (index, stat) in stats.stat.iter().enumerate() {
            assert_eq!(stat.stat_id, STAT_IDS[index]);
        }
        let agents = stats.stat.iter().find(|s| s.stat_id == 13).unwrap();
        assert_eq!(agents.stat_value, 7.0);
        let unacked = stats.stat.iter().find(|s| s.stat_id == 24).unwrap();
        assert_eq!(unacked.stat_value, 0.0);
        assert_eq!(stats.region_info.len(), 0);
        assert_eq!(stats.pid_stat.pid, 0);
    }

    #[test]
    fn zero_frames_time_dilation_is_zero_not_nan() {
        let averages = IntervalAverages {
            interval_secs: 1.0,
            sim_fps: 0.0,
            physics_fps: 0.0,
            frame_ms: 0.0,
            physics_ms: 0.0,
            script_ms: 0.0,
        };
        let input = SimStatsInput {
            region_x: 0,
            region_y: 0,
            region_flags: 0,
            object_capacity: 45000,
            root_agents: 0,
            child_agents: 0,
            total_prims: 0,
            active_prims: 0,
            active_scripts: 0,
            averages,
        };
        let stats = build_sim_stats(&input);
        let td = stats.stat[0].stat_value;
        assert!(!td.is_nan());
        assert_eq!(td, 0.0);
    }

    #[test]
    fn spare_ms_clamps() {
        let averages = IntervalAverages {
            interval_secs: 1.0,
            sim_fps: 45.0,
            physics_fps: 45.0,
            frame_ms: 5.0,
            physics_ms: 4.0,
            script_ms: 1.0,
        };
        let input = SimStatsInput {
            region_x: 0,
            region_y: 0,
            region_flags: 0,
            object_capacity: 45000,
            root_agents: 0,
            child_agents: 0,
            total_prims: 0,
            active_prims: 0,
            active_scripts: 0,
            averages,
        };
        let stats = build_sim_stats(&input);
        let spare = stats.stat.iter().find(|s| s.stat_id == 32).unwrap().stat_value;
        assert!(spare >= 0.0 && spare <= 5.0);
    }
}
