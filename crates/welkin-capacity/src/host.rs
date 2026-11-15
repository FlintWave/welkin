// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use sysinfo::System;

use crate::CapacityError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CpuInfo {
    pub model: String,
    pub cores: usize,
    pub threads: usize,
    pub frequency_mhz: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub cgroup_limit_bytes: Option<u64>,
    pub effective_bytes: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiskInfo {
    pub path: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub write_speed_mbps: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContainerInfo {
    pub is_container: bool,
    pub memory_limit: Option<u64>,
    pub cpu_quota: Option<f64>,
}

pub async fn scan_cpu() -> Result<CpuInfo, CapacityError> {
    let sys = System::new_all();
    let cpus = sys.cpus();

    let model = cpus
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "unknown".into());

    let threads = cpus.len();
    let cores = sys.physical_core_count().unwrap_or(threads);
    let frequency_mhz = cpus
        .first()
        .map(|c| c.frequency() as f64)
        .unwrap_or(0.0);

    tracing::debug!(model = %model, cores, threads, frequency_mhz, "cpu scan complete");

    Ok(CpuInfo {
        model,
        cores,
        threads,
        frequency_mhz,
    })
}

pub async fn scan_memory() -> Result<MemoryInfo, CapacityError> {
    let sys = System::new_all();
    let total_bytes = sys.total_memory();
    let available_bytes = sys.available_memory();

    let cgroup_limit_bytes = read_cgroup_memory_limit().await;

    let effective_bytes = match cgroup_limit_bytes {
        Some(limit) if limit < total_bytes => limit,
        _ => total_bytes,
    };

    tracing::debug!(
        total_bytes,
        available_bytes,
        ?cgroup_limit_bytes,
        effective_bytes,
        "memory scan complete"
    );

    Ok(MemoryInfo {
        total_bytes,
        available_bytes,
        cgroup_limit_bytes,
        effective_bytes,
    })
}

async fn read_cgroup_memory_limit() -> Option<u64> {
    if let Ok(content) = tokio::fs::read_to_string("/sys/fs/cgroup/memory.max").await {
        let trimmed = content.trim();
        if trimmed == "max" {
            return None;
        }
        return trimmed.parse().ok();
    }

    if let Ok(content) =
        tokio::fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes").await
    {
        let val: u64 = content.trim().parse().ok()?;
        if val >= 0x7FFF_FFFF_FFFF_FFFF {
            return None;
        }
        return Some(val);
    }

    None
}

pub async fn scan_disk(path: &str) -> Result<DiskInfo, CapacityError> {
    let stat = nix::sys::statvfs::statvfs(path)
        .map_err(|e| CapacityError::Unavailable(format!("statvfs({path}): {e}")))?;

    let block_size = stat.fragment_size() as u64;
    let total_bytes = stat.blocks() as u64 * block_size;
    let available_bytes = stat.blocks_available() as u64 * block_size;

    tracing::debug!(path, total_bytes, available_bytes, "disk scan complete");

    Ok(DiskInfo {
        path: path.to_string(),
        total_bytes,
        available_bytes,
        write_speed_mbps: None,
    })
}

pub async fn scan_container() -> Result<ContainerInfo, CapacityError> {
    let dockerenv = tokio::fs::metadata("/.dockerenv").await.is_ok();
    let containerenv = tokio::fs::metadata("/run/.containerenv").await.is_ok();
    let is_container = dockerenv || containerenv;

    let memory_limit = read_cgroup_memory_limit().await;

    let cpu_quota = read_cgroup_cpu_quota().await;

    tracing::debug!(is_container, ?memory_limit, ?cpu_quota, "container scan complete");

    Ok(ContainerInfo {
        is_container,
        memory_limit,
        cpu_quota,
    })
}

async fn read_cgroup_cpu_quota() -> Option<f64> {
    if let Ok(content) = tokio::fs::read_to_string("/sys/fs/cgroup/cpu.max").await {
        let parts: Vec<&str> = content.trim().split_whitespace().collect();
        if parts.len() == 2 && parts[0] != "max" {
            let quota: f64 = parts[0].parse().ok()?;
            let period: f64 = parts[1].parse().ok()?;
            if period > 0.0 {
                return Some(quota / period);
            }
        }
        return None;
    }

    let quota_content = tokio::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_quota_us").await;
    let period_content = tokio::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_period_us").await;

    if let (Ok(q), Ok(p)) = (quota_content, period_content) {
        let quota: f64 = q.trim().parse().ok()?;
        let period: f64 = p.trim().parse().ok()?;
        if quota > 0.0 && period > 0.0 {
            return Some(quota / period);
        }
    }

    None
}
