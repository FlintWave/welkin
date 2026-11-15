// SPDX-License-Identifier: LGPL-3.0-or-later

//! Pre-flight wizard checks for connecting a region to an external grid.
//!
//! Before a region registers with a grid it does not own, the operator should
//! run these checks to catch problems early.  Each check uses trait objects so
//! the wizard works with both local (in-process) and remote (HTTP) service
//! implementations.
//!
//! The checks mirror the C# grid service's registration validation
//! (`OpenSim/Services/GridService/GridService.cs`), plus additional
//! reachability tests that the C# code leaves to the operator.

use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::Duration;

use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during wizard checks.
#[derive(Debug, Error)]
pub enum WizardError {
    #[error("region name is invalid: {0}")]
    InvalidName(String),
    #[error("region name '{0}' is already taken on the grid")]
    DuplicateName(String),
    #[error("coordinates ({0}, {1}) overlap an existing region")]
    CoordinatesTaken(u32, u32),
    #[error("estate owner {0} not found on the grid")]
    OwnerNotFound(Uuid),
    #[error("grid query failed: {0}")]
    GridError(String),
    #[error("account query failed: {0}")]
    AccountError(String),
    #[error("port check failed: {0}")]
    PortError(String),
}

// ---------------------------------------------------------------------------
// Service traits (trait objects for local / remote)
// ---------------------------------------------------------------------------

/// Minimal grid-service interface the wizard needs.
///
/// Implementors wrap either a local `GridStore` or a `RemoteGridConnector`.
/// Methods return boxed futures so the trait is dyn-compatible.
pub trait GridConnector: Send + Sync {
    /// Check whether a region with the given name exists on the grid.
    /// Returns `Ok(true)` when the name is already taken.
    fn region_name_exists(
        &self,
        name: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool, WizardError>> + Send + '_>>;

    /// Check whether the grid position at `(x, y)` (in metres) is occupied.
    /// Returns `Ok(true)` when the location is taken.
    fn position_occupied(
        &self,
        x: u32,
        y: u32,
    ) -> Pin<Box<dyn Future<Output = Result<bool, WizardError>> + Send + '_>>;
}

/// Minimal account-service interface the wizard needs.
/// Methods return boxed futures so the trait is dyn-compatible.
pub trait AccountConnector: Send + Sync {
    /// Return `Ok(true)` when an account with the given UUID exists.
    fn account_exists(
        &self,
        owner_id: &Uuid,
    ) -> Pin<Box<dyn Future<Output = Result<bool, WizardError>> + Send + '_>>;
}

// ---------------------------------------------------------------------------
// Port-check result
// ---------------------------------------------------------------------------

/// Result of a single port-reachability test.
#[derive(Debug, Clone)]
pub struct PortCheckResult {
    pub port: u16,
    pub protocol: Protocol,
    pub reachable: bool,
    pub detail: String,
}

/// Transport protocol tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "TCP"),
            Protocol::Udp => write!(f, "UDP"),
        }
    }
}

// ---------------------------------------------------------------------------
// Wizard report
// ---------------------------------------------------------------------------

/// Outcome of a single wizard check.
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub check_name: String,
    pub passed: bool,
    pub message: String,
}

/// Aggregated report from `run_all_checks`.
#[derive(Debug, Clone)]
pub struct WizardReport {
    pub outcomes: Vec<CheckOutcome>,
    pub port_results: Vec<PortCheckResult>,
}

impl WizardReport {
    /// `true` when every check passed.
    pub fn all_passed(&self) -> bool {
        self.outcomes.iter().all(|o| o.passed)
    }
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

/// Validate the region name format and uniqueness on the grid.
///
/// Local rules (checked without a network call):
/// - Must not be empty.
/// - Must not exceed 128 bytes (the C# schema limit).
/// - Must not contain only whitespace.
///
/// Remote rule:
/// - Must not duplicate an existing region name on the grid.
pub async fn check_region_name(
    name: &str,
    grid_connector: &(dyn GridConnector + '_),
) -> Result<(), WizardError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(WizardError::InvalidName("name is empty".into()));
    }
    if name.len() > 128 {
        return Err(WizardError::InvalidName(
            "name exceeds 128-byte limit".into(),
        ));
    }
    // The C# grid service rejects duplicate names when AllowDuplicateNames
    // is false (the default on public grids like OSgrid).
    if grid_connector.region_name_exists(name).await? {
        return Err(WizardError::DuplicateName(name.to_string()));
    }
    Ok(())
}

