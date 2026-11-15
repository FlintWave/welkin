// SPDX-License-Identifier: GPL-3.0-or-later

//! Load-test harness and measurement for OpenSimulator C# vs Rust comparison.
//!
//! Provides configuration, metric collection, baseline persistence, and
//! comparison reporting for cutover section 4 (load testing).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during load-test operations.
#[derive(Debug)]
pub enum LoadTestError {
    /// A required configuration field is missing or invalid.
    ConfigValidation(String),
    /// An I/O error occurred (file read/write, network, etc.).
    Io(std::io::Error),
    /// JSON serialisation/deserialisation failed.
    Json(serde_json::Error),
    /// A runtime error during the load test.
    Runtime(String),
}

impl std::fmt::Display for LoadTestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigValidation(msg) => write!(f, "config validation: {msg}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Runtime(msg) => write!(f, "runtime error: {msg}"),
        }
    }
}

impl std::error::Error for LoadTestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LoadTestError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for LoadTestError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Which implementation is under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    CSharp,
    Rust,
}

/// Configuration for a load-test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestConfig {
    /// Number of concurrent simulated avatar connections.
    pub concurrent_avatars: u32,
    /// Number of physics objects to spawn.
    pub physics_objects: u32,
    /// Number of active scripts firing events.
    pub active_scripts: u32,
    /// Concurrent asset-transfer requests per region.
    pub asset_transfers_per_region: u32,
    /// How long the test should run, in seconds.
    pub duration_secs: u64,
    /// Base URI of the simulator/grid under test.
    pub target_uri: String,
    /// Region names to target.
    pub regions: Vec<String>,
}

