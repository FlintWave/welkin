// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub portal_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGrid {
    pub name: String,
    pub url: String,
}

pub struct AppState {
    pub portal_url: Mutex<String>,
    pub remote_grids: Mutex<Vec<RemoteGrid>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            portal_url: Mutex::new("http://localhost:8002".into()),
            remote_grids: Mutex::new(Vec::new()),
        }
    }
}

#[tauri::command]
pub async fn start_service() -> Result<ServiceStatus, String> {
    let output = tokio::process::Command::new("systemctl")
        .args(["start", "welkin-robust.service"])
        .output()
        .await
        .map_err(|e| format!("Failed to start service: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Service start failed: {stderr}"));
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    service_status().await
}

#[tauri::command]
pub async fn stop_service() -> Result<ServiceStatus, String> {
    let output = tokio::process::Command::new("systemctl")
        .args(["stop", "welkin-robust.service"])
        .output()
        .await
        .map_err(|e| format!("Failed to stop service: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Service stop failed: {stderr}"));
    }

    service_status().await
}

#[tauri::command]
pub async fn service_status() -> Result<ServiceStatus, String> {
    let output = tokio::process::Command::new("systemctl")
        .args(["is-active", "welkin-robust.service"])
        .output()
        .await
        .map_err(|e| format!("Failed to check service: {e}"))?;

    let active = String::from_utf8_lossy(&output.stdout)
        .trim()
        .eq("active");

    let pid = if active {
        tokio::process::Command::new("systemctl")
            .args(["show", "--property=MainPID", "welkin-robust.service"])
            .output()
            .await
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .strip_prefix("MainPID=")
                    .and_then(|s| s.parse::<u32>().ok())
            })
            .filter(|&p| p > 0)
    } else {
        None
    };

    Ok(ServiceStatus {
        running: active,
        pid,
        uptime_secs: None, // TODO: parse ActiveEnterTimestamp from systemctl show
        portal_url: "http://localhost:8002".into(),
    })
}

#[tauri::command]
pub async fn view_logs(lines: Option<u32>) -> Result<Vec<LogEntry>, String> {
    let n = lines.unwrap_or(100).to_string();
    let output = tokio::process::Command::new("journalctl")
        .args(["-u", "welkin-robust.service", "-n", &n, "--no-pager", "-o", "json"])
        .output()
        .await
        .map_err(|e| format!("Failed to read logs: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<LogEntry> = stdout
        .lines()
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            Some(LogEntry {
                timestamp: v["__REALTIME_TIMESTAMP"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                level: v["PRIORITY"].as_str().unwrap_or("6").to_string(),
                message: v["MESSAGE"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
pub fn open_portal(state: State<'_, AppState>) -> Result<(), String> {
    let url = state
        .portal_url
        .lock()
        .map_err(|e| format!("Lock error: {e}"))?
        .clone();
    open::that(&url).map_err(|e| format!("Failed to open browser: {e}"))
}

#[tauri::command]
pub fn connect_remote(
    name: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let grid = RemoteGrid {
        name,
        url: url.clone(),
    };
    state
        .remote_grids
        .lock()
        .map_err(|e| format!("Lock error: {e}"))?
        .push(grid);
    *state
        .portal_url
        .lock()
        .map_err(|e| format!("Lock error: {e}"))? = url;
    Ok(())
}

#[tauri::command]
pub fn list_remote_grids(state: State<'_, AppState>) -> Result<Vec<RemoteGrid>, String> {
    Ok(state
        .remote_grids
        .lock()
        .map_err(|e| format!("Lock error: {e}"))?
        .clone())
}

#[tauri::command]
pub fn set_portal_url(url: String, state: State<'_, AppState>) -> Result<(), String> {
    *state
        .portal_url
        .lock()
        .map_err(|e| format!("Lock error: {e}"))? = url;
    Ok(())
}