/// Verify that the requested map coordinates are free on the grid.
///
/// Coordinates are in OpenSimulator's global metre system (multiples of 256).
pub async fn check_coordinates(
    x: u32,
    y: u32,
    grid_connector: &(dyn GridConnector + '_),
) -> Result<(), WizardError> {
    if grid_connector.position_occupied(x, y).await? {
        return Err(WizardError::CoordinatesTaken(x, y));
    }
    Ok(())
}

/// Verify that the estate owner's account exists on the grid.
pub async fn check_estate_owner(
    owner_id: &Uuid,
    account_connector: &(dyn AccountConnector + '_),
) -> Result<(), WizardError> {
    if !account_connector.account_exists(owner_id).await? {
        return Err(WizardError::OwnerNotFound(*owner_id));
    }
    Ok(())
}

/// Verify that the region's ports are reachable from the local host.
///
/// This is a self-test: it binds to the internal port to confirm the OS
/// allows it, and optionally attempts a loopback connection.  It cannot
/// verify external reachability without an external probe service.
///
/// `external_host` is provided for the report but is not contacted (a
/// future version may use an echo service).
pub async fn check_ports(
    internal_port: u16,
    _external_host: &str,
) -> Result<Vec<PortCheckResult>, WizardError> {
    let mut results = Vec::new();

    // --- TCP bind test ---
    let tcp_result = match tokio::net::TcpListener::bind(SocketAddr::from((
        [0, 0, 0, 0],
        internal_port,
    )))
    .await
    {
        Ok(listener) => {
            drop(listener);
            PortCheckResult {
                port: internal_port,
                protocol: Protocol::Tcp,
                reachable: true,
                detail: "TCP port is available for binding".into(),
            }
        }
        Err(e) => PortCheckResult {
            port: internal_port,
            protocol: Protocol::Tcp,
            reachable: false,
            detail: format!("TCP bind failed: {e}"),
        },
    };
    results.push(tcp_result);

    // --- UDP bind test ---
    let udp_result =
        match tokio::net::UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], internal_port))).await {
            Ok(socket) => {
                drop(socket);
                PortCheckResult {
                    port: internal_port,
                    protocol: Protocol::Udp,
                    reachable: true,
                    detail: "UDP port is available for binding".into(),
                }
            }
            Err(e) => PortCheckResult {
                port: internal_port,
                protocol: Protocol::Udp,
                reachable: false,
                detail: format!("UDP bind failed: {e}"),
            },
        };
    results.push(udp_result);

    // --- TCP loopback connect test (quick sanity) ---
    // Bind, then try to connect within a timeout.
    match tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], internal_port))).await {
        Ok(listener) => {
            let addr = listener.local_addr().unwrap();
            let connect = tokio::time::timeout(
                Duration::from_millis(500),
                tokio::net::TcpStream::connect(addr),
            );
            match connect.await {
                Ok(Ok(_stream)) => {
                    results.push(PortCheckResult {
                        port: internal_port,
                        protocol: Protocol::Tcp,
                        reachable: true,
                        detail: "TCP loopback connection succeeded".into(),
                    });
                }
                Ok(Err(e)) => {
                    results.push(PortCheckResult {
                        port: internal_port,
                        protocol: Protocol::Tcp,
                        reachable: false,
                        detail: format!("TCP loopback connect failed: {e}"),
                    });
                }
                Err(_) => {
                    results.push(PortCheckResult {
                        port: internal_port,
                        protocol: Protocol::Tcp,
                        reachable: false,
                        detail: "TCP loopback connect timed out".into(),
                    });
                }
            }
            drop(listener);
        }
        Err(e) => {
            results.push(PortCheckResult {
                port: internal_port,
                protocol: Protocol::Tcp,
                reachable: false,
                detail: format!("TCP loopback bind failed: {e}"),
            });
        }
    }

    Ok(results)
}

