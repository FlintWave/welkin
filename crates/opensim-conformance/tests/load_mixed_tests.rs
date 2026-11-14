// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the load-test and mixed-grid validation modules.
//!
//! These live in a separate integration test file so they exercise the public
//! API surface of the `cutover` module.

// NOTE: These modules are compiled here for test purposes. Once main.rs adds
// `pub mod cutover;`, the integration tests can use `opensim_conformance::cutover::*`
// instead of the path-include approach below.

#[path = "../src/load_test.rs"]
mod load_test;
#[path = "../src/mixed_grid.rs"]
mod mixed_grid;

use load_test::*;
use mixed_grid::*;

// ---------------------------------------------------------------------------
// Load-test integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_load_config_validates_required_fields() {
    // Valid config should pass.
    let cfg = LoadTestConfig {
        concurrent_avatars: 5,
        physics_objects: 10,
        active_scripts: 20,
        asset_transfers_per_region: 3,
        duration_secs: 30,
        target_uri: "http://localhost:9000".into(),
        regions: vec!["Region1".into()],
    };
    assert!(cfg.validate().is_ok());

    // Zero avatars.
    let mut bad = cfg.clone();
    bad.concurrent_avatars = 0;
    assert!(bad.validate().is_err());

    // Zero duration.
    let mut bad = cfg.clone();
    bad.duration_secs = 0;
    assert!(bad.validate().is_err());

    // Empty URI.
    let mut bad = cfg.clone();
    bad.target_uri = String::new();
    assert!(bad.validate().is_err());

    // No regions.
    let mut bad = cfg.clone();
    bad.regions.clear();
    assert!(bad.validate().is_err());
}

#[test]
fn test_baseline_save_load_roundtrip() {
    let dir = std::env::temp_dir().join("osim_conform_integ_baseline");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("integ_test.json");

    let cfg = LoadTestConfig {
        concurrent_avatars: 2,
        physics_objects: 5,
        active_scripts: 10,
        asset_transfers_per_region: 1,
        duration_secs: 10,
        target_uri: "http://example.com".into(),
        regions: vec!["R".into()],
    };

    let baseline = BaselineMeasurement {
        target: Target::Rust,
        timestamp: "2026-11-14T12:00:00Z".into(),
        hardware_description: "integration test hw".into(),
        config: cfg,
        metrics: LoadMetrics {
            fps_avg: 55.0,
            fps_min: 44.0,
            script_eps: 1200.0,
            memory_mb: 480.0,
            network_bandwidth_mbps: 8.5,
            region_crossing_latency_ms: 120.0,
            asset_fetch_latency_ms: 60.0,
            login_latency_ms: 250.0,
        },
    };

    save_baseline(&path, &baseline).expect("save baseline");
    let loaded = load_baseline(&path).expect("load baseline");

    assert_eq!(loaded.target, Target::Rust);
    assert!((loaded.metrics.fps_avg - 55.0).abs() < f64::EPSILON);
    assert!((loaded.metrics.login_latency_ms - 250.0).abs() < f64::EPSILON);

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn test_comparison_report_mixed_metrics() {
    let cfg = LoadTestConfig {
        concurrent_avatars: 1,
        physics_objects: 1,
        active_scripts: 1,
        asset_transfers_per_region: 1,
        duration_secs: 1,
        target_uri: "http://x".into(),
        regions: vec!["R".into()],
    };

    let cs = BaselineMeasurement {
        target: Target::CSharp,
        timestamp: "t0".into(),
        hardware_description: "hw".into(),
        config: cfg.clone(),
        metrics: LoadMetrics {
            fps_avg: 50.0,
            fps_min: 40.0,
            script_eps: 800.0,
            memory_mb: 600.0,       // lower is better; C# uses 600
            network_bandwidth_mbps: 12.0, // lower is better; C# uses 12
            region_crossing_latency_ms: 150.0, // lower is better; C# 150
            asset_fetch_latency_ms: 80.0,
            login_latency_ms: 300.0,
        },
    };

    // Rust: better FPS, better latency, but worse memory
    let rs = BaselineMeasurement {
        target: Target::Rust,
        timestamp: "t0".into(),
        hardware_description: "hw".into(),
        config: cfg,
        metrics: LoadMetrics {
            fps_avg: 60.0,       // better
            fps_min: 48.0,       // better
            script_eps: 1000.0,  // better
            memory_mb: 700.0,    // WORSE (higher)
            network_bandwidth_mbps: 10.0, // better
            region_crossing_latency_ms: 100.0, // better
            asset_fetch_latency_ms: 50.0, // better
            login_latency_ms: 200.0, // better
        },
    };

    let report = compare_baselines(&cs, &rs);

    // Memory should fail (Rust uses more)
    let mem = report
        .comparisons
        .iter()
        .find(|c| c.metric_name == "memory_mb")
        .expect("memory_mb");
    assert!(!mem.pass, "Rust uses more memory, should fail");

    // FPS should pass
    let fps = report
        .comparisons
        .iter()
        .find(|c| c.metric_name == "fps_avg")
        .expect("fps_avg");
    assert!(fps.pass);

    // Overall should fail because memory failed
    assert!(!report.overall_pass);
}

// ---------------------------------------------------------------------------
// Mixed-grid integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_mixed_grid_config_construction() {
    let cfg = MixedGridConfig {
        robust_endpoint: GridEndpoint {
            uri: "http://robust:8003".into(),
            grid_type: GridType::Rust,
        },
        region_endpoints: vec![
            GridEndpoint {
                uri: "http://region-cs:9000".into(),
                grid_type: GridType::CSharp,
            },
            GridEndpoint {
                uri: "http://region-rs:9001".into(),
                grid_type: GridType::Rust,
            },
        ],
        test_account: TestAccount {
            first_name: "Integ".into(),
            last_name: "Test".into(),
            password: "pass".into(),
        },
    };

    assert!(cfg.validate().is_ok());
    assert_eq!(cfg.regions_of_type(GridType::CSharp).len(), 1);
    assert_eq!(cfg.regions_of_type(GridType::Rust).len(), 1);
}

#[test]
fn test_validation_result_aggregation() {
    let results = vec![
        (
            "dir_a".into(),
            ValidationResult::pass(ValidationTest::Login, 10.0, "ok"),
        ),
        (
            "dir_a".into(),
            ValidationResult::fail(
                ValidationTest::InventoryFetch,
                50.0,
                "timeout",
                "slow",
            ),
        ),
        (
            "dir_b".into(),
            ValidationResult::pass(
                ValidationTest::Teleport {
                    from_region: "A".into(),
                    to_region: "B".into(),
                },
                30.0,
                "ok",
            ),
        ),
    ];

    let report = MixedGridReport::from_results(results);
    assert_eq!(report.total_tests, 3);
    assert_eq!(report.passed, 2);
    assert_eq!(report.failed, 1);

    let md = generate_report(&report);
    assert!(md.contains("FAIL"));
    assert!(md.contains("timeout"));
    assert!(md.contains("dir_a"));
    assert!(md.contains("dir_b"));
}

#[test]
fn test_mixed_grid_report_empty() {
    let report = MixedGridReport::from_results(vec![]);
    assert_eq!(report.total_tests, 0);
    assert_eq!(report.passed, 0);
    assert_eq!(report.failed, 0);
    let md = generate_report(&report);
    assert!(md.contains("**Total**: 0"));
}
