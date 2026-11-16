// SPDX-License-Identifier: LGPL-3.0-or-later
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("internal error: {0}")]
    Internal(String),
    #[error("validation error: {0}")]
    Validation(String),
}

impl IntoResponse for PortalError {
    fn into_response(self) -> Response {
        let status = match &self {
            PortalError::Auth(_) => StatusCode::UNAUTHORIZED,
            PortalError::NotFound => StatusCode::NOT_FOUND,
            PortalError::Forbidden => StatusCode::FORBIDDEN,
            PortalError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            PortalError::Validation(_) => StatusCode::BAD_REQUEST,
        };
        let body = serde_json::json!({ "error": self.to_string() });
        (status, axum::Json(body)).into_response()
    }
}

pub fn not_implemented(message: &str) -> Response {
    (StatusCode::NOT_IMPLEMENTED, axum::Json(serde_json::json!({"error":"not_implemented","message":message}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn not_implemented_status_and_error() {
        let resp = not_implemented("soon");
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);

        let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["error"], "not_implemented");
    }
}
