// SPDX-License-Identifier: GPL-3.0-or-later
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use argon2::PasswordHasher;

use crate::PortalState;
use crate::error::PortalError;
use crate::roles::StaffRole;
use crate::sessions::RequireStaff;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffAccount {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: StaffRole,
    pub totp_secret: Option<String>,
    pub created_at: String,
}

pub fn create_staff(
    username: &str,
    password: &str,
    role: StaffRole,
) -> Result<StaffAccount, PortalError> {
    let salt =
        argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let hash = argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| PortalError::Internal(e.to_string()))?
        .to_string();

    Ok(StaffAccount {
        id: uuid::Uuid::new_v4().to_string(),
        username: username.to_string(),
        password_hash: hash,
        role,
        totp_secret: None,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    })
}

pub fn verify_password(account: &StaffAccount, password: &str) -> bool {
    use argon2::PasswordVerifier;
    let parsed = match argon2::PasswordHash::new(&account.password_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    argon2::Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn verify_totp(_account: &StaffAccount, _code: &str) -> bool {
    // Reject all codes until TOTP verification is implemented.
    // Enabling totp_secret on an account locks it out — by design.
    false
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    role: StaffRole,
    expires_at: i64,
}

pub async fn handle_login(
    State(state): State<PortalState>,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, PortalError> {
    let accounts = state.staff_accounts.lock().unwrap();
    let account = accounts
        .iter()
        .find(|a| a.username == req.username)
        .cloned();
    drop(accounts);

    let account = account.ok_or_else(|| PortalError::Auth("invalid credentials".into()))?;

    if !verify_password(&account, &req.password) {
        state.audit_log.log_action(
            &account.id,
            "login_failed",
            &account.username,
            "wrong password",
            "",
        );
        return Err(PortalError::Auth("invalid credentials".into()));
    }

    if let Some(code) = &req.totp_code {
        if !verify_totp(&account, code) {
            return Err(PortalError::Auth("invalid TOTP code".into()));
        }
    } else if account.totp_secret.is_some() {
        return Err(PortalError::Auth("TOTP code required".into()));
    }

    let session = state
        .sessions
        .create_session(&account.id, account.role, 3600);

    state.audit_log.log_action(
        &account.id,
        "login",
        &account.username,
        "session created",
        "",
    );

    Ok(Json(LoginResponse {
        token: session.id,
        role: session.role,
        expires_at: session.expires_at,
    }))
}

pub async fn handle_logout(
    State(state): State<PortalState>,
    staff: RequireStaff,
) -> impl IntoResponse {
    state
        .audit_log
        .log_action(&staff.0.staff_id, "logout", "", "session destroyed", "");
    state.sessions.destroy_session(&staff.0.id);
    StatusCode::OK
}
