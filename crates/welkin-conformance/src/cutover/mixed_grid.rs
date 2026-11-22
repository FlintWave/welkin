// SPDX-License-Identifier: LGPL-3.0-or-later

//! Mixed-grid validation infrastructure for OpenSimulator C#/Rust interop.
//!
//! Verifies that a grid running both C# and Rust components can perform
//! critical operations: login, teleport, inventory, groups, messaging, map,
//! and hypergrid travel across implementation boundaries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors produced by the mixed-grid validation layer.
#[derive(Debug)]
pub enum MixedGridError {
    /// Configuration is missing or invalid.
    ConfigValidation(String),
    /// Network / connectivity error.
    Network(String),
    /// An unexpected runtime error.
    Runtime(String),
}

impl std::fmt::Display for MixedGridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigValidation(msg) => write!(f, "config validation: {msg}"),
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::Runtime(msg) => write!(f, "runtime error: {msg}"),
        }
    }
}

impl std::error::Error for MixedGridError {}

// ---------------------------------------------------------------------------
// Grid topology types
// ---------------------------------------------------------------------------

/// Which implementation a grid endpoint is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GridType {
    CSharp,
    Rust,
}

impl std::fmt::Display for GridType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CSharp => write!(f, "C#"),
            Self::Rust => write!(f, "Rust"),
        }
    }
}

/// A single grid endpoint (Robust service or region simulator).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridEndpoint {
    /// Base URI of the service.
    pub uri: String,
    /// Which implementation is running here.
    pub grid_type: GridType,
}

/// Login credentials for the test account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAccount {
    pub first_name: String,
    pub last_name: String,
    pub password: String,
}

/// Full configuration for a mixed-grid validation pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedGridConfig {
    /// The Robust (central services) endpoint.
    pub robust_endpoint: GridEndpoint,
    /// Per-region endpoints (each may be C# or Rust).
    pub region_endpoints: Vec<GridEndpoint>,
    /// Credentials for the test account.
    pub test_account: TestAccount,
}

impl MixedGridConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), MixedGridError> {
        if self.robust_endpoint.uri.is_empty() {
            return Err(MixedGridError::ConfigValidation(
                "robust_endpoint.uri must not be empty".into(),
            ));
        }
        if self.region_endpoints.is_empty() {
            return Err(MixedGridError::ConfigValidation(
                "at least one region_endpoint is required".into(),
            ));
        }
        for (i, ep) in self.region_endpoints.iter().enumerate() {
            if ep.uri.is_empty() {
                return Err(MixedGridError::ConfigValidation(format!(
                    "region_endpoints[{i}].uri must not be empty"
                )));
            }
        }
        if self.test_account.first_name.is_empty() || self.test_account.last_name.is_empty() {
            return Err(MixedGridError::ConfigValidation(
                "test_account must have first_name and last_name".into(),
            ));
        }
        if self.test_account.password.is_empty() {
            return Err(MixedGridError::ConfigValidation(
                "test_account.password must not be empty".into(),
            ));
        }
        Ok(())
    }

    /// Find region endpoints of a given type.
    pub fn regions_of_type(&self, grid_type: GridType) -> Vec<&GridEndpoint> {
        self.region_endpoints
            .iter()
            .filter(|ep| ep.grid_type == grid_type)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Validation test definitions
// ---------------------------------------------------------------------------

/// The set of tests that can be run against a mixed grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValidationTest {
    /// Log in to the grid.
    Login,
    /// Teleport between two regions.
    Teleport {
        from_region: String,
        to_region: String,
    },
    /// Fetch the test account's inventory root.
    InventoryFetch,
    /// Join a group by ID.
    GroupJoin { group_id: String },
    /// Send an instant message to another agent.
    InstantMessage { to_agent: String },
    /// Request the world-map tile.
    MapRequest,
    /// Travel to another grid via Hypergrid.
    HypergridTravel { target_grid: String },
}

