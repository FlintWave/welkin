// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};

use crate::coefficients::CoefficientSet;
use crate::report::HostReport;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkloadProfile {
    pub name: String,
    pub agents_per_region: f64,
    pub prims_per_region: f64,
    pub scripts_fraction: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EstimateRange {
    pub min: u32,
    pub max: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CapacityEstimate {
    pub islands: EstimateRange,
    pub total_users: EstimateRange,
    pub limiting_resource: String,
    pub workload: String,
    pub is_measured: bool,
}

pub fn workload_profiles() -> Vec<WorkloadProfile> {
    vec![
        WorkloadProfile {
            name: "quiet".into(),
            agents_per_region: 5.0,
            prims_per_region: 3000.0,
            scripts_fraction: 0.1,
        },
        WorkloadProfile {
            name: "typical".into(),
            agents_per_region: 20.0,
            prims_per_region: 8000.0,
            scripts_fraction: 0.3,
        },
        WorkloadProfile {
            name: "busy".into(),
            agents_per_region: 60.0,
            prims_per_region: 14000.0,
            scripts_fraction: 0.6,
        },
    ]
}

pub fn estimate_capacity(
    report: &HostReport,
    coefficients: &CoefficientSet,
    profile: &WorkloadProfile,
) -> CapacityEstimate {
    let cores_per_region = coefficients.get("cpu_cores_per_region").unwrap_or(1.0);
    let memory_per_region = coefficients.get("memory_per_region_mb").unwrap_or(1024.0);
    let memory_base = coefficients.get("memory_base_mb").unwrap_or(200.0);
    let memory_per_1k_prims = coefficients.get("memory_per_1k_prims_mb").unwrap_or(30.0);
    let memory_per_user = coefficients.get("memory_per_user_mb").unwrap_or(50.0);
    let max_regions = coefficients.get("max_regions_per_server").unwrap_or(16.0);
    let client_throttle = coefficients.get("client_throttle_max_bps").unwrap_or(187500.0);

    let effective_cores = report.cpu.cores as f64;
    let usable_cores = (effective_cores - 1.0).max(0.5);
    let cpu_islands = usable_cores / cores_per_region;

    let effective_mb = report.memory.effective_bytes as f64 / 1_048_576.0;
    let per_region_mb = memory_per_region
        + memory_per_user * profile.agents_per_region
        + memory_per_1k_prims * profile.prims_per_region / 1000.0;
    let mem_islands = if per_region_mb > 0.0 {
        (effective_mb - memory_base) / per_region_mb
    } else {
        0.0
    };

    let bandwidth_users = report.network.upload_mbps.map(|mbps| {
        let bytes_per_sec = mbps * 1_000_000.0 / 8.0;
        if client_throttle > 0.0 {
            bytes_per_sec / client_throttle
        } else {
            f64::MAX
        }
    });

    let mut limiting = "cpu";
    let mut island_count = cpu_islands;

    if mem_islands < island_count {
        island_count = mem_islands;
        limiting = "memory";
    }

    if max_regions < island_count {
        island_count = max_regions;
        limiting = "configuration (max_regions_per_server)";
    }

    island_count = island_count.max(0.0);

    let user_count_from_islands = island_count * profile.agents_per_region;
    let mut user_count = user_count_from_islands;

    if let Some(bw_users) = bandwidth_users {
        if bw_users < user_count {
            user_count = bw_users;
            limiting = "bandwidth";
        }
    }

    let point_islands = island_count.floor() as u32;
    let point_users = user_count.floor() as u32;

    CapacityEstimate {
        islands: EstimateRange {
            min: ((point_islands as f64) * 0.8).floor() as u32,
            max: ((point_islands as f64) * 1.2).ceil() as u32,
        },
        total_users: EstimateRange {
            min: ((point_users as f64) * 0.8).floor() as u32,
            max: ((point_users as f64) * 1.2).ceil() as u32,
        },
        limiting_resource: limiting.into(),
        workload: profile.name.clone(),
        is_measured: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coefficients::default_coefficients;
    use crate::host::{ContainerInfo, CpuInfo, DiskInfo, MemoryInfo};
    use crate::network::NetworkInfo;
    use crate::report::HostReport;

    fn test_report(cores: usize, memory_gb: f64) -> HostReport {
        let mem_bytes = (memory_gb * 1_073_741_824.0) as u64;
        HostReport {
            cpu: CpuInfo {
                model: "Test CPU".into(),
                cores,
                threads: cores * 2,
                frequency_mhz: 3000.0,
            },
            memory: MemoryInfo {
                total_bytes: mem_bytes,
                available_bytes: mem_bytes,
                cgroup_limit_bytes: None,
                effective_bytes: mem_bytes,
            },
            disk: DiskInfo {
                path: "/".into(),
                total_bytes: 500_000_000_000,
                available_bytes: 400_000_000_000,
                write_speed_mbps: None,
            },
            container: ContainerInfo {
                is_container: false,
                memory_limit: None,
                cpu_quota: None,
            },
            network: NetworkInfo {
                public_ip: None,
                upload_mbps: None,
                download_mbps: None,
            },
            timestamp: "0".into(),
        }
    }

    #[test]
    fn quiet_workload_on_8_core_32gb() {
        let report = test_report(8, 32.0);
        let coeffs = default_coefficients();
        let profiles = workload_profiles();
        let quiet = &profiles[0];

        let est = estimate_capacity(&report, &coeffs, quiet);
        assert!(!est.is_measured);
        assert_eq!(est.workload, "quiet");
        assert!(est.islands.max > 0);
        assert!(est.total_users.max > 0);
    }

    #[test]
    fn busy_workload_on_4_core_8gb() {
        let report = test_report(4, 8.0);
        let coeffs = default_coefficients();
        let profiles = workload_profiles();
        let busy = &profiles[2];

        let est = estimate_capacity(&report, &coeffs, busy);
        assert_eq!(est.workload, "busy");
        assert!(est.islands.max <= 16);
    }

    #[test]
    fn single_core_reserves_half() {
        let report = test_report(1, 32.0);
        let coeffs = default_coefficients();
        let profiles = workload_profiles();
        let quiet = &profiles[0];

        let est = estimate_capacity(&report, &coeffs, quiet);
        assert!(est.islands.min == 0 || est.islands.max <= 1);
    }

    #[test]
    fn bandwidth_limited() {
        let mut report = test_report(16, 64.0);
        report.network.upload_mbps = Some(1.0);
        let coeffs = default_coefficients();
        let profiles = workload_profiles();
        let typical = &profiles[1];

        let est = estimate_capacity(&report, &coeffs, typical);
        assert_eq!(est.limiting_resource, "bandwidth");
    }

    #[test]
    fn default_coefficients_round_trip() {
        let coeffs = default_coefficients();
        let json = serde_json::to_string(&coeffs).unwrap();
        let parsed: crate::coefficients::CoefficientSet = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.coefficients.len(), coeffs.coefficients.len());
    }
}