impl LoadTestConfig {
    /// Validate that all required fields are populated and sane.
    pub fn validate(&self) -> Result<(), LoadTestError> {
        if self.concurrent_avatars == 0 {
            return Err(LoadTestError::ConfigValidation(
                "concurrent_avatars must be > 0".into(),
            ));
        }
        if self.duration_secs == 0 {
            return Err(LoadTestError::ConfigValidation(
                "duration_secs must be > 0".into(),
            ));
        }
        if self.target_uri.is_empty() {
            return Err(LoadTestError::ConfigValidation(
                "target_uri must not be empty".into(),
            ));
        }
        if self.regions.is_empty() {
            return Err(LoadTestError::ConfigValidation(
                "regions must not be empty".into(),
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

/// Collected metrics from a single load-test run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoadMetrics {
    /// Average frames per second over the run.
    pub fps_avg: f64,
    /// Minimum instantaneous FPS observed.
    pub fps_min: f64,
    /// Script events processed per second.
    pub script_eps: f64,
    /// Peak resident memory in megabytes.
    pub memory_mb: f64,
    /// Network bandwidth consumed (Mbps).
    pub network_bandwidth_mbps: f64,
    /// Average region-crossing latency in milliseconds.
    pub region_crossing_latency_ms: f64,
    /// Average asset-fetch latency in milliseconds.
    pub asset_fetch_latency_ms: f64,
    /// Average login latency in milliseconds.
    pub login_latency_ms: f64,
}

impl LoadMetrics {
    /// Return the value of the named metric, or `None` if the name is unknown.
    pub fn get(&self, name: &str) -> Option<f64> {
        match name {
            "fps_avg" => Some(self.fps_avg),
            "fps_min" => Some(self.fps_min),
            "script_eps" => Some(self.script_eps),
            "memory_mb" => Some(self.memory_mb),
            "network_bandwidth_mbps" => Some(self.network_bandwidth_mbps),
            "region_crossing_latency_ms" => Some(self.region_crossing_latency_ms),
            "asset_fetch_latency_ms" => Some(self.asset_fetch_latency_ms),
            "login_latency_ms" => Some(self.login_latency_ms),
            _ => None,
        }
    }

    /// Names of every metric field, in declaration order.
    pub fn field_names() -> &'static [&'static str] {
        &[
            "fps_avg",
            "fps_min",
            "script_eps",
            "memory_mb",
            "network_bandwidth_mbps",
            "region_crossing_latency_ms",
            "asset_fetch_latency_ms",
            "login_latency_ms",
        ]
    }
}

// ---------------------------------------------------------------------------
// Load-test result
// ---------------------------------------------------------------------------

/// Complete result of a load-test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestResult {
    /// Configuration that was used.
    pub config: LoadTestConfig,
    /// Metrics that were measured.
    pub metrics: LoadMetrics,
    /// Actual wall-clock duration in seconds.
    pub duration_actual_secs: f64,
    /// Errors encountered during the run (empty means clean).
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Baseline persistence
// ---------------------------------------------------------------------------

/// A single baseline measurement snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineMeasurement {
    /// Which target produced this baseline.
    pub target: Target,
    /// ISO-8601 timestamp of the measurement.
    pub timestamp: String,
    /// Free-form hardware description (CPU, RAM, etc.).
    pub hardware_description: String,
    /// The config used.
    pub config: LoadTestConfig,
    /// The metrics measured.
    pub metrics: LoadMetrics,
}

/// Persist a baseline measurement to a JSON file.
pub fn save_baseline(path: &Path, measurement: &BaselineMeasurement) -> Result<(), LoadTestError> {
    let json = serde_json::to_string_pretty(measurement)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load a previously saved baseline measurement from a JSON file.
pub fn load_baseline(path: &Path) -> Result<BaselineMeasurement, LoadTestError> {
    let data = std::fs::read_to_string(path)?;
    let measurement: BaselineMeasurement = serde_json::from_str(&data)?;
    Ok(measurement)
}

// ---------------------------------------------------------------------------
// Comparison report
// ---------------------------------------------------------------------------

/// One row in a comparison report — a single metric compared between two baselines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricComparison {
    pub metric_name: String,
    pub csharp_value: f64,
    pub rust_value: f64,
    /// `rust_value / csharp_value` for throughput metrics (higher is better),
    /// `csharp_value / rust_value` for latency/resource metrics (lower is better).
    pub ratio: f64,
    /// `true` when the Rust value is at least as good as (ratio >= 1.0).
    pub pass: bool,
}

/// Full comparison report between a C# and a Rust baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub comparisons: Vec<MetricComparison>,
    /// Overall pass: every individual metric passed.
    pub overall_pass: bool,
}

/// Metrics where *lower is better* (latency, resource usage). For these the
/// ratio is computed as `csharp / rust` so a value >= 1.0 means Rust wins.
const LOWER_IS_BETTER: &[&str] = &[
    "memory_mb",
    "network_bandwidth_mbps",
    "region_crossing_latency_ms",
    "asset_fetch_latency_ms",
    "login_latency_ms",
];

fn is_lower_better(name: &str) -> bool {
    LOWER_IS_BETTER.contains(&name)
}

/// Compare two baselines and produce a report. The `csharp` baseline must have
/// `target == Target::CSharp` and the `rust` baseline must have
/// `target == Target::Rust`, but this function does not enforce that — it
/// simply labels the columns accordingly.
pub fn compare_baselines(
    csharp: &BaselineMeasurement,
    rust: &BaselineMeasurement,
) -> ComparisonReport {
    let mut comparisons = Vec::new();

    for &name in LoadMetrics::field_names() {
        let cs = csharp.metrics.get(name).unwrap_or(0.0);
        let rs = rust.metrics.get(name).unwrap_or(0.0);

        let ratio = if is_lower_better(name) {
            // Lower is better: ratio = cs / rs (>= 1.0 when Rust uses less).
            if rs.abs() < f64::EPSILON {
                if cs.abs() < f64::EPSILON {
                    1.0
                } else {
                    f64::INFINITY
                }
            } else {
                cs / rs
            }
        } else {
            // Higher is better: ratio = rs / cs (>= 1.0 when Rust is faster).
            if cs.abs() < f64::EPSILON {
                if rs.abs() < f64::EPSILON {
                    1.0
                } else {
                    f64::INFINITY
                }
            } else {
                rs / cs
            }
        };

        let pass = ratio >= 1.0 || (ratio - 1.0).abs() < f64::EPSILON;

        comparisons.push(MetricComparison {
            metric_name: name.to_string(),
            csharp_value: cs,
            rust_value: rs,
            ratio,
            pass,
        });
    }

    let overall_pass = comparisons.iter().all(|c| c.pass);
    ComparisonReport {
        comparisons,
        overall_pass,
    }
}

// ---------------------------------------------------------------------------
// Load generators (structural stubs — real I/O deferred to integration)
// ---------------------------------------------------------------------------

/// Simulates N concurrent avatar connections.
pub struct AvatarLoadGenerator {
    pub count: u32,
    pub target_uri: String,
    pub region: String,
}

impl AvatarLoadGenerator {
    pub fn new(count: u32, target_uri: &str, region: &str) -> Self {
        Self {
            count,
            target_uri: target_uri.to_string(),
            region: region.to_string(),
        }
    }