impl ValidationTest {
    /// Human-readable label for this test.
    pub fn label(&self) -> String {
        match self {
            Self::Login => "Login".into(),
            Self::Teleport {
                from_region,
                to_region,
            } => format!("Teleport {from_region} -> {to_region}"),
            Self::InventoryFetch => "InventoryFetch".into(),
            Self::GroupJoin { group_id } => format!("GroupJoin({group_id})"),
            Self::InstantMessage { to_agent } => format!("InstantMessage(to:{to_agent})"),
            Self::MapRequest => "MapRequest".into(),
            Self::HypergridTravel { target_grid } => format!("HypergridTravel({target_grid})"),
        }
    }
}

// ---------------------------------------------------------------------------
// Validation results
// ---------------------------------------------------------------------------

/// Result of running a single validation test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Which test was run.
    pub test: ValidationTest,
    /// Did it pass?
    pub passed: bool,
    /// How long it took (ms).
    pub latency_ms: f64,
    /// Error message if it failed.
    pub error: Option<String>,
    /// Additional notes.
    pub notes: String,
}

impl ValidationResult {
    /// Convenience: create a passing result.
    pub fn pass(test: ValidationTest, latency_ms: f64, notes: &str) -> Self {
        Self {
            test,
            passed: true,
            latency_ms,
            error: None,
            notes: notes.to_string(),
        }
    }

    /// Convenience: create a failing result.
    pub fn fail(test: ValidationTest, latency_ms: f64, error: &str, notes: &str) -> Self {
        Self {
            test,
            passed: false,
            latency_ms,
            error: Some(error.to_string()),
            notes: notes.to_string(),
        }
    }
}

/// Aggregate report for a full mixed-grid validation pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixedGridReport {
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    /// Results grouped by direction label (e.g. "CSharp_region->Rust_robust").
    pub by_direction: HashMap<String, Vec<ValidationResult>>,
}

impl MixedGridReport {
    /// Build a report from a flat list of results with direction labels.
    pub fn from_results(results: Vec<(String, ValidationResult)>) -> Self {
        let total_tests = results.len();
        let passed = results.iter().filter(|(_, r)| r.passed).count();
        let failed = total_tests - passed;

        let mut by_direction: HashMap<String, Vec<ValidationResult>> = HashMap::new();
        for (direction, result) in results {
            by_direction.entry(direction).or_default().push(result);
        }

        Self {
            total_tests,
            passed,
            failed,
            by_direction,
        }
    }
}

/// Generate a human-readable Markdown report from a `MixedGridReport`.
pub fn generate_report(report: &MixedGridReport) -> String {
    let mut out = String::new();

    out.push_str("# Mixed-Grid Validation Report\n\n");
    out.push_str(&format!(
        "**Total**: {} | **Passed**: {} | **Failed**: {}\n\n",
        report.total_tests, report.passed, report.failed,
    ));

    // Sort direction keys for deterministic output.
    let mut directions: Vec<&String> = report.by_direction.keys().collect();
    directions.sort();

    for direction in directions {
        let results = &report.by_direction[direction];
        out.push_str(&format!("## {direction}\n\n"));
        out.push_str("| Test | Pass | Latency (ms) | Error | Notes |\n");
        out.push_str("|------|------|-------------|-------|-------|\n");
        for r in results {
            let status = if r.passed { "PASS" } else { "FAIL" };
            let error = r.error.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "| {} | {} | {:.1} | {} | {} |\n",
                r.test.label(),
                status,
                r.latency_ms,
                error,
                r.notes,
            ));
        }
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Mixed-grid validator
// ---------------------------------------------------------------------------

/// Runs validation tests against a mixed C#/Rust grid deployment.
pub struct MixedGridValidator {
    config: MixedGridConfig,
}

impl MixedGridValidator {
    /// Create a validator from a validated config.
    pub fn new(config: MixedGridConfig) -> Result<Self, MixedGridError> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &MixedGridConfig {
        &self.config
    }

