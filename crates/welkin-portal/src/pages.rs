// SPDX-License-Identifier: GPL-3.0-or-later
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::PortalState;
use crate::sessions::RequireStaff;

const PORTAL_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>OpenSimulator Grid Portal</title>
<style>
:root { --bg: #f8f9fa; --fg: #212529; --accent: #0d6efd; --card: #fff; --border: #dee2e6; }
@media (prefers-color-scheme: dark) {
  :root { --bg: #1a1a2e; --fg: #e0e0e0; --accent: #4dabf7; --card: #16213e; --border: #334; }
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: system-ui, sans-serif; background: var(--bg); color: var(--fg); padding: 2rem; max-width: 48rem; margin: 0 auto; }
h1 { margin-bottom: 1rem; }
.card { background: var(--card); border: 1px solid var(--border); border-radius: 0.5rem; padding: 1.5rem; margin-bottom: 1rem; }
a { color: var(--accent); }
ul { list-style: none; padding: 0; }
li { padding: 0.25rem 0; }
</style>
</head>
<body>
<h1>OpenSimulator Grid Portal</h1>
<div class="card">
<h2>API Endpoints</h2>
<ul>
<li><a href="/portal/api/status">/portal/api/status</a> — Grid status</li>
<li><code>POST /portal/api/auth/login</code> — Sign in</li>
<li><code>POST /portal/api/auth/logout</code> — Sign out</li>
<li><a href="/portal/api/regions">/portal/api/regions</a> — Region list</li>
<li><a href="/portal/api/audit">/portal/api/audit</a> — Audit log</li>
<li><a href="/portal/api/logs">/portal/api/logs</a> — Server logs</li>
<li><a href="/portal/api/metrics">/portal/api/metrics</a> — Metrics</li>
</ul>
</div>
<p>Portal UI will replace this page.</p>
</body>
</html>"#;

pub async fn handle_portal_page() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        PORTAL_HTML,
    )
        .into_response()
}

pub async fn handle_status(State(state): State<PortalState>) -> impl IntoResponse {
    let regions = state.region_statuses.lock().unwrap();
    let running = regions.iter().filter(|r| r.status == "running").count();
    let agents: u32 = regions.iter().map(|r| r.agents).sum();
    Json(serde_json::json!({
        "regions_running": running,
        "agents_online": agents,
        "pending_signups": 0,
        "uptime_secs": 0,
        "version": &state.version,
    }))
}

pub async fn handle_logs(_staff: RequireStaff) -> impl IntoResponse {
    // TODO: connect to tracing subscriber for real log entries
    Json(serde_json::json!([]))
}

pub async fn handle_metrics(_staff: RequireStaff) -> impl IntoResponse {
    // TODO: connect to metrics-exporter-prometheus
    Json(serde_json::json!({}))
}

pub async fn handle_audit(
    State(state): State<PortalState>,
    staff: RequireStaff,
) -> Result<impl IntoResponse, crate::error::PortalError> {
    if !staff.0.role.can_manage_accounts() {
        return Err(crate::error::PortalError::Forbidden);
    }
    Ok(Json(state.audit_log.recent(100)))
}
