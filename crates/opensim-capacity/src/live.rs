// SPDX-License-Identifier: GPL-3.0-or-later
// TODO: feed measured region statistics into model (Phase 3)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionStatistics {
    pub region_name: String,
    pub frame_time_ms: f64,
    pub physics_time_ms: f64,
    pub script_time_ms: f64,
    pub agents: u32,
    pub active_objects: u32,
    pub active_scripts: u32,
    pub packets_per_sec: f64,
    pub unacked_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityWarning {
    pub region_name: String,
    pub resource: String,
    pub current_usage_pct: f64,
    pub threshold_pct: f64,
    pub message: String,
}

pub const WARNING_THRESHOLD_PCT: f64 = 80.0;
pub const CRITICAL_THRESHOLD_PCT: f64 = 95.0;

pub fn check_warnings(stats: &[RegionStatistics]) -> Vec<CapacityWarning> {
    // TODO: compare measured stats against model predictions
    // and emit warnings when approaching limits
    let _ = stats;
    Vec::new()
}

pub fn update_model_from_stats(
    _stats: &[RegionStatistics],
    _model: &mut super::model::CalibratedModel,
) {
    // TODO: replace estimate coefficients with measured values
}
