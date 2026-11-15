// SPDX-License-Identifier: GPL-3.0-or-later
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::CapacityError;
use crate::host::{ContainerInfo, CpuInfo, DiskInfo, MemoryInfo};
use crate::network::NetworkInfo;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HostReport {
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disk: DiskInfo,
    pub container: ContainerInfo,
    pub network: NetworkInfo,
    pub timestamp: String,
}

impl HostReport {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

pub async fn scan_host(disk_path: &str) -> Result<HostReport, CapacityError> {
    let cpu = crate::host::scan_cpu().await?;
    let memory = crate::host::scan_memory().await?;
    let disk = crate::host::scan_disk(disk_path).await?;
    let container = crate::host::scan_container().await?;
    let network = crate::network::scan_network().await?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into());

    Ok(HostReport {
        cpu,
        memory,
        disk,
        container,
        network,
        timestamp,
    })
}

impl fmt::Display for HostReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Host Capacity Report ===")?;
        writeln!(f)?;
        writeln!(
            f,
            "CPU: {} ({} cores, {} threads, {:.0} MHz)",
            self.cpu.model, self.cpu.cores, self.cpu.threads, self.cpu.frequency_mhz
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "Memory: {:.1} GB total, {:.1} GB available",
            self.memory.total_bytes as f64 / 1_073_741_824.0,
            self.memory.available_bytes as f64 / 1_073_741_824.0
        )?;
        if let Some(limit) = self.memory.cgroup_limit_bytes {
            writeln!(
                f,
                "  cgroup limit: {:.1} GB",
                limit as f64 / 1_073_741_824.0
            )?;
        }
        writeln!(
            f,
            "  effective: {:.1} GB",
            self.memory.effective_bytes as f64 / 1_073_741_824.0
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "Disk ({}): {:.1} GB total, {:.1} GB available",
            self.disk.path,
            self.disk.total_bytes as f64 / 1_073_741_824.0,
            self.disk.available_bytes as f64 / 1_073_741_824.0
        )?;
        if let Some(speed) = self.disk.write_speed_mbps {
            writeln!(f, "  write speed: {speed:.1} MB/s")?;
        }
        writeln!(f)?;
        writeln!(
            f,
            "Container: {}",
            if self.container.is_container {
                "yes"
            } else {
                "no"
            }
        )?;
        if let Some(mem) = self.container.memory_limit {
            writeln!(f, "  memory limit: {:.1} GB", mem as f64 / 1_073_741_824.0)?;
        }
        if let Some(cpu) = self.container.cpu_quota {
            writeln!(f, "  CPU quota: {cpu:.2} cores")?;
        }
        writeln!(f)?;
        write!(f, "Network: ")?;
        match &self.network.public_ip {
            Some(ip) => writeln!(f, "public IP {ip}")?,
            None => writeln!(f, "public IP not detected")?,
        }
        if let Some(up) = self.network.upload_mbps {
            writeln!(f, "  upload: {up:.1} Mbps")?;
        }
        if let Some(down) = self.network.download_mbps {
            writeln!(f, "  download: {down:.1} Mbps")?;
        }
        Ok(())
    }
}
