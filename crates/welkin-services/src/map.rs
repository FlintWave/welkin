// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, fmt::Write as _, path::PathBuf, sync::Arc};

use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::Engine as _;
use quick_xml::escape::escape;

const ZERO_SCOPE: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Clone)]
pub struct MapImageServiceState {
    pub tile_dir: Arc<PathBuf>,
}

pub fn router(state: MapImageServiceState) -> Router {
    Router::new()
        .route("/map", post(handle_map_post))
        .route("/removemap", post(handle_removemap))
        .route("/map/{*path}", get(handle_map_get))
        .with_state(state)
}

#[derive(Default)]
struct Form {
    single: HashMap<String, String>,
}

impl Form {
    fn str(&self, key: &str) -> &str {
        self.single.get(key).map(String::as_str).unwrap_or("")
    }

    fn get(&self, key: &str) -> Option<&String> {
        self.single.get(key)
    }

    fn num<T: std::str::FromStr + Default>(&self, key: &str) -> T {
        self.str(key).trim().parse().unwrap_or_default()
    }
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_form(body: &[u8]) -> Form {
    let text = String::from_utf8_lossy(body);
    let mut form = Form::default();
    for pair in text.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        form.single.insert(url_decode(k), url_decode(v));
    }
    form
}

fn xml_response(content: &str) -> Response {
    let body = format!("<?xml version=\"1.0\"?><ServerResponse>{content}</ServerResponse>");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

fn tag(out: &mut String, name: &str, value: &str) {
    let _ = write!(out, "<{name}>{}</{name}>", escape(value));
}

fn ok_response() -> Response {
    xml_response("<Result>Success</Result>")
}

fn fail_response(message: &str) -> Response {
    let mut s = String::from("<Result>Failure</Result>");
    tag(&mut s, "Message", message);
    xml_response(&s)
}

fn tile_filename(x: i32, y: i32) -> String {
    format!("map-1-{x}-{y}-objects.jpg")
}

fn tile_path(tile_dir: &std::path::Path, scope: &str, x: i32, y: i32) -> PathBuf {
    tile_dir.join(scope).join(tile_filename(x, y))
}

fn remove_tile(tile_dir: &std::path::Path, scope: &str, x: i32, y: i32) {
    let path = tile_path(tile_dir, scope, x, y);
    let _ = std::fs::remove_file(path);
}

async fn handle_map_post(State(state): State<MapImageServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let x: i32 = f.num("X");
    let y: i32 = f.num("Y");
    let scope = {
        let s = f.str("SCOPE");
        if s.is_empty() { ZERO_SCOPE } else { s }
    };
    tracing::debug!(x, y, scope, "map post request");

    match f.get("DATA") {
        Some(data) if !data.is_empty() => {
            let decoded = match base64::engine::general_purpose::STANDARD.decode(data.as_bytes()) {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!(error = %e, "map tile base64 decode failed");
                    return fail_response("Invalid base64 data");
                }
            };
            let dir = state.tile_dir.join(scope);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                tracing::warn!(error = %e, "map tile directory creation failed");
                return fail_response("Failed to create tile directory");
            }
            let path = tile_path(&state.tile_dir, scope, x, y);
            if let Err(e) = std::fs::write(&path, &decoded) {
                tracing::warn!(error = %e, "map tile write failed");
                return fail_response("Failed to write tile");
            }
            ok_response()
        }
        _ => {
            remove_tile(&state.tile_dir, scope, x, y);
            ok_response()
        }
    }
}

async fn handle_removemap(State(state): State<MapImageServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let x: i32 = f.num("X");
    let y: i32 = f.num("Y");
    let scope = {
        let s = f.str("SCOPE");
        if s.is_empty() { ZERO_SCOPE } else { s }
    };
    tracing::debug!(x, y, scope, "removemap request");
    remove_tile(&state.tile_dir, scope, x, y);
    ok_response()
}

async fn handle_map_get(
    State(state): State<MapImageServiceState>,
    Path(path): Path<String>,
) -> Response {
    tracing::debug!(path = %path, "map get request");
    let full_path = state.tile_dir.join(&path);

    // Prevent path traversal
    let canonical = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let tile_dir_canonical = match state.tile_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    if !canonical.starts_with(&tile_dir_canonical) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match std::fs::read(&canonical) {
        Ok(data) => {
            let content_type = if path.ends_with(".png") {
                "image/png"
            } else {
                "image/jpeg"
            };
            (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], data).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
