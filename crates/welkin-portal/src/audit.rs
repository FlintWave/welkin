// SPDX-License-Identifier: LGPL-3.0-or-later
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub staff_id: String,
    pub action: String,
    pub target: String,
    pub detail: String,
    pub timestamp: String,
    pub ip: String,
}

// TODO: back with sqlx for persistence
#[derive(Debug, Clone)]
pub struct AuditLog {
    entries: Arc<Mutex<Vec<AuditEntry>>>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log_action(&self, staff_id: &str, action: &str, target: &str, detail: &str, ip: &str) {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            staff_id: staff_id.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            detail: detail.to_string(),
            timestamp: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            ip: ip.to_string(),
        };
        tracing::info!(
            staff_id = entry.staff_id,
            action = entry.action,
            target = entry.target,
            "audit"
        );
        self.entries.lock().unwrap().push(entry);
    }

    pub fn recent(&self, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().unwrap();
        entries.iter().rev().take(limit).cloned().collect()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}
