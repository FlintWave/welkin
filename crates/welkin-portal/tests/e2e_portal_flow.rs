// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end test: fresh machine → walkable island via installer + browser.
//!
//! Exercises the full portal flow through HTTP, proving every section works
//! together: wizard, auth, regions, map, people, content, and portal pages.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use welkin_portal::{PortalState, auth, content, map, people, portal_router, roles, wizard};

fn build_full_router() -> Router {
    let state = PortalState::new("0.1.0-e2e");
    let admin = auth::create_staff("admin", "admin-pass-42", roles::StaffRole::Admin).unwrap();
    state.staff_accounts.lock().unwrap().push(admin);

    let wizard_state = wizard::WizardServiceState::default();
    let content_state = content::ContentServiceState::default();

    portal_router(state.clone())
        .merge(wizard::router(wizard_state))
        .merge(map::router::<PortalState>().with_state(state.clone()))
        .merge(people::public_router::<PortalState>().with_state(state.clone()))
        .merge(people::staff_router::<PortalState>().with_state(state.clone()))
        .merge(content::router(content_state))
}

async fn json_resp(resp: axum::response::Response) -> (StatusCode, serde_json::Value) {
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    if bytes.is_empty() {
        return (status, serde_json::json!({}));
    }
    let json: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or(serde_json::json!({"_raw": String::from_utf8_lossy(&bytes).into_owned()}));
    (status, json)
}

async fn get(app: &Router, path: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::get(path).body(Body::empty()).unwrap();
    json_resp(app.clone().oneshot(req).await.unwrap()).await
}

