// SPDX-License-Identifier: GPL-3.0-or-later
// TODO: calibrate against cutover load tests (Phase 3)

use serde::{Deserialize, Serialize};

use crate::benchmark::BenchmarkResult;
use crate::estimate::{CapacityEstimate, EstimateRange};
use crate::report::HostReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibratedModel {
    pub measured_coefficients: Vec<MeasuredCoefficient>,
    pub calibration_error: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasuredCoefficient {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeoffPoint {
    pub islands: u32,
    pub users_per_island: u32,
    pub total_users: u32,
    pub limiting_resource: String,
}

pub fn calibrate_from_benchmark(
    _report: &HostReport,
    _benchmark: &BenchmarkResult,
) -> CalibratedModel {
    // TODO: derive measured coefficients from benchmark results
    CalibratedModel {
        measured_coefficients: Vec::new(),
        calibration_error: None,
    }
}

pub fn island_user_tradeoff(
    _report: &HostReport,
    _model: &CalibratedModel,
) -> Vec<TradeoffPoint> {
    // TODO: compute trade-off curve from calibrated model
    Vec::new()
}

pub fn calibrated_estimate(
    _report: &HostReport,
    _model: &CalibratedModel,
    _workload: &str,
) -> CapacityEstimate {
    // TODO: produce measured estimate from calibrated model
    CapacityEstimate {
        islands: EstimateRange { min: 0, max: 0 },
        total_users: EstimateRange { min: 0, max: 0 },
        limiting_resource: "uncalibrated".into(),
        workload: "unknown".into(),
        is_measured: false,
    }
}

pub fn calibrate_against_load_test(
    _model: &mut CalibratedModel,
    _load_test_results: &serde_json::Value,
) -> f64 {
    // TODO: compare model predictions against cutover load test results
    // Return calibration error percentage
    f64::NAN
}
