// SPDX-License-Identifier: GPL-3.0-or-later

//! Service-connector conformance testing.
//!
//! Validates that a Robust service grid responds correctly by sending
//! known request XML and checking the response against expected patterns.
//! Each [`ServiceTestCase`] targets a specific [`ServiceEndpoint`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Service endpoints
// ---------------------------------------------------------------------------

/// The OpenSimulator service endpoints that can be tested.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ServiceEndpoint {
    Login,
    Grid,
    Asset,
    Inventory,
    UserAccount,
    Presence,
    Authentication,
    Friends,
    Groups,
    OfflineIM,
    Map,
    Hypergrid,
}

impl ServiceEndpoint {
    /// Return the conventional URL path suffix for this endpoint.
    pub fn path(&self) -> &'static str {
        match self {
            Self::Login => "/",
            Self::Grid => "/grid",
            Self::Asset => "/assets",
            Self::Inventory => "/xinventory",
            Self::UserAccount => "/accounts",
            Self::Presence => "/presence",
            Self::Authentication => "/auth",
            Self::Friends => "/friends",
            Self::Groups => "/groups",
            Self::OfflineIM => "/offline",
            Self::Map => "/map",
            Self::Hypergrid => "/hypergrid",
        }
    }

    /// All endpoint variants.
    pub fn all() -> &'static [ServiceEndpoint] {
        &[
            Self::Login,
            Self::Grid,
            Self::Asset,
            Self::Inventory,
            Self::UserAccount,
            Self::Presence,
            Self::Authentication,
            Self::Friends,
            Self::Groups,
            Self::OfflineIM,
            Self::Map,
            Self::Hypergrid,
        ]
    }
}

// ---------------------------------------------------------------------------
// Test cases
// ---------------------------------------------------------------------------

/// A single test case for a service endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTestCase {
    pub name: String,
    pub endpoint: ServiceEndpoint,
    pub request_xml: String,
    /// A substring or simple pattern expected in the response body.
    pub expected_response_pattern: String,
}

/// A failure recorded during a conformance run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceFailure {
    pub endpoint: ServiceEndpoint,
    pub test_name: String,
    pub reason: String,
}

/// Summary report from a conformance run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub failures: Vec<ConformanceFailure>,
}

// ---------------------------------------------------------------------------
// Built-in test cases
// ---------------------------------------------------------------------------

