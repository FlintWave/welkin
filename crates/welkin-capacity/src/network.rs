// SPDX-License-Identifier: LGPL-3.0-or-later
use serde::{Deserialize, Serialize};

use crate::CapacityError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkInfo {
    pub public_ip: Option<String>,
    pub upload_mbps: Option<f64>,
    pub download_mbps: Option<f64>,
}

pub async fn detect_public_ip() -> Option<String> {
    tracing::info!("public IP detection requires manual configuration or an external service");
    None
}

pub async fn scan_network() -> Result<NetworkInfo, CapacityError> {
    let public_ip = detect_public_ip().await;

    tracing::info!("network speed test requires manual benchmark or external endpoint");

    Ok(NetworkInfo {
        public_ip,
        upload_mbps: None,
        download_mbps: None,
    })
}
