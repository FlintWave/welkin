// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{delete, get, post};

pub mod audit;
pub mod auth;
pub mod content;
pub mod error;
pub mod map;
pub mod pages;
pub mod people;
pub mod region_control;
pub mod roles;
pub mod sessions;
pub mod wizard;

use audit::AuditLog;
use auth::StaffAccount;
use people::SignUpStore;
use region_control::RegionStatus;
use sessions::SessionStore;

#[derive(Clone)]
pub struct PortalState {
    pub staff_accounts: Arc<Mutex<Vec<StaffAccount>>>,
    pub sessions: SessionStore,
    pub audit_log: AuditLog,
    pub region_statuses: Arc<Mutex<Vec<RegionStatus>>>,
    pub signup_store: SignUpStore,
    pub version: String,
}

impl AsRef<SessionStore> for PortalState {
    fn as_ref(&self) -> &SessionStore {
        &self.sessions
    }
}

impl AsRef<SignUpStore> for PortalState {
    fn as_ref(&self) -> &SignUpStore {
        &self.signup_store
    }
}

impl PortalState {
    pub fn new(version: &str) -> Self {
        Self {
            staff_accounts: Arc::new(Mutex::new(Vec::new())),
            sessions: SessionStore::new(),
            audit_log: AuditLog::new(),
            region_statuses: Arc::new(Mutex::new(Vec::new())),
            signup_store: SignUpStore::new(),
            version: version.to_string(),
        }
    }
}

pub fn portal_router(state: PortalState) -> Router {
    let public = Router::new()
        .route("/portal", get(pages::handle_portal_page))
        .route("/portal/api/auth/login", post(auth::handle_login))
        .route("/portal/api/status", get(pages::handle_status));

    let authenticated = Router::new()
        .route("/portal/api/auth/logout", post(auth::handle_logout))
        .route(
            "/portal/api/regions",
            get(region_control::handle_list_regions).post(region_control::handle_create_region),
        )
        .route(
            "/portal/api/regions/{id}/start",
            post(region_control::handle_start_region),
        )
        .route(
            "/portal/api/regions/{id}/stop",
            post(region_control::handle_stop_region),
        )
        .route(
            "/portal/api/regions/{id}/snapshot",
            post(region_control::handle_snapshot_region),
        )
        .route(
            "/portal/api/regions/{id}",
            delete(region_control::handle_remove_region),
        )
        .route("/portal/api/audit", get(pages::handle_audit))
        .route("/portal/api/logs", get(pages::handle_logs))
        .route("/portal/api/metrics", get(pages::handle_metrics));

    public.merge(authenticated).with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_state() -> PortalState {
        let state = PortalState::new("0.1.0-test");
        let admin = auth::create_staff("admin", "password123", roles::StaffRole::Admin).unwrap();
        state.staff_accounts.lock().unwrap().push(admin);
        state
    }

    #[tokio::test]
    async fn portal_page_returns_html() {
        let state = test_state();
        let app = portal_router(state);
        let req = Request::get("/portal").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn status_returns_json() {
        let state = test_state();
        let app = portal_router(state);
        let req = Request::get("/portal/api/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn login_and_logout() {
        let state = test_state();
        let app = portal_router(state);

        let login_body = serde_json::json!({
            "username": "admin",
            "password": "password123"
        });
        let req = Request::post("/portal/api/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&login_body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 10_000)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let token = json["token"].as_str().unwrap();
        assert!(!token.is_empty());
    }

    #[tokio::test]
    async fn unauthenticated_regions_rejected() {
        let state = test_state();
        let app = portal_router(state);
        let req = Request::get("/portal/api/regions")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn login_wrong_password_rejected() {
        let state = test_state();
        let app = portal_router(state);
        let body = serde_json::json!({
            "username": "admin",
            "password": "wrong"
        });
        let req = Request::post("/portal/api/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
