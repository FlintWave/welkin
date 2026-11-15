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
use base64::Engine as _;
use quick_xml::escape::escape;
use welkin_data::mute::{MuteRow, MuteStore};

#[derive(Clone)]
pub struct MuteServiceState {
    pub store: Arc<MuteStore>,
}

pub fn router(state: MuteServiceState) -> Router {
    Router::new()
        .route("/mutelist", post(handle_mutelist))
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

    fn num<T: std::str::FromStr + Default>(&self, key: &str) -> T {
        self.str(key).trim().parse().unwrap_or_default()
    }

    fn has(&self, key: &str) -> bool {
        self.single.contains_key(key)
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

fn result(success: bool) -> Response {
    xml_response(if success {
        "<result>Success</result>"
    } else {
        "<result>Failure</result>"
    })
}

fn unix_now() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i32)
        .unwrap_or(0)
}

async fn handle_mutelist(State(state): State<MuteServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let store = &state.store;
    tracing::debug!(method, "mutelist request");

    match method {
        "get" => {
            let agent_id = f.str("agentid");
            let mute_crc: u32 = f.num("mutecrc");
            if agent_id.is_empty() {
                return result(false);
            }

            let mutes = match store.get_mutes(agent_id).await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(error = %e, "get mutes failed");
                    return result(false);
                }
            };

            if mutes.is_empty() {
                let mut s = String::new();
                tag(&mut s, "result", "");
                return xml_response(&s);
            }

            let mut text = String::with_capacity(16384);
            for m in &mutes {
                let _ = writeln!(
                    text,
                    "{} {} {}|{}",
                    m.mute_type, m.mute_id, m.mute_name, m.mute_flags
                );
            }

            let file_data = text.as_bytes();
            let data_crc = crc32fast::hash(file_data);

            if data_crc == mute_crc && mute_crc != 0 {
                let b64 = base64::engine::general_purpose::STANDARD.encode([1u8]);
                let mut s = String::new();
                tag(&mut s, "result", &b64);
                return xml_response(&s);
            }

            let b64 = base64::engine::general_purpose::STANDARD.encode(file_data);
            let mut s = String::new();
            tag(&mut s, "result", &b64);
            xml_response(&s)
        }
        "update" => {
            let agent_id = f.str("agentid");
            let mute_id = f.str("muteid");
            if agent_id.is_empty() || mute_id.is_empty() {
                return result(false);
            }
            let mute = MuteRow {
                agent_id: agent_id.to_string(),
                mute_id: mute_id.to_string(),
                mute_name: f.str("mutename").to_string(),
                mute_type: if f.has("mutetype") {
                    f.num("mutetype")
                } else {
                    0
                },
                mute_flags: if f.has("muteflags") {
                    f.num("muteflags")
                } else {
                    0
                },
                stamp: if f.has("mutestamp") {
                    f.num("mutestamp")
                } else {
                    unix_now()
                },
            };
            match store.add_mute(&mute).await {
                Ok(()) => result(true),
                Err(e) => {
                    tracing::warn!(error = %e, "update mute failed");
                    result(false)
                }
            }
        }
        "delete" => {
            let agent_id = f.str("agentid");
            let mute_id = f.str("muteid");
            if agent_id.is_empty() || mute_id.is_empty() {
                return result(false);
            }
            let mute_name = f.str("mutename");
            match store.remove_mute(agent_id, mute_id, mute_name).await {
                Ok(_) => result(true),
                Err(e) => {
                    tracing::warn!(error = %e, "delete mute failed");
                    result(false)
                }
            }
        }
        _ => {
            tracing::warn!(method, "unknown mutelist method");
            result(false)
        }
    }
}
