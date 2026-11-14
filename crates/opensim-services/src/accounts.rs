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
use opensim_data::accounts::{UserAccountRow, UserAccountStore};
use opensim_data::auth::{AuthRow, AuthStore};
use opensim_data::inventory::InventoryStore;

#[derive(Clone)]
pub struct AccountsServiceState {
    pub store: Arc<UserAccountStore>,
    pub auth_store: Arc<AuthStore>,
    pub inventory_store: Arc<InventoryStore>,
}

pub fn router(state: AccountsServiceState) -> Router {
    Router::new()
        .route("/accounts", post(handle_accounts))
        .with_state(state)
}

#[derive(Default)]
struct Form {
    single: HashMap<String, String>,
    multi: HashMap<String, Vec<String>>,
}

impl Form {
    fn get(&self, key: &str) -> Option<&String> {
        self.single.get(key)
    }

    fn str(&self, key: &str) -> &str {
        self.single.get(key).map(String::as_str).unwrap_or("")
    }

    fn num<T: std::str::FromStr + Default>(&self, key: &str) -> T {
        self.str(key).trim().parse().unwrap_or_default()
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

fn result_fail() -> Response {
    xml_response("<result>Failure</result>")
}

fn account_xml(name: &str, a: &UserAccountRow) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "FirstName", &a.first_name);
    tag(&mut s, "LastName", &a.last_name);
    tag(&mut s, "Email", &a.email);
    tag(&mut s, "PrincipalID", &a.principal_id);
    tag(&mut s, "ScopeID", &a.scope_id);
    tag(&mut s, "Created", &a.created.to_string());
    tag(&mut s, "UserLevel", &a.user_level.to_string());
    tag(&mut s, "UserFlags", &a.user_flags.to_string());
    tag(&mut s, "UserTitle", &a.user_title);
    tag(&mut s, "UserCountry", "");
    tag(&mut s, "LocalToGrid", "True");
    tag(&mut s, "ServiceURLs", &a.service_urls);
    let _ = write!(s, "</{name}>");
    s
}

fn md5_hex(input: &str) -> String {
    format!("{:x}", md5::compute(input.as_bytes()))
}

