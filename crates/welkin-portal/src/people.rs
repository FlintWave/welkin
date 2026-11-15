// SPDX-License-Identifier: GPL-3.0-or-later
use axum::extract::{Json, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::PortalError;
use crate::sessions::RequireStaff;

fn require_admin(staff: &RequireStaff) -> Result<(), PortalError> {
    if !staff.0.role.can_manage_accounts() {
        return Err(PortalError::Forbidden);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignUpRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captcha_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignUpStatus {
    Pending,
    Approved,
    Rejected,
    EmailSent,
    Confirmed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSignUp {
    pub id: String,
    #[serde(flatten)]
    pub request: SignUpRequest,
    pub status: SignUpStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    pub principal_id: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub created: String,
    pub user_level: i32,
    pub is_suspended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetToken {
    pub token: String,
    pub principal_id: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetConfirm {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccountSearchQuery {
    pub q: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SignUpStore {
    signups: Arc<Mutex<HashMap<String, PendingSignUp>>>,
    reset_tokens: Arc<Mutex<HashMap<String, PasswordResetToken>>>,
}

impl SignUpStore {
    pub fn new() -> Self {
        Self {
            signups: Arc::new(Mutex::new(HashMap::new())),
            reset_tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_signup(&self, signup: PendingSignUp) {
        self.signups
            .lock()
            .unwrap()
            .insert(signup.id.clone(), signup);
    }

    pub fn list_pending(&self) -> Vec<PendingSignUp> {
        self.signups
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.status == SignUpStatus::Pending)
            .cloned()
            .collect()
    }

    pub fn update_status(&self, id: &str, status: SignUpStatus) -> bool {
        if let Some(s) = self.signups.lock().unwrap().get_mut(id) {
            s.status = status;
            true
        } else {
            false
        }
    }

    pub fn store_reset_token(&self, token: PasswordResetToken) {
        self.reset_tokens
            .lock()
            .unwrap()
            .insert(token.token.clone(), token);
    }

    pub fn validate_reset_token(&self, token: &str) -> Option<PasswordResetToken> {
        let tokens = self.reset_tokens.lock().unwrap();
        let t = tokens.get(token)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        if t.expires_at < now {
            return None;
        }
        Some(t.clone())
    }

    pub fn consume_reset_token(&self, token: &str) -> Option<PasswordResetToken> {
        self.reset_tokens.lock().unwrap().remove(token)
    }
}

impl Default for SignUpStore {
    fn default() -> Self {
        Self::new()
    }
}

pub fn public_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static + AsRef<SignUpStore>,
{
    Router::new()
        .route("/portal/api/people/signup", post(handle_signup::<S>))
        .route(
            "/portal/api/people/password-reset",
            post(handle_password_reset::<S>),
        )
        .route(
            "/portal/api/people/password-reset/confirm",
            post(handle_password_reset_confirm::<S>),
        )
}

pub fn staff_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static + AsRef<crate::sessions::SessionStore> + AsRef<SignUpStore>,
{
    Router::new()
        .route(
            "/portal/api/people/signups",
            get(handle_list_signups::<S>),
        )
        .route(
            "/portal/api/people/signups/{id}/approve",
            post(handle_approve::<S>),
        )
        .route(
            "/portal/api/people/signups/{id}/reject",
            post(handle_reject::<S>),
        )
        .route(
            "/portal/api/people/accounts",
            get(handle_search_accounts::<S>),
        )
        .route(
            "/portal/api/people/accounts/{id}",
            get(handle_get_account::<S>),
        )
        .route(
            "/portal/api/people/accounts/{id}/suspend",
            post(handle_suspend::<S>),
        )
        .route(
            "/portal/api/people/accounts/{id}/unsuspend",
            post(handle_unsuspend::<S>),
        )
}

async fn handle_signup<S>(State(state): State<S>, Json(req): Json<SignUpRequest>) -> Response
where
    S: Clone + Send + Sync + 'static + AsRef<SignUpStore>,
{
    // TODO: validate captcha token (bot protection)
    let _ = req.captcha_token;

    let store: &SignUpStore = state.as_ref();
    let id = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    // TODO: send email confirmation
    tracing::info!(email = %req.email, "sign-up received; email confirmation not yet implemented");

    let password_hash = {
        use argon2::PasswordHasher;
        let salt = argon2::password_hash::SaltString::generate(
            &mut argon2::password_hash::rand_core::OsRng,
        );
        match argon2::Argon2::default().hash_password(req.password.as_bytes(), &salt) {
            Ok(h) => h.to_string(),
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "internal error" })),
                )
                    .into_response();
            }
        }
    };

    let signup = PendingSignUp {
        id: id.clone(),
        request: SignUpRequest {
            password: password_hash,
            ..req
        },
        status: SignUpStatus::Pending,
        created_at: now,
    };
    store.add_signup(signup);

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "status": "pending" })),
    )
        .into_response()
}