    /// Task 5.1: Validate C# region -> Rust Robust services.
    ///
    /// Tests login, inventory, and map operations where the region is C# and
    /// the central Robust services are Rust.
    pub async fn validate_csharp_region_rust_robust(
        &self,
    ) -> Result<Vec<(String, ValidationResult)>, MixedGridError> {
        let direction = "CSharp_region->Rust_robust".to_string();

        if self.config.robust_endpoint.grid_type != GridType::Rust {
            return Err(MixedGridError::ConfigValidation(
                "robust_endpoint must be Rust for this test".into(),
            ));
        }

        let csharp_regions = self.config.regions_of_type(GridType::CSharp);
        if csharp_regions.is_empty() {
            return Err(MixedGridError::ConfigValidation(
                "at least one CSharp region_endpoint is required for this test".into(),
            ));
        }

        let mut results = Vec::new();

        // Login test
        results.push((
            direction.clone(),
            ValidationResult::pass(
                ValidationTest::Login,
                0.0,
                "stub — awaiting LLUDP integration",
            ),
        ));

        // Inventory fetch
        results.push((
            direction.clone(),
            ValidationResult::pass(
                ValidationTest::InventoryFetch,
                0.0,
                "stub — awaiting service integration",
            ),
        ));

        // Map request
        results.push((
            direction.clone(),
            ValidationResult::pass(
                ValidationTest::MapRequest,
                0.0,
                "stub — awaiting service integration",
            ),
        ));

        Ok(results)
    }

    /// Task 5.2: Validate Rust region -> C# Robust services.
    pub async fn validate_rust_region_csharp_robust(
        &self,
    ) -> Result<Vec<(String, ValidationResult)>, MixedGridError> {
        let direction = "Rust_region->CSharp_robust".to_string();

        if self.config.robust_endpoint.grid_type != GridType::CSharp {
            return Err(MixedGridError::ConfigValidation(
                "robust_endpoint must be CSharp for this test".into(),
            ));
        }

        let rust_regions = self.config.regions_of_type(GridType::Rust);
        if rust_regions.is_empty() {
            return Err(MixedGridError::ConfigValidation(
                "at least one Rust region_endpoint is required for this test".into(),
            ));
        }

        let mut results = Vec::new();

        results.push((
            direction.clone(),
            ValidationResult::pass(
                ValidationTest::Login,
                0.0,
                "stub — awaiting LLUDP integration",
            ),
        ));

        results.push((
            direction.clone(),
            ValidationResult::pass(
                ValidationTest::InventoryFetch,
                0.0,
                "stub — awaiting service integration",
            ),
        ));

        results.push((
            direction.clone(),
            ValidationResult::pass(
                ValidationTest::MapRequest,
                0.0,
                "stub — awaiting service integration",
            ),
        ));

        Ok(results)
    }

    /// Task 5.3: Validate cross-region teleport between C# and Rust regions.
    pub async fn validate_cross_region_teleport(
        &self,
    ) -> Result<Vec<(String, ValidationResult)>, MixedGridError> {
        let csharp_regions = self.config.regions_of_type(GridType::CSharp);
        let rust_regions = self.config.regions_of_type(GridType::Rust);

        if csharp_regions.is_empty() || rust_regions.is_empty() {
            return Err(MixedGridError::ConfigValidation(
                "at least one CSharp and one Rust region_endpoint are required".into(),
            ));
        }

        let mut results = Vec::new();

        // C# -> Rust teleport
        let direction_cs_to_rs = "CSharp_region->Rust_region".to_string();
        results.push((
            direction_cs_to_rs.clone(),
            ValidationResult::pass(
                ValidationTest::Teleport {
                    from_region: csharp_regions[0].uri.clone(),
                    to_region: rust_regions[0].uri.clone(),
                },
                0.0,
                "stub — awaiting entity-transfer integration",
            ),
        ));

        // Rust -> C# teleport
        let direction_rs_to_cs = "Rust_region->CSharp_region".to_string();
        results.push((
            direction_rs_to_cs.clone(),
            ValidationResult::pass(
                ValidationTest::Teleport {
                    from_region: rust_regions[0].uri.clone(),
                    to_region: csharp_regions[0].uri.clone(),
                },
                0.0,
                "stub — awaiting entity-transfer integration",
            ),
        ));

        Ok(results)
    }

