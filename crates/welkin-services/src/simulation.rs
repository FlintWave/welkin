// SPDX-License-Identifier: LGPL-3.0-or-later
use axum::{
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};

#[derive(Clone)]
pub struct SimulationServiceState;

pub fn router(state: SimulationServiceState) -> Router {
    Router::new()
        .route("/agent/{*path}", post(not_implemented))
        .route("/object/{*path}", post(not_implemented))
        .with_state(state)
}

async fn not_implemented() -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        "Simulation service runs on region servers, not Robust",
    )
        .into_response()
}