async fn handle_list_signups<S>(staff: RequireStaff, State(state): State<S>) -> Response
where
    S: Clone + Send + Sync + 'static + AsRef<SignUpStore>,
{
    if let Err(e) = require_admin(&staff) {
        return e.into_response();
    }
    let store: &SignUpStore = state.as_ref();
    let pending = store.list_pending();
    (StatusCode::OK, Json(serde_json::json!({ "signups": pending }))).into_response()
}

async fn handle_approve<S>(
    staff: RequireStaff,
    State(state): State<S>,
    Path(id): Path<String>,
) -> Response
where
    S: Clone + Send + Sync + 'static + AsRef<SignUpStore>,
{
    if let Err(e) = require_admin(&staff) {
        return e.into_response();
    }
    let store: &SignUpStore = state.as_ref();
    if store.update_status(&id, SignUpStatus::Approved) {
        // TODO: create UserAccount via welkin-data, send welcome email
        tracing::info!(signup_id = %id, "sign-up approved; account creation not yet wired");
        (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
    } else {
        PortalError::NotFound.into_response()
    }
}

async fn handle_reject<S>(
    staff: RequireStaff,
    State(state): State<S>,
    Path(id): Path<String>,
) -> Response
where
    S: Clone + Send + Sync + 'static + AsRef<SignUpStore>,
{
    if let Err(e) = require_admin(&staff) {
        return e.into_response();
    }
    let store: &SignUpStore = state.as_ref();
    if store.update_status(&id, SignUpStatus::Rejected) {
        (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
    } else {
        PortalError::NotFound.into_response()
    }
}

async fn handle_search_accounts<S>(
    staff: RequireStaff,
    State(_state): State<S>,
    Query(_q): Query<AccountSearchQuery>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    if let Err(e) = require_admin(&staff) {
        return e.into_response();
    }
    // TODO: query UserAccountStore
    let accounts: Vec<AccountSummary> = Vec::new();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "accounts": accounts, "total": 0 })),
    )
        .into_response()
}

async fn handle_get_account<S>(
    staff: RequireStaff,
    State(_state): State<S>,
    Path(_id): Path<String>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    if let Err(e) = require_admin(&staff) {
        return e.into_response();
    }
    // TODO: query UserAccountStore
    PortalError::NotFound.into_response()
}

async fn handle_suspend<S>(
    staff: RequireStaff,
    State(_state): State<S>,
    Path(_id): Path<String>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    if let Err(e) = require_admin(&staff) {
        return e.into_response();
    }
    // TODO: set UserLevel = -1 in UserAccountStore, audit log
    (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
}

async fn handle_unsuspend<S>(
    staff: RequireStaff,
    State(_state): State<S>,
    Path(_id): Path<String>,
) -> Response
where
    S: Clone + Send + Sync + 'static,
{
    if let Err(e) = require_admin(&staff) {
        return e.into_response();
    }
    // TODO: restore UserLevel in UserAccountStore, audit log
    (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
}

async fn handle_password_reset<S>(
    State(state): State<S>,
    Json(req): Json<PasswordResetRequest>,
) -> Response
where
    S: Clone + Send + Sync + 'static + AsRef<SignUpStore>,
{
    // TODO: look up account by email, generate token, send email
    tracing::info!(email = %req.email, "password reset requested; email not yet implemented");

    let store: &SignUpStore = state.as_ref();
    let token = PasswordResetToken {
        token: uuid::Uuid::new_v4().to_string(),
        principal_id: String::new(),
        expires_at: time::OffsetDateTime::now_utc().unix_timestamp() + 3600,
    };
    store.store_reset_token(token);

    (StatusCode::OK, Json(serde_json::json!({ "message": "if an account exists, a reset email has been sent" }))).into_response()
}

async fn handle_password_reset_confirm<S>(
    State(state): State<S>,
    Json(req): Json<PasswordResetConfirm>,
) -> Response
where
    S: Clone + Send + Sync + 'static + AsRef<SignUpStore>,
{
    let store: &SignUpStore = state.as_ref();
    match store.consume_reset_token(&req.token) {
        Some(_token_data) => {
            // TODO: update password via AuthStore
            tracing::info!("password reset confirmed; password update not yet wired");
            (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
        }
        None => {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid or expired token" })),
            )
                .into_response()
        }
    }
}
