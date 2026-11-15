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
use welkin_data::agent_prefs::{AgentPrefsRow, AgentPrefsStore};

#[derive(Clone)]
pub struct AgentPrefsServiceState {
    pub store: Arc<AgentPrefsStore>,
}

pub fn router(state: AgentPrefsServiceState) -> Router {
    Router::new()
        .route("/agentprefs", post(handle_agentprefs))
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

fn result_msg(success: bool) -> Response {
    xml_response(if success {
        "<result>Success</result>"
    } else {
        "<result>Failure</result>"
    })
}

async fn handle_agentprefs(State(state): State<AgentPrefsServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let store = &state.store;
    tracing::debug!(method, "agentprefs request");

    match method {
        "getagentprefs" => {
            let user_id = f.str("UserID");
            if user_id.is_empty() {
                return xml_response("<result>null</result>");
            }
            match store.get(user_id).await {
                Ok(Some(p)) => {
                    let mut s = String::from("<result type=\"List\">");
                    tag(&mut s, "PrincipalID", &p.principal_id);
                    tag(&mut s, "AccessPrefs", &p.access_prefs);
                    tag(&mut s, "HoverHeight", &p.hover_height.to_string());
                    tag(&mut s, "Language", &p.language);
                    tag(
                        &mut s,
                        "LanguageIsPublic",
                        &p.language_is_public.to_string(),
                    );
                    tag(&mut s, "PermEveryone", &p.perm_everyone.to_string());
                    tag(&mut s, "PermGroup", &p.perm_group.to_string());
                    tag(&mut s, "PermNextOwner", &p.perm_next_owner.to_string());
                    s.push_str("</result>");
                    xml_response(&s)
                }
                Ok(None) => xml_response("<result>null</result>"),
                Err(e) => {
                    tracing::warn!(error = %e, "getagentprefs failed");
                    xml_response("<result>null</result>")
                }
            }
        }
        "setagentprefs" => {
            let principal = f.str("PrincipalID");
            if principal.is_empty() {
                return result_msg(false);
            }
            let row = AgentPrefsRow {
                principal_id: principal.to_string(),
                access_prefs: f.str("AccessPrefs").to_string(),
                hover_height: f.num("HoverHeight"),
                language: f.str("Language").to_string(),
                language_is_public: f.num("LanguageIsPublic"),
                perm_everyone: f.num("PermEveryone"),
                perm_group: f.num("PermGroup"),
                perm_next_owner: f.num("PermNextOwner"),
            };
            match store.store(&row).await {
                Ok(()) => result_msg(true),
                Err(e) => {
                    tracing::warn!(error = %e, "setagentprefs failed");
                    result_msg(false)
                }
            }
        }
        "getagentlang" => {
            let user_id = f.str("UserID");
            if user_id.is_empty() {
                return xml_response("<Language>en-us</Language>");
            }
            match store.get(user_id).await {
                Ok(Some(p)) if p.language_is_public != 0 => {
                    let mut s = String::new();
                    tag(&mut s, "Language", &p.language);
                    xml_response(&s)
                }
                _ => xml_response("<Language>en-us</Language>"),
            }
        }
        _ => {
            tracing::warn!(method, "unknown agentprefs method");
            result_msg(false)
        }
    }
}