/// Run all wizard checks and collect results into a `WizardReport`.
///
/// Individual check failures are captured in the report rather than
/// short-circuiting, so the operator sees every problem at once.
pub async fn run_all_checks(
    region_name: &str,
    x: u32,
    y: u32,
    owner_id: &Uuid,
    internal_port: u16,
    external_host: &str,
    grid_connector: &(dyn GridConnector + '_),
    account_connector: &(dyn AccountConnector + '_),
) -> WizardReport {
    let mut outcomes = Vec::new();

    // 1. Region name
    match check_region_name(region_name, grid_connector).await {
        Ok(()) => outcomes.push(CheckOutcome {
            check_name: "region_name".into(),
            passed: true,
            message: format!("Region name '{}' is valid and available", region_name),
        }),
        Err(e) => outcomes.push(CheckOutcome {
            check_name: "region_name".into(),
            passed: false,
            message: e.to_string(),
        }),
    }

    // 2. Coordinates
    match check_coordinates(x, y, grid_connector).await {
        Ok(()) => outcomes.push(CheckOutcome {
            check_name: "coordinates".into(),
            passed: true,
            message: format!("Coordinates ({}, {}) are free", x, y),
        }),
        Err(e) => outcomes.push(CheckOutcome {
            check_name: "coordinates".into(),
            passed: false,
            message: e.to_string(),
        }),
    }

    // 3. Estate owner
    match check_estate_owner(owner_id, account_connector).await {
        Ok(()) => outcomes.push(CheckOutcome {
            check_name: "estate_owner".into(),
            passed: true,
            message: format!("Estate owner {} exists on the grid", owner_id),
        }),
        Err(e) => outcomes.push(CheckOutcome {
            check_name: "estate_owner".into(),
            passed: false,
            message: e.to_string(),
        }),
    }

    // 4. Ports
    let port_results = match check_ports(internal_port, external_host).await {
        Ok(results) => {
            let all_ok = results.iter().all(|r| r.reachable);
            outcomes.push(CheckOutcome {
                check_name: "ports".into(),
                passed: all_ok,
                message: if all_ok {
                    format!("Port {} is available on TCP and UDP", internal_port)
                } else {
                    format!(
                        "Port {} has issues: {}",
                        internal_port,
                        results
                            .iter()
                            .filter(|r| !r.reachable)
                            .map(|r| format!("{}: {}", r.protocol, r.detail))
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                },
            });
            results
        }
        Err(e) => {
            outcomes.push(CheckOutcome {
                check_name: "ports".into(),
                passed: false,
                message: e.to_string(),
            });
            Vec::new()
        }
    };

    WizardReport {
        outcomes,
        port_results,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;

    // -- Stub connectors for testing --

    struct StubGrid {
        name_exists: bool,
        position_taken: bool,
    }

    impl GridConnector for StubGrid {
        fn region_name_exists(
            &self,
            _name: &str,
        ) -> Pin<Box<dyn Future<Output = Result<bool, WizardError>> + Send + '_>> {
            let val = self.name_exists;
            Box::pin(async move { Ok(val) })
        }

        fn position_occupied(
            &self,
            _x: u32,
            _y: u32,
        ) -> Pin<Box<dyn Future<Output = Result<bool, WizardError>> + Send + '_>> {
            let val = self.position_taken;
            Box::pin(async move { Ok(val) })
        }
    }

    struct StubAccounts {
        exists: bool,
    }

    impl AccountConnector for StubAccounts {
        fn account_exists(
            &self,
            _id: &Uuid,
        ) -> Pin<Box<dyn Future<Output = Result<bool, WizardError>> + Send + '_>> {
            let val = self.exists;
            Box::pin(async move { Ok(val) })
        }
    }

    // -- Name format validation tests --

    #[tokio::test]
    async fn empty_name_rejected() {
        let grid = StubGrid {
            name_exists: false,
            position_taken: false,
        };
        let result = check_region_name("", &grid).await;
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), WizardError::InvalidName(_)),
            "expected InvalidName"
        );
    }

    #[tokio::test]
    async fn whitespace_only_name_rejected() {
        let grid = StubGrid {
            name_exists: false,
            position_taken: false,
        };
        let result = check_region_name("   ", &grid).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn long_name_rejected() {
        let grid = StubGrid {
            name_exists: false,
            position_taken: false,
        };
        let long_name = "A".repeat(129);
        let result = check_region_name(&long_name, &grid).await;
        assert!(matches!(result.unwrap_err(), WizardError::InvalidName(_)));
    }

    #[tokio::test]
    async fn valid_name_passes() {
        let grid = StubGrid {
            name_exists: false,
            position_taken: false,
        };
        let result = check_region_name("My Test Region", &grid).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn duplicate_name_rejected() {
        let grid = StubGrid {
            name_exists: true,
            position_taken: false,
        };
        let result = check_region_name("Taken Region", &grid).await;
        assert!(matches!(result.unwrap_err(), WizardError::DuplicateName(_)));
    }

    // -- Coordinate validation tests --

    #[tokio::test]
    async fn free_coordinates_pass() {
        let grid = StubGrid {
            name_exists: false,
            position_taken: false,
        };
        let result = check_coordinates(256000, 256000, &grid).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn occupied_coordinates_rejected() {
        let grid = StubGrid {
            name_exists: false,
            position_taken: true,
        };
        let result = check_coordinates(256000, 256000, &grid).await;
        assert!(matches!(
            result.unwrap_err(),
            WizardError::CoordinatesTaken(256000, 256000)
        ));
    }

    // -- Estate owner validation tests --

    #[tokio::test]
    async fn existing_owner_passes() {
        let accounts = StubAccounts { exists: true };
        let id = Uuid::new_v4();
        let result = check_estate_owner(&id, &accounts).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn missing_owner_rejected() {
        let accounts = StubAccounts { exists: false };
        let id = Uuid::new_v4();
        let result = check_estate_owner(&id, &accounts).await;
        assert!(matches!(result.unwrap_err(), WizardError::OwnerNotFound(_)));
    }

    // -- Port validation tests --

    #[tokio::test]
    async fn port_check_returns_results() {
        // Use port 0 to let the OS pick an available port for binding.
        // (Port 0 should always succeed since the OS assigns an ephemeral port.)
        let results = check_ports(0, "example.com").await.unwrap();
        assert!(!results.is_empty(), "should return at least one result");
        // With port 0, at least the initial bind tests should succeed.
        let tcp_bind = results
            .iter()
            .find(|r| r.protocol == Protocol::Tcp && r.detail.contains("available"));
        assert!(tcp_bind.is_some(), "TCP bind on port 0 should succeed");
    }

    #[tokio::test]
    async fn privileged_port_may_fail() {
        // Port 1 is privileged; binding should fail on non-root.
        let results = check_ports(1, "example.com").await.unwrap();
        // We just verify it returns results without panicking.
        assert!(!results.is_empty());
    }

    // -- Full wizard report test --

    #[tokio::test]
    async fn wizard_report_collects_all_checks() {
        let grid = StubGrid {
            name_exists: false,
            position_taken: false,
        };
        let accounts = StubAccounts { exists: true };
        let owner = Uuid::new_v4();

        let report = run_all_checks(
            "Test Region",
            256000,
            256000,
            &owner,
            0,
            "example.com",
            &grid,
            &accounts,
        )
        .await;

        assert_eq!(report.outcomes.len(), 4, "should have 4 check outcomes");
        assert!(
            report
                .outcomes
                .iter()
                .any(|o| o.check_name == "region_name"),
            "should have region_name check"
        );
        assert!(
            report
                .outcomes
                .iter()
                .any(|o| o.check_name == "coordinates"),
            "should have coordinates check"
        );
        assert!(
            report
                .outcomes
                .iter()
                .any(|o| o.check_name == "estate_owner"),
            "should have estate_owner check"
        );
        assert!(
            report.outcomes.iter().any(|o| o.check_name == "ports"),
            "should have ports check"
        );
    }

    #[tokio::test]
    async fn wizard_report_captures_failures_without_short_circuit() {
        let grid = StubGrid {
            name_exists: true,
            position_taken: true,
        };
        let accounts = StubAccounts { exists: false };
        let owner = Uuid::new_v4();

        let report = run_all_checks(
            "Bad Region",
            256000,
            256000,
            &owner,
            0,
            "example.com",
            &grid,
            &accounts,
        )
        .await;

        // Even though multiple checks fail, all four are reported.
        assert_eq!(report.outcomes.len(), 4);
        assert!(!report.all_passed());

        let name_check = report
            .outcomes
            .iter()
            .find(|o| o.check_name == "region_name")
            .unwrap();
        assert!(!name_check.passed);

        let coord_check = report
            .outcomes
            .iter()
            .find(|o| o.check_name == "coordinates")
            .unwrap();
        assert!(!coord_check.passed);

        let owner_check = report
            .outcomes
            .iter()
            .find(|o| o.check_name == "estate_owner")
            .unwrap();
        assert!(!owner_check.passed);
    }
}
