// SPDX-License-Identifier: LGPL-3.0-or-later
// TODO: requires region-simulator-core (Phase 3)

use serde::{Deserialize, Serialize};

use crate::CapacityError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub object_count: u32,
    pub script_count: u32,
    pub avatar_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub frame_time_ms: f64,
    pub memory_per_island_mb: f64,
    pub memory_per_avatar_mb: f64,
    pub bytes_per_avatar_per_sec: f64,
    pub config: BenchmarkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadBenchmark {
    pub name: String,
    pub result: BenchmarkResult,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            object_count: 5000,
            script_count: 1000,
            avatar_count: 10,
        }
    }
}

pub async fn run_benchmark(_config: &BenchmarkConfig) -> Result<BenchmarkResult, CapacityError> {
    // TODO: run synthetic island with region-simulator-core loopback avatars
    Err(CapacityError::Unavailable(
        "benchmark requires region-simulator-core (Phase 3)".into(),
    ))
}

pub fn workload_configs() -> Vec<(String, BenchmarkConfig)> {
    vec![
        (
            "quiet".into(),
            BenchmarkConfig {
                object_count: 3000,
                script_count: 300,
                avatar_count: 5,
            },
        ),
        (
            "typical".into(),
            BenchmarkConfig {
                object_count: 8000,
                script_count: 2400,
                avatar_count: 20,
            },
        ),
        (
            "busy".into(),
            BenchmarkConfig {
                object_count: 14000,
                script_count: 8400,
                avatar_count: 60,
            },
        ),
    ]
}