    /// Run the avatar load (login, heartbeat, movement) and return per-avatar
    /// latency samples.
    pub async fn run(&self, duration_secs: u64) -> Result<AvatarLoadResult, LoadTestError> {
        // Structural: in a real deployment this drives LLUDP connections.
        // For now return a placeholder so the orchestrator compiles.
        Ok(AvatarLoadResult {
            logins_attempted: self.count,
            logins_succeeded: self.count,
            avg_login_latency_ms: 0.0,
            avg_heartbeat_latency_ms: 0.0,
            region_crossings: 0,
            avg_crossing_latency_ms: 0.0,
            duration_secs,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarLoadResult {
    pub logins_attempted: u32,
    pub logins_succeeded: u32,
    pub avg_login_latency_ms: f64,
    pub avg_heartbeat_latency_ms: f64,
    pub region_crossings: u32,
    pub avg_crossing_latency_ms: f64,
    pub duration_secs: u64,
}

/// Creates physics objects (boxes, spheres) that interact.
pub struct PhysicsLoadGenerator {
    pub object_count: u32,
    pub target_uri: String,
    pub region: String,
}

impl PhysicsLoadGenerator {
    pub fn new(object_count: u32, target_uri: &str, region: &str) -> Self {
        Self {
            object_count,
            target_uri: target_uri.to_string(),
            region: region.to_string(),
        }
    }

    pub async fn run(&self, _duration_secs: u64) -> Result<PhysicsLoadResult, LoadTestError> {
        Ok(PhysicsLoadResult {
            objects_created: self.object_count,
            avg_fps: 0.0,
            min_fps: 0.0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsLoadResult {
    pub objects_created: u32,
    pub avg_fps: f64,
    pub min_fps: f64,
}

/// Registers N active scripts that fire events.
pub struct ScriptLoadGenerator {
    pub script_count: u32,
    pub target_uri: String,
    pub region: String,
}

impl ScriptLoadGenerator {
    pub fn new(script_count: u32, target_uri: &str, region: &str) -> Self {
        Self {
            script_count,
            target_uri: target_uri.to_string(),
            region: region.to_string(),
        }
    }

    pub async fn run(&self, _duration_secs: u64) -> Result<ScriptLoadResult, LoadTestError> {
        Ok(ScriptLoadResult {
            scripts_registered: self.script_count,
            events_per_second: 0.0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptLoadResult {
    pub scripts_registered: u32,
    pub events_per_second: f64,
}

/// Issues concurrent asset-fetch requests.
pub struct AssetLoadGenerator {
    pub concurrent_fetches: u32,
    pub target_uri: String,
    pub region: String,
}

impl AssetLoadGenerator {
    pub fn new(concurrent_fetches: u32, target_uri: &str, region: &str) -> Self {
        Self {
            concurrent_fetches,
            target_uri: target_uri.to_string(),
            region: region.to_string(),
        }
    }

    pub async fn run(&self, _duration_secs: u64) -> Result<AssetLoadResult, LoadTestError> {
        Ok(AssetLoadResult {
            fetches_attempted: self.concurrent_fetches,
            fetches_succeeded: self.concurrent_fetches,
            avg_latency_ms: 0.0,
            bandwidth_mbps: 0.0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetLoadResult {
    pub fetches_attempted: u32,
    pub fetches_succeeded: u32,
    pub avg_latency_ms: f64,
    pub bandwidth_mbps: f64,
}

// ---------------------------------------------------------------------------
// Load-test runner (orchestrator)
// ---------------------------------------------------------------------------

/// Orchestrates a full load-test run: spawns avatar tasks, physics load,
/// script load, and asset-transfer load.
pub struct LoadTestRunner {
    config: LoadTestConfig,
}

impl LoadTestRunner {
    /// Create a runner from a validated config.
    pub fn new(config: LoadTestConfig) -> Result<Self, LoadTestError> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Execute the full load test and return aggregated results.
    pub async fn run(&self) -> Result<LoadTestResult, LoadTestError> {
        let start = std::time::Instant::now();
        let mut errors = Vec::new();

        // Use the first region as the primary target (others would be hit in
        // a multi-region extension).
        let region = self
            .config
            .regions
            .first()
            .ok_or_else(|| LoadTestError::ConfigValidation("no regions configured".into()))?;

        let avatar_gen =
            AvatarLoadGenerator::new(self.config.concurrent_avatars, &self.config.target_uri, region);
        let physics_gen =
            PhysicsLoadGenerator::new(self.config.physics_objects, &self.config.target_uri, region);
        let script_gen =
            ScriptLoadGenerator::new(self.config.active_scripts, &self.config.target_uri, region);
        let asset_gen = AssetLoadGenerator::new(
            self.config.asset_transfers_per_region,
            &self.config.target_uri,
            region,
        );

        let avatar_res = avatar_gen.run(self.config.duration_secs).await;
        let physics_res = physics_gen.run(self.config.duration_secs).await;
        let script_res = script_gen.run(self.config.duration_secs).await;
        let asset_res = asset_gen.run(self.config.duration_secs).await;

        let avatar = match avatar_res {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("avatar load: {e}"));
                AvatarLoadResult {
                    logins_attempted: 0,
                    logins_succeeded: 0,
                    avg_login_latency_ms: 0.0,
                    avg_heartbeat_latency_ms: 0.0,
                    region_crossings: 0,
                    avg_crossing_latency_ms: 0.0,
                    duration_secs: 0,
                }
            }
        };

        let physics = match physics_res {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("physics load: {e}"));
                PhysicsLoadResult {
                    objects_created: 0,
                    avg_fps: 0.0,
                    min_fps: 0.0,
                }
            }
        };

        let script = match script_res {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("script load: {e}"));
                ScriptLoadResult {
                    scripts_registered: 0,
                    events_per_second: 0.0,
                }
            }
        };

        let asset = match asset_res {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("asset load: {e}"));
                AssetLoadResult {
                    fetches_attempted: 0,
                    fetches_succeeded: 0,
                    avg_latency_ms: 0.0,
                    bandwidth_mbps: 0.0,
                }
            }
        };

        let metrics = LoadMetrics {
            fps_avg: physics.avg_fps,
            fps_min: physics.min_fps,
            script_eps: script.events_per_second,
            memory_mb: 0.0, // Would come from OS-level sampling
            network_bandwidth_mbps: asset.bandwidth_mbps,
            region_crossing_latency_ms: avatar.avg_crossing_latency_ms,
            asset_fetch_latency_ms: asset.avg_latency_ms,
            login_latency_ms: avatar.avg_login_latency_ms,
        };

        let duration_actual_secs = start.elapsed().as_secs_f64();

        Ok(LoadTestResult {
            config: self.config.clone(),
            metrics,
            duration_actual_secs,
            errors,
        })
    }
}

/// Top-level convenience: validate config, run the load test, return results.
pub async fn run_load_test(config: LoadTestConfig) -> Result<LoadTestResult, LoadTestError> {
    let runner = LoadTestRunner::new(config)?;
    runner.run().await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> LoadTestConfig {
        LoadTestConfig {
            concurrent_avatars: 10,
            physics_objects: 50,
            active_scripts: 100,
            asset_transfers_per_region: 5,
            duration_secs: 60,
            target_uri: "http://localhost:9000".into(),
            regions: vec!["TestRegion".into()],
        }
    }

    fn sample_metrics(fps: f64, latency: f64) -> LoadMetrics {
        LoadMetrics {
            fps_avg: fps,
            fps_min: fps * 0.8,
            script_eps: 1000.0,
            memory_mb: 512.0,
            network_bandwidth_mbps: 10.0,
            region_crossing_latency_ms: latency,
            asset_fetch_latency_ms: latency * 0.5,
            login_latency_ms: latency * 2.0,
        }
    }

    fn sample_baseline(target: Target, fps: f64, latency: f64) -> BaselineMeasurement {
        BaselineMeasurement {
            target,
            timestamp: "2026-11-14T00:00:00Z".into(),
            hardware_description: "test-hw".into(),
            config: sample_config(),
            metrics: sample_metrics(fps, latency),
        }
    }

    // -- config validation tests --

    #[test]
    fn test_valid_config_passes() {
        assert!(sample_config().validate().is_ok());
    }

    #[test]
    fn test_zero_avatars_rejected() {
        let mut cfg = sample_config();
        cfg.concurrent_avatars = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("concurrent_avatars"));
    }

    #[test]
    fn test_zero_duration_rejected() {
        let mut cfg = sample_config();
        cfg.duration_secs = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("duration_secs"));
    }

    #[test]
    fn test_empty_uri_rejected() {
        let mut cfg = sample_config();
        cfg.target_uri = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("target_uri"));
    }

    #[test]
    fn test_empty_regions_rejected() {
        let mut cfg = sample_config();
        cfg.regions.clear();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("regions"));
    }

    // -- metric access --

    #[test]
    fn test_metric_get_known_fields() {
        let m = sample_metrics(55.0, 100.0);
        assert!((m.get("fps_avg").expect("known") - 55.0).abs() < f64::EPSILON);
        assert!(m.get("nonexistent").is_none());
    }

    #[test]
    fn test_field_names_count() {
        assert_eq!(LoadMetrics::field_names().len(), 8);
    }

    // -- baseline JSON round-trip --

    #[test]
    fn test_baseline_json_roundtrip() {
        let baseline = sample_baseline(Target::Rust, 55.0, 100.0);
        let json = serde_json::to_string(&baseline).expect("serialize");
        let restored: BaselineMeasurement = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.target, baseline.target);
        assert!(
            (restored.metrics.fps_avg - baseline.metrics.fps_avg).abs() < f64::EPSILON
        );
    }

    #[test]
    fn test_baseline_save_load_roundtrip() {
        let dir = std::env::temp_dir().join("osim_conform_test_baseline");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_baseline.json");

        let baseline = sample_baseline(Target::CSharp, 45.0, 150.0);
        save_baseline(&path, &baseline).expect("save");
        let loaded = load_baseline(&path).expect("load");

        assert_eq!(loaded.target, Target::CSharp);
        assert!(
            (loaded.metrics.fps_avg - 45.0).abs() < f64::EPSILON
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    // -- comparison logic --

    #[test]
    fn test_comparison_rust_wins_all() {
        // Rust: higher FPS, lower latency => all pass
        let cs = sample_baseline(Target::CSharp, 40.0, 200.0);
        let rs = sample_baseline(Target::Rust, 50.0, 100.0);
        let report = compare_baselines(&cs, &rs);
        assert!(report.overall_pass);
        for c in &report.comparisons {
            assert!(c.pass, "metric {} should pass", c.metric_name);
        }
    }

    #[test]
    fn test_comparison_rust_loses_fps() {
        // Rust has lower FPS => fps_avg and fps_min fail
        let cs = sample_baseline(Target::CSharp, 50.0, 200.0);
        let rs = sample_baseline(Target::Rust, 30.0, 100.0);
        let report = compare_baselines(&cs, &rs);
        assert!(!report.overall_pass);
        let fps_avg = report
            .comparisons
            .iter()
            .find(|c| c.metric_name == "fps_avg")
            .expect("fps_avg");
        assert!(!fps_avg.pass);
    }

    #[test]
    fn test_comparison_equal_values() {
        let cs = sample_baseline(Target::CSharp, 55.0, 100.0);
        let rs = sample_baseline(Target::Rust, 55.0, 100.0);
        let report = compare_baselines(&cs, &rs);
        assert!(report.overall_pass);
        for c in &report.comparisons {
            assert!(
                (c.ratio - 1.0).abs() < f64::EPSILON,
                "metric {} ratio should be 1.0, got {}",
                c.metric_name,
                c.ratio
            );
        }
    }

    #[test]
    fn test_comparison_zero_csharp_higher_is_better() {
        // C# FPS = 0, Rust FPS > 0 => ratio should be infinity (pass)
        let cs = sample_baseline(Target::CSharp, 0.0, 100.0);
        let rs = sample_baseline(Target::Rust, 55.0, 100.0);
        let report = compare_baselines(&cs, &rs);
        let fps_avg = report
            .comparisons
            .iter()
            .find(|c| c.metric_name == "fps_avg")
            .expect("fps_avg");
        assert!(fps_avg.pass);
        assert!(fps_avg.ratio.is_infinite());
    }

    // -- load-test runner --

    #[tokio::test]
    async fn test_runner_rejects_invalid_config() {
        let mut cfg = sample_config();
        cfg.concurrent_avatars = 0;
        assert!(LoadTestRunner::new(cfg).is_err());
    }

    #[tokio::test]
    async fn test_run_load_test_returns_result() {
        let cfg = sample_config();
        let result = run_load_test(cfg).await.expect("run");
        assert!(result.errors.is_empty());
        assert!(result.duration_actual_secs >= 0.0);
    }
}
