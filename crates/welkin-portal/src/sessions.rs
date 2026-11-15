// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use serde::{Deserialize, Serialize};

use crate::roles::StaffRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub staff_id: String,
    pub role: StaffRole,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_session(&self, staff_id: &str, role: StaffRole, ttl_secs: i64) -> Session {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            staff_id: staff_id.to_string(),
            role,
            created_at: now,
            expires_at: now + ttl_secs,
        };
        self.sessions
            .lock()
            .unwrap()
            .insert(session.id.clone(), session.clone());
        session
    }

    pub fn validate_session(&self, token: &str) -> Option<Session> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions.get(token)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        if session.expires_at < now {
            return None;
        }
        Some(session.clone())
    }

    pub fn destroy_session(&self, token: &str) {
        self.sessions.lock().unwrap().remove(token);
    }

    pub fn cleanup_expired(&self) {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.sessions
            .lock()
            .unwrap()
            .retain(|_, s| s.expires_at >= now);
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RequireStaff(pub Session);

impl<S> FromRequestParts<S> for RequireStaff
where
    S: Send + Sync + AsRef<SessionStore>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store: &SessionStore = state.as_ref();

        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .or_else(|| {
                parts
                    .headers
                    .get("cookie")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|cookies| {
                        cookies.split(';').find_map(|c| {
                            let c = c.trim();
                            c.strip_prefix("portal_session=")
                        })
                    })
            });

        let token = token.ok_or(StatusCode::UNAUTHORIZED)?;
        store
            .validate_session(token)
            .map(RequireStaff)
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}