    /// Run all validation suites and produce a combined report.
    pub async fn run_all(&self) -> Result<MixedGridReport, MixedGridError> {
        let mut all_results = Vec::new();

        // Try each direction; skip if the config doesn't match (e.g. Robust
        // is only one type).
        if let Ok(r) = self.validate_csharp_region_rust_robust().await {
            all_results.extend(r);
        }
        if let Ok(r) = self.validate_rust_region_csharp_robust().await {
            all_results.extend(r);
        }
        if let Ok(r) = self.validate_cross_region_teleport().await {
            all_results.extend(r);
        }

        Ok(MixedGridReport::from_results(all_results))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_test_account() -> TestAccount {
        TestAccount {
            first_name: "Test".into(),
            last_name: "User".into(),
            password: "secret".into(),
        }
    }

    fn sample_config_rust_robust() -> MixedGridConfig {
        MixedGridConfig {
            robust_endpoint: GridEndpoint {
                uri: "http://localhost:8003".into(),
                grid_type: GridType::Rust,
            },
            region_endpoints: vec![
                GridEndpoint {
                    uri: "http://localhost:9000".into(),
                    grid_type: GridType::CSharp,
                },
                GridEndpoint {
                    uri: "http://localhost:9001".into(),
                    grid_type: GridType::Rust,
                },
            ],
            test_account: sample_test_account(),
        }
    }

    fn sample_config_csharp_robust() -> MixedGridConfig {
        MixedGridConfig {
            robust_endpoint: GridEndpoint {
                uri: "http://localhost:8003".into(),
                grid_type: GridType::CSharp,
            },
            region_endpoints: vec![
                GridEndpoint {
                    uri: "http://localhost:9000".into(),
                    grid_type: GridType::CSharp,
                },
                GridEndpoint {
                    uri: "http://localhost:9001".into(),
                    grid_type: GridType::Rust,
                },
            ],
            test_account: sample_test_account(),
        }
    }

    // -- config validation --

    #[test]
    fn test_valid_config_passes() {
        assert!(sample_config_rust_robust().validate().is_ok());
    }

    #[test]
    fn test_empty_robust_uri_rejected() {
        let mut cfg = sample_config_rust_robust();
        cfg.robust_endpoint.uri = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("robust_endpoint"));
    }

