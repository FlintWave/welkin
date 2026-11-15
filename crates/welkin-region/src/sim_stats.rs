// SPDX-License-Identifier: LGPL-3.0-or-later

use welkin_msg::msgs::{SimStats, sim_stats};

pub fn build_sim_stats(
    region_x: u32,
    region_y: u32,
    region_flags: u32,
    root_agents: u32,
    child_agents: u32,
    total_prims: u32,
    active_prims: u32,
    active_scripts: u32,
    frame_ms: f32,
    physics_ms: f32,
    net_ms: f32,
    script_ms: f32,
    time_dilation: f32,
) -> SimStats {
    let time_dilation = time_dilation.clamp(0.0, 1.0);
    let other_ms = (frame_ms - physics_ms - net_ms - script_ms).max(0.0);
    let sim_spare_ms = (22.222 - frame_ms).max(0.0);

    SimStats {
        region: sim_stats::Region {
            region_x,
            region_y,
            region_flags,
            object_capacity: 45000,
        },
        stat: vec![
            sim_stats::Stat { stat_id: 0, stat_value: time_dilation },
            sim_stats::Stat { stat_id: 1, stat_value: 45.0 },
            sim_stats::Stat { stat_id: 2, stat_value: 45.0 },
            sim_stats::Stat { stat_id: 3, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 4, stat_value: root_agents as f32 },
            sim_stats::Stat { stat_id: 5, stat_value: child_agents as f32 },
            sim_stats::Stat { stat_id: 6, stat_value: total_prims as f32 },
            sim_stats::Stat { stat_id: 7, stat_value: active_prims as f32 },
            sim_stats::Stat { stat_id: 8, stat_value: frame_ms },
            sim_stats::Stat { stat_id: 9, stat_value: net_ms },
            sim_stats::Stat { stat_id: 10, stat_value: physics_ms },
            sim_stats::Stat { stat_id: 11, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 12, stat_value: other_ms },
            sim_stats::Stat { stat_id: 13, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 14, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 15, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 16, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 17, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 18, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 19, stat_value: active_scripts as f32 },
            sim_stats::Stat { stat_id: 20, stat_value: 0.0 },
            sim_stats::Stat { stat_id: 21, stat_value: sim_spare_ms },
            sim_stats::Stat { stat_id: 37, stat_value: script_ms },
            sim_stats::Stat { stat_id: 38, stat_value: 0.0 },
        ],
        pid_stat: sim_stats::PidStat {
            pid: std::process::id() as i32,
        },
        region_info: vec![sim_stats::RegionInfo {
            region_flags_extended: region_flags as u64,
        }],
    }
}
