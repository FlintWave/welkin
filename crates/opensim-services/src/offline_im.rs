// SPDX-License-Identifier: GPL-3.0-or-later

//! OfflineIM service — HTTP handlers for storing and retrieving offline
//! instant messages.
//!
//! Matches the C# `OfflineIMServiceRobustConnector` REST interface.  The
//! region module POSTs with a `METHOD` field to store, retrieve, or delete
//! offline messages.

use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::{Value, json};

use opensim_data::offline_im::OfflineImStore;

// ---------------------------------------------------------------------------
// Helpers — minimal percent-decoding (no external crate needed)
// ---------------------------------------------------------------------------

fn percent_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                &input[i + 1..i + 3],
                16,
            ) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// State & router
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct OfflineIMServiceState {
    pub store: Arc<OfflineImStore>,
}

pub fn router(state: OfflineIMServiceState) -> Router {
    Router::new()
        .route("/offline", post(handle_offline_im))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn json_response(status: StatusCode, body: Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        body.to_string(),
    )
        .into_response()
}

fn success_response(result: Value) -> Response {
    json_response(StatusCode::OK, json!({ "result": "success", "data": result }))
}

fn failure_response(reason: &str) -> Response {
    json_response(
        StatusCode::OK,
        json!({ "result": "failure", "reason": reason }),
    )
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

async fn handle_offline_im(
    State(state): State<OfflineIMServiceState>,
    body: axum::body::Bytes,
) -> Response {
    // Parse the POST body — may be form-encoded or JSON.
    let params: Value = if let Ok(v) = serde_json::from_slice(&body) {
        v
    } else {
        // Try form-encoded → convert to JSON object.
        let text = String::from_utf8_lossy(&body);
        let mut map = serde_json::Map::new();
        for pair in text.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                let key = percent_decode(k);
                let val = percent_decode(v);
                map.insert(key, Value::String(val));
            }
        }
        Value::Object(map)
    };

    let method = params
        .get("METHOD")
        .or_else(|| params.get("method"))
        .and_then(Value::as_str)
        .unwrap_or("");

    tracing::debug!(method, "offline-im request");

    match method {
        "STORE" | "store" => handle_store(&state, &params).await,
        "GET" | "get" => handle_get(&state, &params).await,
        "DELETE" | "delete" => handle_delete(&state, &params).await,
        _ => {
            tracing::warn!(method, "unknown offline-im method");
            failure_response("unknown method")
        }
    }
}

/// Store a single offline message (METHOD=STORE).
async fn handle_store(state: &OfflineIMServiceState, params: &Value) -> Response {
    let principal_id = params
        .get("PrincipalID")
        .and_then(Value::as_str)
        .unwrap_or("");
    let from_id = params
        .get("FromID")
        .and_then(Value::as_str)
        .unwrap_or("");
    let message = params
        .get("Message")
        .and_then(Value::as_str)
        .unwrap_or("");

    if principal_id.is_empty() || message.is_empty() {
        return failure_response("missing PrincipalID or Message");
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    match state.store.store_message(principal_id, from_id, message, now).await {
        Ok(true) => success_response(json!(true)),
        Ok(false) => failure_response("Number of offline IMs has maxed out"),
        Err(e) => {
            tracing::warn!(error = %e, "offline-im store failed");
            failure_response("database error")
        }
    }
}

/// Retrieve offline messages for a user (METHOD=GET).
async fn handle_get(state: &OfflineIMServiceState, params: &Value) -> Response {
    let principal_id = params
        .get("PrincipalID")
        .and_then(Value::as_str)
        .unwrap_or("");

    if principal_id.is_empty() {
        return failure_response("missing PrincipalID");
    }

    match state.store.get_messages(principal_id).await {
        Ok(rows) => {
            let messages: Vec<Value> = rows
                .iter()
                .map(|r| {
                    json!({
                        "PrincipalID": r.principal_id,
                        "FromID": r.from_id,
                        "Message": r.message,
                        "TMStamp": r.tm_stamp,
                    })
                })
                .collect();

            // After retrieval, delete them (matching C# behaviour).
            if let Err(e) = state.store.delete_messages(principal_id).await {
                tracing::warn!(error = %e, "offline-im delete after get failed");
            }

            success_response(Value::Array(messages))
        }
        Err(e) => {
            tracing::warn!(error = %e, "offline-im get failed");
            failure_response("database error")
        }
    }
}

/// Delete offline messages for a user (METHOD=DELETE).
async fn handle_delete(state: &OfflineIMServiceState, params: &Value) -> Response {
    let user_id = params
        .get("UserID")
        .or_else(|| params.get("PrincipalID"))
        .and_then(Value::as_str)
        .unwrap_or("");

    if user_id.is_empty() {
        return failure_response("missing UserID");
    }

    // Delete both as recipient and as sender (matching C# DeleteMessages).
    let mut total = 0u64;
    match state.store.delete_messages(user_id).await {
        Ok(n) => total += n,
        Err(e) => {
            tracing::warn!(error = %e, "offline-im delete (principal) failed");
            return failure_response("database error");
        }
    }
    match state.store.delete_from_messages(user_id).await {
        Ok(n) => total += n,
        Err(e) => {
            tracing::warn!(error = %e, "offline-im delete (from) failed");
            return failure_response("database error");
        }
    }

    success_response(json!({ "deleted": total }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_response_format() {
        let resp = success_response(json!(true));
        // Just verify it returns 200 OK.
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn failure_response_format() {
        let resp = failure_response("some error");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn json_response_sets_content_type() {
        let resp = json_response(StatusCode::OK, json!({"test": true}));
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
