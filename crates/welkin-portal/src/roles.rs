// SPDX-License-Identifier: LGPL-3.0-or-later
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StaffRole {
    Admin,
    Operator,
    Viewer,
}

impl StaffRole {
    pub fn can_manage_accounts(self) -> bool {
        matches!(self, StaffRole::Admin)
    }

    pub fn can_manage_regions(self) -> bool {
        matches!(self, StaffRole::Admin | StaffRole::Operator)
    }

    pub fn can_view_logs(self) -> bool {
        true
    }
}

impl std::fmt::Display for StaffRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StaffRole::Admin => write!(f, "admin"),
            StaffRole::Operator => write!(f, "operator"),
            StaffRole::Viewer => write!(f, "viewer"),
        }
    }
}
