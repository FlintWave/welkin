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
