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
use welkin_data::friends::{FriendWithFlags, FriendsStore};

#[derive(Clone)]
pub struct FriendsServiceState {
    pub store: Arc<FriendsStore>,
}

pub fn router(state: FriendsServiceState) -> Router {
    Router::new()
        .route("/friends", post(handle_friends))
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

fn friend_xml(name: &str, f: &FriendWithFlags) -> String {
    let mut s = format!("<{name} type=\"List\">");
    tag(&mut s, "PrincipalID", &f.principal_id);
    tag(&mut s, "Friend", &f.friend);
    tag(&mut s, "MyFlags", &f.my_flags);
    tag(&mut s, "TheirFlags", &f.their_flags);
    let _ = write!(s, "</{name}>");
    s
}

fn package_friends(friends: Vec<FriendWithFlags>) -> Response {
    if friends.is_empty() {
        xml_response("<result>null</result>")
    } else {
        let mut s = String::new();
        for (i, f) in friends.iter().enumerate() {
            s.push_str(&friend_xml(&format!("friend{i}"), f));
        }
        xml_response(&s)
    }
}

async fn handle_friends(State(state): State<FriendsServiceState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let store = &state.store;
    tracing::debug!(method, "friends request");

    match method {
        "getfriends" => {
            let principal = f.str("PRINCIPALID");
            match store.get_friends_with_their_flags(principal).await {
                Ok(friends) => package_friends(friends),
                Err(e) => {
                    tracing::warn!(error = %e, "getfriends failed");
                    xml_response("<result>null</result>")
                }
            }
        }
        "getfriends_string" => {
            let principal = f.str("PRINCIPALID");
            match store.get_friends_string_with_their_flags(principal).await {
                Ok(friends) => package_friends(friends),
                Err(e) => {
                    tracing::warn!(error = %e, "getfriends_string failed");
                    xml_response("<result>null</result>")
                }
            }
        }
        "storefriend" => {
            let principal = f.str("PrincipalID");
            let friend = f.str("Friend");
            let flags = f.str("MyFlags");
            match store.store_friend(principal, friend, flags).await {
                Ok(()) => ok(true),
                Err(e) => {
                    tracing::warn!(error = %e, "storefriend failed");
                    ok(false)
                }
            }
        }
        "deletefriend" | "deletefriend_string" => {
            let principal = f.str("PRINCIPALID");
            let friend = f.str("FRIEND");
            match store.delete_friend(principal, friend).await {
                Ok(true) => ok(true),
                Ok(false) => ok(false),
                Err(e) => {
                    tracing::warn!(error = %e, "deletefriend failed");
                    ok(false)
                }
            }
        }
        _ => {
            tracing::warn!(method, "unknown friends method");
            ok(false)
        }
    }
}
