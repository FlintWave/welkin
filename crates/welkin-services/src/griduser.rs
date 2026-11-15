// SPDX-License-Identifier: LGPL-3.0-or-later
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
use welkin_data::grid_user::{GridUserRow, GridUserStore};

const DEFAULT_POSITION: &str = "<128, 128, 70>";
const DEFAULT_LOOK_AT: &str = "<0, 0, 0>";

#[derive(Clone)]
pub struct GridUserServiceState {
    pub store: Arc<GridUserStore>,
}

pub fn router(state: GridUserServiceState) -> Router {
    Router::new()
        .route("/griduser", post(handle_griduser))
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

fn griduser_xml(name: &str, u: &GridUserRow) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "UserID", &u.user_id);
    tag(&mut s, "HomeRegionID", &u.home_region_id);
    tag(&mut s, "HomePosition", &u.home_position);
    tag(&mut s, "HomeLookAt", &u.home_look_at);
    tag(&mut s, "LastRegionID", &u.last_region_id);
    tag(&mut s, "LastPosition", &u.last_position);
    tag(&mut s, "LastLookAt", &u.last_look_at);
    let online = if u.online.eq_ignore_ascii_case("true") {
        "True"
    } else {
        "False"
    };
    tag(&mut s, "Online", online);
    tag(&mut s, "Login", &u.login);
    tag(&mut s, "Logout", &u.logout);
    let _ = write!(s, "</{name}>");
    s
}

fn or_default<'a>(v: &'a str, d: &'a str) -> &'a str {
    if v.is_empty() { d } else { v }
}

fn db(r: Result<(), welkin_data::DataError>, what: &str) -> Response {
    match r {
        Ok(()) => result(true),
        Err(e) => {
            tracing::warn!(error = %e, what, "griduser update failed");
            result(false)
        }
    }
}

async fn handle_griduser(State(state): State<GridUserServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let store = &state.store;
    let user = f.str("UserID");
    let region = f.str("RegionID");
    let pos = or_default(f.str("Position"), DEFAULT_POSITION);
    let look = or_default(f.str("LookAt"), DEFAULT_LOOK_AT);
    tracing::debug!(method, "griduser request");

    match method {
        "loggedin" => match store.logged_in(user).await {
            Ok(row) => xml_response(&griduser_xml("result", &row)),
            Err(e) => {
                tracing::warn!(error = %e, "loggedin failed");
                result(false)
            }
        },
        "loggedout" => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .to_string();
            let r = match store.set_last_position(user, region, pos, look).await {
                Ok(()) => store.set_online(user, false, &now).await,
                Err(e) => Err(e),
            };
            db(r, "loggedout")
        }
        "sethome" => db(store.set_home(user, region, pos, look).await, "sethome"),
        "setposition" => db(
            store.set_last_position(user, region, pos, look).await,
            "setposition",
        ),
        "getgriduserinfo" => match store.get(user).await {
            Ok(Some(row)) => xml_response(&griduser_xml("result", &row)),
            _ => result(false),
        },
        "getgriduserinfos" => match store.get_multiple(&f.list("AgentIDs")).await {
            Ok(rows) => {
                let mut s = String::new();
                for (i, row) in rows.iter().flatten().enumerate() {
                    s.push_str(&griduser_xml(&format!("griduser{i}"), row));
                }
                if s.is_empty() {
                    xml_response("<result>null</result>")
                } else {
                    xml_response(&s)
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "getgriduserinfos failed");
                xml_response("<result>null</result>")
            }
        },
        _ => {
            tracing::warn!(method, "unknown griduser method");
            result(false)
        }
    }
}