async fn handle_accounts(State(state): State<AccountsServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.get("METHOD").map(String::as_str).unwrap_or("");
    let store = &state.store;
    tracing::debug!(method, "accounts request");

    match method {
        "getaccount" => {
            let uid = f.str("UserID");
            let pid = f.str("PrincipalID");
            let email = f.str("Email");
            let first = f.str("FirstName");
            let last = f.str("LastName");

            let result = if !uid.is_empty() {
                store.get_by_id(uid).await
            } else if !pid.is_empty() {
                store.get_by_id(pid).await
            } else if !email.is_empty() {
                store.get_by_email(email).await
            } else if !first.is_empty() && !last.is_empty() {
                store.get_by_name(first, last).await
            } else {
                Ok(None)
            };

            match result {
                Ok(Some(a)) => xml_response(&account_xml("result", &a)),
                Ok(None) => xml_response("<result>null</result>"),
                Err(e) => {
                    tracing::warn!(error = %e, "getaccount failed");
                    xml_response("<result>null</result>")
                }
            }
        }
        "getaccounts" => {
            let scope = f.str("ScopeID");
            let query = f.str("query");
            match store.get_accounts(scope, query).await {
                Ok(accounts) if !accounts.is_empty() => {
                    let mut s = String::new();
                    for (i, a) in accounts.iter().enumerate() {
                        s.push_str(&account_xml(&format!("account{i}"), a));
                    }
                    xml_response(&s)
                }
                Ok(_) => xml_response("<result>null</result>"),
                Err(e) => {
                    tracing::warn!(error = %e, "getaccounts failed");
                    xml_response("<result>null</result>")
                }
            }
        }
        "getmultiaccounts" => {
            let ids = f.list("IDS");
            match store.get_multi_accounts(&ids).await {
                Ok(accounts) => {
                    let mut s = String::new();
                    for (idx, a) in accounts.iter().flatten().enumerate() {
                        s.push_str(&account_xml(&format!("account{idx}"), a));
                    }
                    if s.is_empty() {
                        xml_response("<result>null</result>")
                    } else {
                        xml_response(&s)
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "getmultiaccounts failed");
                    xml_response("<result>null</result>")
                }
            }
        }
        // TODO: gate with AllowSetAccount config
        "setaccount" => {
            let pid = f.str("PrincipalID");
            if pid.is_empty() {
                return result_fail();
            }
            let existing = match store.get_by_id(pid).await {
                Ok(Some(a)) => a,
                Ok(None) => {
                    return result_fail();
                }
                Err(e) => {
                    tracing::warn!(error = %e, "setaccount lookup failed");
                    return result_fail();
                }
            };
            let updated = UserAccountRow {
                principal_id: existing.principal_id.clone(),
                scope_id: if f.get("ScopeID").is_some() {
                    f.str("ScopeID").to_string()
                } else {
                    existing.scope_id.clone()
                },
                first_name: if f.get("FirstName").is_some() {
                    f.str("FirstName").to_string()
                } else {
                    existing.first_name.clone()
                },
                last_name: if f.get("LastName").is_some() {
                    f.str("LastName").to_string()
                } else {
                    existing.last_name.clone()
                },
                email: if f.get("Email").is_some() {
                    f.str("Email").to_string()
                } else {
                    existing.email.clone()
                },
                service_urls: existing.service_urls.clone(),
                created: if f.get("Created").is_some() {
                    f.num("Created")
                } else {
                    existing.created
                },
                user_level: if f.get("UserLevel").is_some() {
                    f.num("UserLevel")
                } else {
                    existing.user_level
                },
                user_flags: if f.get("UserFlags").is_some() {
                    f.num("UserFlags")
                } else {
                    existing.user_flags
                },
                user_title: if f.get("UserTitle").is_some() {
                    f.str("UserTitle").to_string()
                } else {
                    existing.user_title.clone()
                },
                active: existing.active,
            };
            match store.store(&updated).await {
                Ok(()) => xml_response(&account_xml("result", &updated)),
                Err(e) => {
                    tracing::warn!(error = %e, "setaccount store failed");
                    result_fail()
                }
            }
        }
        // TODO: gate with AllowCreateUser config
        "createuser" => {
            let first = f.str("FirstName").to_string();
            let last = f.str("LastName").to_string();
            let password = f.str("Password").to_string();
            if first.is_empty() || last.is_empty() || password.is_empty() {
                return result_fail();
            }
            let pid = if f.str("PrincipalID").is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                f.str("PrincipalID").to_string()
            };
            let scope = if f.str("ScopeID").is_empty() {
                "00000000-0000-0000-0000-000000000000".to_string()
            } else {
                f.str("ScopeID").to_string()
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i32)
                .unwrap_or(0);

            let account = UserAccountRow {
                principal_id: pid.clone(),
                scope_id: scope,
                first_name: first,
                last_name: last,
                email: f.str("Email").to_string(),
                service_urls: String::new(),
                created: now,
                user_level: 0,
                user_flags: 0,
                user_title: String::new(),
                active: 1,
            };

            if let Err(e) = store.store(&account).await {
                tracing::warn!(error = %e, "createuser account store failed");
                return result_fail();
            }

            let salt = uuid::Uuid::new_v4().to_string();
            let hash = md5_hex(&format!("{password}:{salt}"));
            let auth_row = AuthRow {
                uuid: pid.clone(),
                password_hash: hash,
                password_salt: salt,
                web_login_key: "00000000-0000-0000-0000-000000000000".to_string(),
                account_type: "UserAccount".to_string(),
            };
            if let Err(e) = state.auth_store.store(&auth_row).await {
                tracing::warn!(error = %e, "createuser auth store failed");
                return result_fail();
            }

            if let Err(e) = state.inventory_store.create_user_inventory(&pid).await {
                tracing::warn!(error = %e, "createuser inventory creation failed");
            }

            xml_response(&account_xml("result", &account))
        }
        other => {
            tracing::warn!(method = other, "unknown accounts method");
            result_fail()
        }
    }
}