async fn get_auth(app: &Router, path: &str, token: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::get(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    json_resp(app.clone().oneshot(req).await.unwrap()).await
}

async fn post_json(
    app: &Router,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::post(path)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    json_resp(app.clone().oneshot(req).await.unwrap()).await
}

async fn post_auth(
    app: &Router,
    path: &str,
    body: serde_json::Value,
    token: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::post(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    json_resp(app.clone().oneshot(req).await.unwrap()).await
}

async fn post_empty(app: &Router, path: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::post(path).body(Body::empty()).unwrap();
    json_resp(app.clone().oneshot(req).await.unwrap()).await
}

async fn post_empty_auth(app: &Router, path: &str, token: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::post(path)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    json_resp(app.clone().oneshot(req).await.unwrap()).await
}

#[tokio::test]
async fn full_portal_flow() {
    let app = build_full_router();

    // ── 1. Portal home page ──
    let (status, _) = get(&app, "/portal").await;
    assert_eq!(status, StatusCode::OK, "portal page should load");

    // ── 2. Public status endpoint ──
    let (status, json) = get(&app, "/portal/api/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["version"], "0.1.0-e2e");

    // ── 3. First-run wizard (7 steps) ──
    // 3a. Start
    let (status, json) = post_empty(&app, "/portal/api/wizard/start").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["step"], "mode");

    // 3b. Mode → standalone
    let (status, json) = post_json(
        &app,
        "/portal/api/wizard/step/mode",
        serde_json::json!({"mode": "standalone"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["step"], "owner");

    // 3c. Owner + world name
    let (status, json) = post_json(
        &app,
        "/portal/api/wizard/step/owner",
        serde_json::json!({
            "first_name": "Test",
            "last_name": "Owner",
            "password": "secure-pass-99",
            "world_name": "TestWorld"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["step"], "network");

    // 3d. Network probe
    let (status, json) = post_empty(&app, "/portal/api/wizard/step/network").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["step"], "capacity");

    // 3e. Capacity assessment
    let (status, json) = post_empty(&app, "/portal/api/wizard/step/capacity").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["step"], "placement");
    assert!(json["estimate"]["islands"]["min"].as_u64().unwrap() >= 1);

    // 3f. Placement — one island
    let (status, json) = post_json(
        &app,
        "/portal/api/wizard/step/placement",
        serde_json::json!({
            "islands": [{"name": "Welcome", "loc_x": 1000, "loc_y": 1000}]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["step"], "content");

    // 3g. Content choice — flat terrain
    let (status, json) = post_json(
        &app,
        "/portal/api/wizard/step/content",
        serde_json::json!({
            "assignments": [{"island_name": "Welcome", "content_type": "flat"}]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["step"], "finish");

    // 3h. Finish wizard
    let (status, json) = post_empty(&app, "/portal/api/wizard/finish").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "completed");
    assert_eq!(json["world_name"], "TestWorld");
    assert_eq!(json["islands_created"], 1);

    // Wizard is now completed; starting again should fail
    let (status, _) = post_empty(&app, "/portal/api/wizard/start").await;
    assert_eq!(status, StatusCode::CONFLICT);

    // ── 4. Staff login ──
    let (status, json) = post_json(
        &app,
        "/portal/api/auth/login",
        serde_json::json!({"username": "admin", "password": "admin-pass-42"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let token = json["token"]
        .as_str()
        .expect("login must return token")
        .to_string();
    assert!(!token.is_empty());
    assert_eq!(json["role"], "admin");

    // Wrong password rejected
    let (status, _) = post_json(
        &app,
        "/portal/api/auth/login",
        serde_json::json!({"username": "admin", "password": "wrong"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // ── 5. Region creation ──
    let (status, json) = post_auth(
        &app,
        "/portal/api/regions",
        serde_json::json!({
            "name": "Welcome",
            "loc_x": 1000,
            "loc_y": 1000,
            "size_x": 256,
            "size_y": 256
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let region_id = json["region_id"]
        .as_str()
        .expect("region must have id")
        .to_string();

    // ── 6. Region start ──
    let (status, json) = post_empty_auth(
        &app,
        &format!("/portal/api/regions/{region_id}/start"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "running");

    // ── 7. Verify region appears in list ──
    let (status, json) = get_auth(&app, "/portal/api/regions", &token).await;
    assert_eq!(status, StatusCode::OK);
    let regions = json.as_array().expect("regions list");
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0]["name"], "Welcome");
    assert_eq!(regions[0]["status"], "running");

    // ── 8. Map endpoint ──
    let (status, json) = get_auth(&app, "/portal/api/map/regions", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("regions").is_some());

    // Map placement
    let (status, json) = post_auth(
        &app,
        "/portal/api/map/place",
        serde_json::json!({
            "name": "SecondIsland",
            "loc_x": 2000,
            "loc_y": 2000,
            "size_x": 256,
            "size_y": 256
        }),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["success"], true);

    // ── 9. People: sign-up flow ──
    let (status, json) = post_json(
        &app,
        "/portal/api/people/signup",
        serde_json::json!({
            "first_name": "New",
            "last_name": "User",
            "email": "newuser@example.com",
            "password": "user-pass-42"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let signup_id = json["id"].as_str().expect("signup id").to_string();
    assert_eq!(json["status"], "pending");

    // Staff lists pending signups
    let (status, json) = get_auth(&app, "/portal/api/people/signups", &token).await;
    assert_eq!(status, StatusCode::OK);
    let signups = json["signups"].as_array().expect("signups list");
    assert_eq!(signups.len(), 1);
    assert_eq!(signups[0]["email"], "newuser@example.com");

    // Approve
    let (status, json) = post_empty_auth(
        &app,
        &format!("/portal/api/people/signups/{signup_id}/approve"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    // After approval, pending list should be empty
    let (status, json) = get_auth(&app, "/portal/api/people/signups", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["signups"].as_array().unwrap().is_empty());

    // ── 10. Portal pages (authenticated) ──
    let (status, _) = get_auth(&app, "/portal/api/logs", &token).await;
    assert_eq!(status, StatusCode::OK, "logs page");

    let (status, _) = get_auth(&app, "/portal/api/metrics", &token).await;
    assert_eq!(status, StatusCode::OK, "metrics page");

    let (status, _) = get_auth(&app, "/portal/api/audit", &token).await;
    assert_eq!(status, StatusCode::OK, "audit page");

    // ── 11. Content catalogue ──
    let (status, json) = get(&app, "/portal/api/content/catalogue").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("entries").is_some());

    // Terrain presets
    let (status, json) = get(&app, "/portal/api/content/terrain/presets").await;
    assert_eq!(status, StatusCode::OK);
    let presets = json["presets"].as_array().expect("presets");
    assert!(presets.len() >= 4);

    // ── 12. Unauthenticated access rejected ──
    let (status, _) = get(&app, "/portal/api/regions").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "regions needs auth");

    let (status, _) = get(&app, "/portal/api/logs").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "logs needs auth");

    // ── 13. Region snapshot + stop + remove ──
    let (status, _) = post_empty_auth(
        &app,
        &format!("/portal/api/regions/{region_id}/snapshot"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "snapshot");

    let (status, json) = post_empty_auth(
        &app,
        &format!("/portal/api/regions/{region_id}/stop"),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "stopped");

    // ── 14. Staff logout ──
    let (status, _) = post_empty_auth(&app, "/portal/api/auth/logout", &token).await;
    assert_eq!(status, StatusCode::OK);

    // Token should now be invalid
    let (status, _) = get_auth(&app, "/portal/api/regions", &token).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "token invalidated after logout"
    );
}
