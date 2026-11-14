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
use opensim_data::auth::{AuthRow, AuthStore};

const MAX_LIFETIME: i32 = 30;

#[derive(Clone)]
pub struct AuthServiceState {
    pub store: Arc<AuthStore>,
}

pub fn router(state: AuthServiceState) -> Router {
    Router::new()
        .route("/auth/plain", post(handle_auth))
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

fn ok(success: bool) -> Response {
    xml_response(if success {
        "<Result>Success</Result>"
    } else {
        "<Result>Failure</Result>"
    })
}

fn md5_hex(input: &str) -> String {
    format!("{:x}", md5::compute(input.as_bytes()))
}

async fn handle_auth(State(state): State<AuthServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let principal = f.str("PRINCIPAL");
    let store = &state.store;
    tracing::debug!(method, "auth request");

    if principal.is_empty() {
        return ok(false);
    }

    match method {
        "authenticate" => {
            let password = f.str("PASSWORD");
            let lifetime: i32 = {
                let v: i32 = f.num("LIFETIME");
                if v <= 0 || v > MAX_LIFETIME { MAX_LIFETIME } else { v }
            };

            let auth = match store.get(principal).await {
                Ok(Some(a)) => a,
                Ok(None) => {
                    tracing::debug!(principal, "auth: principal not found");
                    return ok(false);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "auth lookup failed");
                    return ok(false);
                }
            };

            let expected = md5_hex(&format!("{password}:{}", auth.password_salt));
            if !crate::constant_time_eq(&expected, &auth.password_hash) {
                tracing::debug!(principal, "auth: password mismatch");
                return ok(false);
            }

            let token = uuid::Uuid::new_v4().to_string();
            if let Err(e) = store.store_token(principal, &token, lifetime).await {
                tracing::warn!(error = %e, "auth: failed to store token");
                return ok(false);
            }

            let mut s = String::from("<Result>Success</Result>");
            tag(&mut s, "Token", &token);
            tag(&mut s, "Lifetime", &lifetime.to_string());
            xml_response(&s)
        }
        "verify" => {
            let token = f.str("TOKEN");
            match store.check_token(principal, token).await {
                Ok(true) => ok(true),
                Ok(false) => ok(false),
                Err(e) => {
                    tracing::warn!(error = %e, "verify failed");
                    ok(false)
                }
            }
        }
        "release" => {
            let token = f.str("TOKEN");
            match store.release_token(principal, token).await {
                Ok(true) => ok(true),
                Ok(false) => ok(false),
                Err(e) => {
                    tracing::warn!(error = %e, "release failed");
                    ok(false)
                }
            }
        }
        // TODO: gate with AllowSetPassword config
        "setpassword" => {
            let password = f.str("PASSWORD");
            if password.is_empty() {
                return ok(false);
            }
            let salt = uuid::Uuid::new_v4().to_string();
            let hash = md5_hex(&format!("{password}:{salt}"));
            match store.set_password(principal, &hash, &salt).await {
                Ok(()) => ok(true),
                Err(e) => {
                    tracing::warn!(error = %e, "setpassword failed");
                    ok(false)
                }
            }
        }
        // TODO: gate with AllowGetAuthInfo config
        "getauthinfo" => match store.get(principal).await {
            Ok(Some(auth)) => {
                let mut s = String::from("<result type=\"List\">");
                tag(&mut s, "PrincipalID", &auth.uuid);
                tag(&mut s, "AccountType", &auth.account_type);
                tag(&mut s, "PasswordHash", &auth.password_hash);
                tag(&mut s, "PasswordSalt", &auth.password_salt);
                tag(&mut s, "WebLoginKey", &auth.web_login_key);
                s.push_str("</result>");
                xml_response(&s)
            }
            Ok(None) => ok(false),
            Err(e) => {
                tracing::warn!(error = %e, "getauthinfo failed");
                ok(false)
            }
        },
        // TODO: gate with AllowSetAuthInfo config
        "setauthinfo" => {
            let existing = match store.get(principal).await {
                Ok(Some(a)) => a,
                Ok(None) => {
                    return ok(false);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "setauthinfo lookup failed");
                    return ok(false);
                }
            };
            let updated = AuthRow {
                uuid: existing.uuid.clone(),
                account_type: if f.str("AccountType").is_empty() {
                    existing.account_type.clone()
                } else {
                    f.str("AccountType").to_string()
                },
                password_hash: if f.str("PasswordHash").is_empty() {
                    existing.password_hash.clone()
                } else {
                    f.str("PasswordHash").to_string()
                },
                password_salt: if f.str("PasswordSalt").is_empty() {
                    existing.password_salt.clone()
                } else {
                    f.str("PasswordSalt").to_string()
                },
                web_login_key: if f.str("WebLoginKey").is_empty() {
                    existing.web_login_key.clone()
                } else {
                    f.str("WebLoginKey").to_string()
                },
            };
            match store.store(&updated).await {
                Ok(()) => ok(true),
                Err(e) => {
                    tracing::warn!(error = %e, "setauthinfo store failed");
                    ok(false)
                }
            }
        }
        _ => {
            tracing::warn!(method, "unknown auth method");
            ok(false)
        }
    }
}