/// Return the built-in suite of test cases covering the core service endpoints.
pub fn builtin_test_cases() -> Vec<ServiceTestCase> {
    vec![
        // Login (XMLRPC login_to_simulator)
        ServiceTestCase {
            name: "login_to_simulator".to_string(),
            endpoint: ServiceEndpoint::Login,
            request_xml: r#"<?xml version="1.0"?>
<methodCall>
  <methodName>login_to_simulator</methodName>
  <params><param><value><struct>
    <member><name>first</name><value><string>Test</string></value></member>
    <member><name>last</name><value><string>User</string></value></member>
    <member><name>passwd</name><value><string>$1$test</string></value></member>
    <member><name>start</name><value><string>last</string></value></member>
    <member><name>channel</name><value><string>conformance</string></value></member>
    <member><name>version</name><value><string>0.1</string></value></member>
    <member><name>platform</name><value><string>Lin</string></value></member>
  </struct></value></param></params>
</methodCall>"#
                .to_string(),
            expected_response_pattern: "methodResponse".to_string(),
        },
        // Grid: register
        ServiceTestCase {
            name: "grid_register".to_string(),
            endpoint: ServiceEndpoint::Grid,
            request_xml: r#"<?xml version="1.0" encoding="utf-8"?>
<ServerRequest>
  <RequestMethod>register</RequestMethod>
  <RegionID>00000000-0000-0000-0000-000000000001</RegionID>
  <RegionName>TestRegion</RegionName>
  <ExternalHostName>127.0.0.1</ExternalHostName>
  <HttpPort>9000</HttpPort>
  <InternalEndPoint>0.0.0.0:9000</InternalEndPoint>
  <ScopeID>00000000-0000-0000-0000-000000000000</ScopeID>
</ServerRequest>"#
                .to_string(),
            expected_response_pattern: "Result".to_string(),
        },
        // Asset: get
        ServiceTestCase {
            name: "asset_get".to_string(),
            endpoint: ServiceEndpoint::Asset,
            request_xml: r#"<?xml version="1.0" encoding="utf-8"?>
<ServerRequest>
  <RequestMethod>get</RequestMethod>
  <ID>00000000-0000-0000-0000-000000000000</ID>
</ServerRequest>"#
                .to_string(),
            expected_response_pattern: "ServerResponse".to_string(),
        },
        // Inventory: fetch
        ServiceTestCase {
            name: "inventory_fetch".to_string(),
            endpoint: ServiceEndpoint::Inventory,
            request_xml: r#"<?xml version="1.0" encoding="utf-8"?>
<ServerRequest>
  <RequestMethod>GETFOLDER</RequestMethod>
  <FolderID>00000000-0000-0000-0000-000000000000</FolderID>
  <PRINCIPAL>00000000-0000-0000-0000-000000000001</PRINCIPAL>
</ServerRequest>"#
                .to_string(),
            expected_response_pattern: "ServerResponse".to_string(),
        },
        // UserAccount: get
        ServiceTestCase {
            name: "useraccount_get".to_string(),
            endpoint: ServiceEndpoint::UserAccount,
            request_xml: r#"<?xml version="1.0" encoding="utf-8"?>
<ServerRequest>
  <RequestMethod>getaccount</RequestMethod>
  <ScopeID>00000000-0000-0000-0000-000000000000</ScopeID>
  <FirstName>Test</FirstName>
  <LastName>User</LastName>
</ServerRequest>"#
                .to_string(),
            expected_response_pattern: "Result".to_string(),
        },
        // Presence: report
        ServiceTestCase {
            name: "presence_report".to_string(),
            endpoint: ServiceEndpoint::Presence,
            request_xml: r#"<?xml version="1.0" encoding="utf-8"?>
<ServerRequest>
  <RequestMethod>report</RequestMethod>
  <SessionID>00000000-0000-0000-0000-000000000001</SessionID>
  <RegionID>00000000-0000-0000-0000-000000000002</RegionID>
</ServerRequest>"#
                .to_string(),
            expected_response_pattern: "Result".to_string(),
        },
        // Authentication: authenticate
        ServiceTestCase {
            name: "authentication_authenticate".to_string(),
            endpoint: ServiceEndpoint::Authentication,
            request_xml: r#"<?xml version="1.0" encoding="utf-8"?>
<ServerRequest>
  <RequestMethod>authenticate</RequestMethod>
  <PRINCIPAL>00000000-0000-0000-0000-000000000001</PRINCIPAL>
  <PASSWORD>test</PASSWORD>
  <LIFETIME>30</LIFETIME>
</ServerRequest>"#
                .to_string(),
            expected_response_pattern: "Result".to_string(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Conformance runner
// ---------------------------------------------------------------------------

/// Runner that sends test cases to a target Robust server and validates
/// responses.
pub struct ConformanceRunner {
    pub target: String,
    pub test_cases: Vec<ServiceTestCase>,
}

impl ConformanceRunner {
    /// Create a runner targeting the given base URL with the built-in suite.
    pub fn new(target: &str) -> Self {
        Self {
            target: target.trim_end_matches('/').to_string(),
            test_cases: builtin_test_cases(),
        }
    }

    /// Create a runner with custom test cases.
    pub fn with_cases(target: &str, cases: Vec<ServiceTestCase>) -> Self {
        Self {
            target: target.trim_end_matches('/').to_string(),
            test_cases: cases,
        }
    }

    /// Validate a response body against a test case's expected pattern.
    pub fn validate_response(test: &ServiceTestCase, response_body: &str) -> bool {
        response_body.contains(&test.expected_response_pattern)
    }

    /// Run the conformance suite in offline mode — the caller provides
    /// pre-collected response bodies keyed by test-case name.
    pub fn run_offline(&self, responses: &BTreeMap<String, (u16, String)>) -> ConformanceReport {
        let mut report = ConformanceReport {
            total: self.test_cases.len(),
            passed: 0,
            failed: 0,
            skipped: 0,
            failures: Vec::new(),
        };

        for test in &self.test_cases {
            match responses.get(&test.name) {
                Some((status, body)) => {
                    if *status >= 400 {
                        report.failed += 1;
                        report.failures.push(ConformanceFailure {
                            endpoint: test.endpoint,
                            test_name: test.name.clone(),
                            reason: format!("HTTP status {status}"),
                        });
                    } else if Self::validate_response(test, body) {
                        report.passed += 1;
                    } else {
                        report.failed += 1;
                        report.failures.push(ConformanceFailure {
                            endpoint: test.endpoint,
                            test_name: test.name.clone(),
                            reason: format!(
                                "response does not contain expected pattern '{}'",
                                test.expected_response_pattern
                            ),
                        });
                    }
                }
                None => {
                    report.skipped += 1;
                }
            }
        }

        report
    }
}

/// Run the full conformance suite from the CLI.  In this offline-capable
/// implementation, if `recordings_path` is provided it loads pre-recorded
/// exchanges; otherwise it reports the suite as skipped.
pub fn run_conformance_suite(target: &str, recordings_path: Option<&Path>) -> ConformanceReport {
    let runner = ConformanceRunner::new(target);

    if let Some(path) = recordings_path {
        // Try to load pre-recorded responses
        let responses = load_recorded_responses(path);
        info!(
            "Loaded {} recorded responses from {}",
            responses.len(),
            path.display()
        );
        runner.run_offline(&responses)
    } else {
        info!("No recordings path provided — all tests skipped");
        ConformanceReport {
            total: runner.test_cases.len(),
            passed: 0,
            failed: 0,
            skipped: runner.test_cases.len(),
            failures: Vec::new(),
        }
    }
}

/// Load pre-recorded responses from JSON files in a directory.
/// Each file should have `{ "name": "...", "status": 200, "body": "..." }`.
fn load_recorded_responses(dir: &Path) -> BTreeMap<String, (u16, String)> {
    let mut map = BTreeMap::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("Cannot read recordings directory {}: {e}", dir.display());
            return map;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
            let name = v["name"].as_str().unwrap_or("").to_string();
            let status = v["status"].as_u64().unwrap_or(200) as u16;
            let body = v["body"].as_str().unwrap_or("").to_string();
            if !name.is_empty() {
                map.insert(name, (status, body));
            }
        }
    }

    map
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_endpoint_all() {
        let all = ServiceEndpoint::all();
        assert_eq!(all.len(), 12);
    }

    #[test]
    fn test_service_endpoint_paths() {
        assert_eq!(ServiceEndpoint::Login.path(), "/");
        assert_eq!(ServiceEndpoint::Grid.path(), "/grid");
        assert_eq!(ServiceEndpoint::Asset.path(), "/assets");
        assert_eq!(ServiceEndpoint::Inventory.path(), "/xinventory");
    }

    #[test]
    fn test_builtin_test_cases_not_empty() {
        let cases = builtin_test_cases();
        assert!(cases.len() >= 4);
        // Check that we have the four required endpoints
        let endpoints: Vec<ServiceEndpoint> = cases.iter().map(|c| c.endpoint).collect();
        assert!(endpoints.contains(&ServiceEndpoint::Login));
        assert!(endpoints.contains(&ServiceEndpoint::Grid));
        assert!(endpoints.contains(&ServiceEndpoint::Asset));
        assert!(endpoints.contains(&ServiceEndpoint::Inventory));
    }

    #[test]
    fn test_validate_response_match() {
        let test = ServiceTestCase {
            name: "test".to_string(),
            endpoint: ServiceEndpoint::Login,
            request_xml: String::new(),
            expected_response_pattern: "methodResponse".to_string(),
        };
        assert!(ConformanceRunner::validate_response(
            &test,
            "<methodResponse><params></params></methodResponse>"
        ));
    }

    #[test]
    fn test_validate_response_no_match() {
        let test = ServiceTestCase {
            name: "test".to_string(),
            endpoint: ServiceEndpoint::Login,
            request_xml: String::new(),
            expected_response_pattern: "methodResponse".to_string(),
        };
        assert!(!ConformanceRunner::validate_response(
            &test,
            "<html>404 not found</html>"
        ));
    }

    #[test]
    fn test_run_offline_all_pass() {
        let cases = vec![ServiceTestCase {
            name: "login_to_simulator".to_string(),
            endpoint: ServiceEndpoint::Login,
            request_xml: String::new(),
            expected_response_pattern: "methodResponse".to_string(),
        }];
        let runner = ConformanceRunner::with_cases("http://localhost:8002", cases);

        let mut responses = BTreeMap::new();
        responses.insert(
            "login_to_simulator".to_string(),
            (200, "<methodResponse>ok</methodResponse>".to_string()),
        );

        let report = runner.run_offline(&responses);
        assert_eq!(report.total, 1);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn test_run_offline_failure() {
        let cases = vec![ServiceTestCase {
            name: "grid_register".to_string(),
            endpoint: ServiceEndpoint::Grid,
            request_xml: String::new(),
            expected_response_pattern: "Result".to_string(),
        }];
        let runner = ConformanceRunner::with_cases("http://localhost:8002", cases);

        let mut responses = BTreeMap::new();
        responses.insert(
            "grid_register".to_string(),
            (200, "<error>bad request</error>".to_string()),
        );

        let report = runner.run_offline(&responses);
        assert_eq!(report.failed, 1);
        assert_eq!(report.failures.len(), 1);
        assert!(report.failures[0].reason.contains("Result"));
    }

    #[test]
    fn test_run_offline_http_error() {
        let cases = vec![ServiceTestCase {
            name: "asset_get".to_string(),
            endpoint: ServiceEndpoint::Asset,
            request_xml: String::new(),
            expected_response_pattern: "ServerResponse".to_string(),
        }];
        let runner = ConformanceRunner::with_cases("http://localhost:8002", cases);

        let mut responses = BTreeMap::new();
        responses.insert(
            "asset_get".to_string(),
            (500, "Internal Server Error".to_string()),
        );

        let report = runner.run_offline(&responses);
        assert_eq!(report.failed, 1);
        assert!(report.failures[0].reason.contains("500"));
    }

    #[test]
    fn test_run_offline_skipped() {
        let cases = vec![ServiceTestCase {
            name: "missing_test".to_string(),
            endpoint: ServiceEndpoint::Presence,
            request_xml: String::new(),
            expected_response_pattern: "ok".to_string(),
        }];
        let runner = ConformanceRunner::with_cases("http://localhost:8002", cases);
        let responses = BTreeMap::new();
        let report = runner.run_offline(&responses);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn test_service_endpoint_serde_roundtrip() {
        let ep = ServiceEndpoint::Hypergrid;
        let json = serde_json::to_string(&ep).expect("serialize");
        let restored: ServiceEndpoint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, ep);
    }

    #[test]
    fn test_test_case_serde_roundtrip() {
        let tc = ServiceTestCase {
            name: "round".to_string(),
            endpoint: ServiceEndpoint::Friends,
            request_xml: "<test/>".to_string(),
            expected_response_pattern: "ok".to_string(),
        };
        let json = serde_json::to_string(&tc).expect("serialize");
        let restored: ServiceTestCase = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.name, "round");
        assert_eq!(restored.endpoint, ServiceEndpoint::Friends);
    }

    #[test]
    fn test_conformance_report_serde() {
        let report = ConformanceReport {
            total: 10,
            passed: 8,
            failed: 1,
            skipped: 1,
            failures: vec![ConformanceFailure {
                endpoint: ServiceEndpoint::Grid,
                test_name: "grid_register".to_string(),
                reason: "mismatch".to_string(),
            }],
        };
        let json = serde_json::to_string(&report).expect("serialize");
        let restored: ConformanceReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.total, 10);
        assert_eq!(restored.failures.len(), 1);
    }

    #[test]
    fn test_run_conformance_suite_no_recordings() {
        let report = run_conformance_suite("http://localhost:8002", None);
        assert!(report.skipped > 0);
        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 0);
    }
}
