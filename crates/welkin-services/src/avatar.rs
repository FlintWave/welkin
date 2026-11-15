// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, fmt::Write as _, sync::Arc};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use quick_xml::escape::escape;
use welkin_data::avatar::AvatarStore;

#[derive(Clone)]
pub struct AvatarServiceState {
    pub store: Arc<AvatarStore>,
}

pub fn router(state: AvatarServiceState) -> Router {
    Router::new()
        .route("/avatar", post(handle_avatar))
        .with_state(state)
}

#[derive(Default)]
struct Form {
    single: HashMap<String, String>,
    multi: HashMap<String, Vec<String>>,
}

impl Form {
    fn str(&self, key: &str) -> &str {
        self.single.get(key).map(String::as_str).unwrap_or("")
    }

    fn list(&self, key: &str) -> Vec<&str> {
        self.multi
            .get(key)
            .map(|v| v.iter().map(String::as_str).collect())
            .unwrap_or_default()
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
        let key = url_decode(k);
        let value = url_decode(v);
        if let Some(base) = key.strip_suffix("[]") {
            form.multi.entry(base.to_string()).or_default().push(value);
        } else {
            form.single.insert(key, value);
        }
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

fn result(success: bool) -> Response {
    xml_response(if success {
        "<result>Success</result>"
    } else {
        "<result>Failure</result>"
    })
}

const STRIP_KEYS: &[&str] = &["METHOD", "VERSIONMAX", "VERSIONMIN", "UserID"];

async fn handle_avatar(State(state): State<AvatarServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let store = &state.store;
    let user = f.str("UserID");
    tracing::debug!(method, "avatar request");

    match method {
        "getavatar" => {
            if user.is_empty() {
                return result(false);
            }
            match store.get(user).await {
                Ok(rows) if !rows.is_empty() => {
                    let mut s = String::from("<result type=\"List\">");
                    tag(&mut s, "AvatarType", "1");
                    for row in &rows {
                        let val = row.value.as_deref().unwrap_or("");
                        tag(&mut s, &row.name, val);
                    }
                    s.push_str("</result>");
                    xml_response(&s)
                }
                Ok(_) => result(false),
                Err(e) => {
                    tracing::warn!(error = %e, "getavatar failed");
                    result(false)
                }
            }
        }
        "setavatar" => {
            if user.is_empty() {
                return result(false);
            }
            let entries: Vec<(String, String)> = f
                .single
                .iter()
                .filter(|(k, _)| !STRIP_KEYS.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            match store.store(user, &entries).await {
                Ok(()) => result(true),
                Err(e) => {
                    tracing::warn!(error = %e, "setavatar failed");
                    result(false)
                }
            }
        }
        "resetavatar" => {
            if user.is_empty() {
                return result(false);
            }
            match store.reset(user).await {
                Ok(_) => result(true),
                Err(e) => {
                    tracing::warn!(error = %e, "resetavatar failed");
                    result(false)
                }
            }
        }
        "setitems" => {
            if user.is_empty() {
                return result(false);
            }
            let names = f.list("Names");
            let values = f.list("Values");
            if names.is_empty() || names.len() != values.len() {
                return result(false);
            }
            match store.set_items(user, &names, &values).await {
                Ok(()) => result(true),
                Err(e) => {
                    tracing::warn!(error = %e, "setitems failed");
                    result(false)
                }
            }
        }
        "removeitems" => {
            if user.is_empty() {
                return result(false);
            }
            let names = f.list("Names");
            if names.is_empty() {
                return result(false);
            }
            match store.remove_items(user, &names).await {
                Ok(()) => result(true),
                Err(e) => {
                    tracing::warn!(error = %e, "removeitems failed");
                    result(false)
                }
            }
        }
        _ => {
            tracing::warn!(method, "unknown avatar method");
            result(false)
        }
    }
}