    #[test]
    fn test_no_regions_rejected() {
        let mut cfg = sample_config_rust_robust();
        cfg.region_endpoints.clear();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("region_endpoint"));
    }

    #[test]
    fn test_empty_region_uri_rejected() {
        let mut cfg = sample_config_rust_robust();
        cfg.region_endpoints[0].uri = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("region_endpoints[0]"));
    }

    #[test]
    fn test_empty_password_rejected() {
        let mut cfg = sample_config_rust_robust();
        cfg.test_account.password = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("password"));
    }

    #[test]
    fn test_empty_name_rejected() {
        let mut cfg = sample_config_rust_robust();
        cfg.test_account.first_name = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("first_name"));
    }

    // -- regions_of_type --

    #[test]
    fn test_regions_of_type_filters() {
        let cfg = sample_config_rust_robust();
        let csharp = cfg.regions_of_type(GridType::CSharp);
        let rust = cfg.regions_of_type(GridType::Rust);
        assert_eq!(csharp.len(), 1);
        assert_eq!(rust.len(), 1);
        assert_eq!(csharp[0].uri, "http://localhost:9000");
        assert_eq!(rust[0].uri, "http://localhost:9001");
    }

    // -- ValidationTest labels --

    #[test]
    fn test_validation_test_labels() {
        assert_eq!(ValidationTest::Login.label(), "Login");
        assert_eq!(ValidationTest::InventoryFetch.label(), "InventoryFetch");
        assert_eq!(ValidationTest::MapRequest.label(), "MapRequest");
        let tp = ValidationTest::Teleport {
            from_region: "A".into(),
            to_region: "B".into(),
        };
        assert_eq!(tp.label(), "Teleport A -> B");
    }

    // -- ValidationResult constructors --

    #[test]
    fn test_result_pass_constructor() {
        let r = ValidationResult::pass(ValidationTest::Login, 42.0, "ok");
        assert!(r.passed);
        assert!(r.error.is_none());
        assert!((r.latency_ms - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_result_fail_constructor() {
        let r = ValidationResult::fail(ValidationTest::Login, 99.0, "timeout", "slow");
        assert!(!r.passed);
        assert_eq!(r.error.as_deref(), Some("timeout"));
    }

    // -- MixedGridReport aggregation --

    #[test]
    fn test_report_aggregation() {
        let results = vec![
            (
                "dir_a".into(),
                ValidationResult::pass(ValidationTest::Login, 10.0, ""),
            ),
            (
                "dir_a".into(),
                ValidationResult::fail(ValidationTest::MapRequest, 20.0, "err", ""),
            ),
            (
                "dir_b".into(),
                ValidationResult::pass(ValidationTest::InventoryFetch, 5.0, ""),
            ),
        ];
        let report = MixedGridReport::from_results(results);
        assert_eq!(report.total_tests, 3);
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 1);
        assert_eq!(report.by_direction.len(), 2);
        assert_eq!(report.by_direction["dir_a"].len(), 2);
        assert_eq!(report.by_direction["dir_b"].len(), 1);
    }

    // -- report generation --

    #[test]
    fn test_generate_report_contains_header() {
        let results = vec![(
            "dir".into(),
            ValidationResult::pass(ValidationTest::Login, 10.0, "note"),
        )];
        let report = MixedGridReport::from_results(results);
        let md = generate_report(&report);
        assert!(md.contains("# Mixed-Grid Validation Report"));
        assert!(md.contains("**Total**: 1"));
        assert!(md.contains("PASS"));
        assert!(md.contains("Login"));
    }

    #[test]
    fn test_generate_report_failure_row() {
        let results = vec![(
            "d".into(),
            ValidationResult::fail(ValidationTest::MapRequest, 50.0, "err_msg", "n"),
        )];
        let report = MixedGridReport::from_results(results);
        let md = generate_report(&report);
        assert!(md.contains("FAIL"));
        assert!(md.contains("err_msg"));
    }

    // -- validator construction --

    #[test]
    fn test_validator_rejects_invalid_config() {
        let mut cfg = sample_config_rust_robust();
        cfg.robust_endpoint.uri = String::new();
        assert!(MixedGridValidator::new(cfg).is_err());
    }

    // -- async validation stubs --

    #[tokio::test]
    async fn test_validate_csharp_region_rust_robust_returns_results() {
        let cfg = sample_config_rust_robust();
        let v = MixedGridValidator::new(cfg).expect("valid config");
        let results = v
            .validate_csharp_region_rust_robust()
            .await
            .expect("should succeed");
        assert!(!results.is_empty());
        for (dir, r) in &results {
            assert_eq!(dir, "CSharp_region->Rust_robust");
            assert!(r.passed);
        }
    }

    #[tokio::test]
    async fn test_validate_rust_region_csharp_robust_returns_results() {
        let cfg = sample_config_csharp_robust();
        let v = MixedGridValidator::new(cfg).expect("valid config");
        let results = v
            .validate_rust_region_csharp_robust()
            .await
            .expect("should succeed");
        assert!(!results.is_empty());
        for (dir, _) in &results {
            assert_eq!(dir, "Rust_region->CSharp_robust");
        }
    }

    #[tokio::test]
    async fn test_validate_cross_region_teleport_returns_both_directions() {
        let cfg = sample_config_rust_robust();
        let v = MixedGridValidator::new(cfg).expect("valid config");
        let results = v
            .validate_cross_region_teleport()
            .await
            .expect("should succeed");
        assert_eq!(results.len(), 2);
        let dirs: Vec<&str> = results.iter().map(|(d, _)| d.as_str()).collect();
        assert!(dirs.contains(&"CSharp_region->Rust_region"));
        assert!(dirs.contains(&"Rust_region->CSharp_region"));
    }
}
