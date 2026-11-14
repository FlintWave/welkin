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
use opensim_data::presence::{PresenceRow, PresenceStore};

#[derive(Clone)]
pub struct PresenceServiceState {
    pub store: Arc<PresenceStore>,
}

pub fn router(state: PresenceServiceState) -> Router {
    Router::new()
        .route("/presence", post(handle_presence))
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

fn presence_xml(name: &str, p: &PresenceRow) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "UserID", &p.user_id);
    tag(&mut s, "RegionID", &p.region_id);
    let _ = write!(s, "</{name}>");
    s
}

async fn handle_presence(State(state): State<PresenceServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let store = &state.store;
    tracing::debug!(method, "presence request");

    match method {
        "login" => {
            tracing::warn!(user = f.str("UserID"), "illegal login attempt blocked");
            result(false)
        }
        "logout" => match store.logout(f.str("SessionID")).await {
            Ok(b) => result(b),
            Err(e) => {
                tracing::warn!(error = %e, "logout failed");
                result(false)
            }
        },
        "logoutregion" => match store.logout_region(f.str("RegionID")).await {
            Ok(_) => result(true),
            Err(e) => {
                tracing::warn!(error = %e, "logoutregion failed");
                result(false)
            }
        },
        "report" => match store.report_agent(f.str("SessionID"), f.str("RegionID")).await {
            Ok(()) => result(true),
            Err(e) => {
                tracing::warn!(error = %e, "report failed");
                result(false)
            }
        },
        "getagent" => match store.get_by_session(f.str("SessionID")).await {
            Ok(Some(p)) => xml_response(&presence_xml("result", &p)),
            _ => xml_response("<result>null</result>"),
        },
        "getagents" => match store.get_agents(&f.list("uuids")).await {
            Ok(rows) if !rows.is_empty() => {
                let mut s = String::new();
                for (i, p) in rows.iter().enumerate() {
                    s.push_str(&presence_xml(&format!("presence{i}"), p));
                }
                xml_response(&s)
            }
            _ => xml_response("<result>null</result>"),
        },
        _ => {
            tracing::warn!(method, "unknown presence method");
            result(false)
        }
    }
}
