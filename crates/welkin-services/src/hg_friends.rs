// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid friends service -- manages cross-grid friendship records.
//!
//! Port of `OpenSim.Services.HypergridService.HGFriendsService`.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::RwLock;

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use quick_xml::escape::escape;
use uuid::Uuid;

use crate::hypergrid::*;

// ---------------------------------------------------------------------------
// Internal record
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct FriendRecord {
    principal_id: Uuid,
    /// Full UUI string, e.g. `"uuid;homeuri;first last;secret"`.
    friend: String,
    my_flags: i32,
    their_flags: i32,
}

// ---------------------------------------------------------------------------
// Service implementation
// ---------------------------------------------------------------------------

pub struct HGFriendsServiceImpl {
    friends: RwLock<Vec<FriendRecord>>,
}

impl HGFriendsServiceImpl {
    pub fn new() -> Self {
        Self {
            friends: RwLock::new(Vec::new()),
        }
    }

    // -- helpers --

    /// Extract the leading UUID from a UUI string like `"uuid;rest..."`.
    fn extract_uuid(uui: &str) -> Option<Uuid> {
        let id_str = uui.split(';').next()?;
        Uuid::parse_str(id_str.trim()).ok()
    }

    /// Parse a UUI `"uuid;homeuri;first last;secret"` into parts.
    fn parse_uui(uui: &str) -> Option<(Uuid, String, String, String)> {
        let parts: Vec<&str> = uui.splitn(4, ';').collect();
        if parts.len() < 2 {
            return None;
        }
        let id = Uuid::parse_str(parts[0].trim()).ok()?;
        let home_uri = parts.get(1).unwrap_or(&"").to_string();
        let name = parts.get(2).unwrap_or(&"").to_string();
        let secret = parts.get(3).unwrap_or(&"").to_string();
        Some((id, home_uri, name, secret))
    }

    fn store_friend(&self, principal_id: Uuid, friend: &str, my_flags: i32) {
        let mut friends = self.friends.write().unwrap();
        friends.push(FriendRecord {
            principal_id,
            friend: friend.to_string(),
            my_flags,
            their_flags: -1,
        });
    }

    fn delete_friend(&self, principal_id: Uuid, friend_prefix: &str) {
        let mut friends = self.friends.write().unwrap();
        friends
            .retain(|r| !(r.principal_id == principal_id && r.friend.starts_with(friend_prefix)));
    }

    fn get_friends_for(&self, principal_id: Uuid) -> Vec<FriendRecord> {
        let friends = self.friends.read().unwrap();
        friends
            .iter()
            .filter(|r| r.principal_id == principal_id)
            .cloned()
            .collect()
    }

    /// Look up their_flags for a reverse friendship:
    /// find records of `principal_id` where `friend` starts with `friend_id`.
    fn lookup_their_flags(&self, principal_id: Uuid, friend_id: Uuid) -> i32 {
        let friends = self.friends.read().unwrap();
        let prefix = friend_id.to_string();
        for r in friends.iter() {
            if r.principal_id == principal_id && r.friend.starts_with(&prefix) {
                return r.their_flags;
            }
        }
        -1
    }
}

impl Default for HGFriendsServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl HypergridFriendsService for HGFriendsServiceImpl {
    fn get_friend_perms(&self, user_id: Uuid, friend_id: Uuid) -> HgResult<i32> {
        let friends = self.get_friends_for(user_id);
        let prefix = friend_id.to_string();
        for finfo in &friends {
            if finfo.friend.starts_with(&prefix) {
                return Ok(finfo.their_flags);
            }
        }
        Ok(-1)
    }

    fn new_friendship(&self, friend: &HgFriendInfo, verified: bool) -> HgResult<bool> {
        // Parse the friend UUI to extract the friend UUID
        let friend_id = match Self::extract_uuid(&friend.friend.to_string()) {
            Some(id) => id,
            None => return Ok(false),
        };

        // Check if friendship already exists
        let existing = self.get_friends_for(friend.principal_id);
        let prefix = friend_id.to_string();
        for finfo in &existing {
            if finfo.friend.starts_with(&prefix) {
                return Ok(false);
            }
        }

        // Verified user session -- store directly
        if verified {
            self.store_friend(friend.principal_id, &friend.friend.to_string(), 0);
            return Ok(true);
        }

        // Check if the reverse friendship exists (user initiated the offer)
        let reverse_friends = self.get_friends_for(friend_id);
        let mut user_initiated = false;
        for finfo in &reverse_friends {
            let f_starts = finfo.friend.starts_with(&friend.principal_id.to_string());
            let p_matches = friend.friend.starts_with(&finfo.principal_id.to_string());
            if p_matches && f_starts && finfo.their_flags == -1 {
                user_initiated = true;
                // Delete the existing pending friendship
                self.delete_friend(friend_id, &finfo.friend);
                break;
            }
        }

        if user_initiated {
            // Complete friendship in both directions
            self.store_friend(friend.principal_id, &friend.friend.to_string(), 1);
            self.store_friend(friend_id, &friend.principal_id.to_string(), 1);
            // Update their_flags for both records
            {
                let mut friends = self.friends.write().unwrap();
                for r in friends.iter_mut() {
                    if r.principal_id == friend.principal_id && r.friend.starts_with(&prefix) {
                        r.their_flags = 1;
                    }
                    if r.principal_id == friend_id
                        && r.friend.starts_with(&friend.principal_id.to_string())
                    {
                        r.their_flags = 1;
                    }
                }
            }
            return Ok(true);
        }

        Ok(false)
    }

    fn delete_friendship(&self, friend: &HgFriendInfo, secret: &str) -> HgResult<bool> {
        let existing = self.get_friends_for(friend.principal_id);
        let friend_str = friend.friend.to_string();

        for finfo in &existing {
            if finfo.friend.starts_with(&friend_str) && finfo.friend.ends_with(secret) {
                // Delete both directions
                self.delete_friend(friend.principal_id, &finfo.friend);
                self.delete_friend(
                    Self::extract_uuid(&finfo.friend).unwrap_or(Uuid::nil()),
                    &friend.principal_id.to_string(),
                );
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn friendship_offered(
        &self,
        _from_id: Uuid,
        _from_name: &str,
        _to_id: Uuid,
        _message: &str,
    ) -> HgResult<bool> {
        // In the C# code this checks for a local user account and then
        // fires an async verification flow.  We stub the account check
        // (always present) and return true.
        Ok(true)
    }

    fn validate_friendship_offered(&self, from_id: Uuid, to_id: Uuid) -> HgResult<bool> {
        let friends = self.get_friends_for(to_id);
        let prefix = from_id.to_string();
        for finfo in &friends {
            if finfo.friend.starts_with(&prefix) && finfo.their_flags == -1 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn status_notification(
        &self,
        friends: &[String],
        foreign_user_id: Uuid,
        _online: bool,
    ) -> HgResult<Vec<Uuid>> {
        // Validate friends and collect locally-present users.
        // For each UUI, parse out localUserID and secret, then verify that
        // the friendship actually exists and the secret matches.
        let mut _users_to_notify = Vec::new();
        let foreign_prefix = foreign_user_id.to_string();

        for uui in friends {
            if let Some((local_user_id, _home_uri, _name, secret)) = Self::parse_uui(uui) {
                let user_friends = self.get_friends_for(local_user_id);
                for finfo in &user_friends {
                    if finfo.friend.starts_with(&foreign_prefix) && finfo.friend.ends_with(&secret)
                    {
                        _users_to_notify.push(local_user_id.to_string());
                    }
                }
            }
        }

        // In a real implementation we would query the presence service to
        // find which of `_users_to_notify` are online and forward status
        // notifications.  For now return empty -- the C# code also returns
        // empty on !online.
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// HTTP handler (axum)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct HGFriendsHandlerState {
    pub service: std::sync::Arc<HGFriendsServiceImpl>,
}

pub fn router(state: HGFriendsHandlerState) -> Router {
    Router::new()
        .route("/hgfriends", post(handle_hg_friends))
        .with_state(state)
}

// -- form helpers (same pattern as friends.rs / groups.rs) --

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

fn success_result(value: &str) -> Response {
    if value.is_empty() {
        xml_response("<RESULT>Success</RESULT>")
    } else {
        xml_response(&format!(
            "<RESULT>Success</RESULT><Value>{}</Value>",
            escape(value)
        ))
    }
}

fn failure_result(reason: &str) -> Response {
    if reason.is_empty() {
        xml_response("<RESULT>Failure</RESULT>")
    } else {
        xml_response(&format!(
            "<RESULT>Failure</RESULT><REASON>{}</REASON>",
            escape(reason)
        ))
    }
}

fn bool_result(v: bool) -> Response {
    xml_response(&format!(
        "<RESULT>{}</RESULT>",
        if v { "true" } else { "false" }
    ))
}

async fn handle_hg_friends(State(state): State<HGFriendsHandlerState>, body: Bytes) -> Response {
    let f = parse_form(&body);
    let method = f.str("METHOD");
    let svc = &state.service;
    tracing::debug!(method, "hgfriends request");

    match method {
        "getfriendperms" => {
            let principal_id = Uuid::parse_str(f.str("PRINCIPALID")).unwrap_or(Uuid::nil());
            let friend_id = Uuid::parse_str(f.str("FRIENDID")).unwrap_or(Uuid::nil());
            match svc.get_friend_perms(principal_id, friend_id) {
                Ok(perms) if perms >= 0 => success_result(&perms.to_string()),
                _ => failure_result("Friend not found"),
            }
        }
        "newfriendship" => {
            let friend = HgFriendInfo {
                principal_id: Uuid::parse_str(f.str("PrincipalID")).unwrap_or(Uuid::nil()),
                friend: f.str("Friend").to_string(),
                my_flags: f.str("MyFlags").parse().unwrap_or(0),
                their_flags: f.str("TheirFlags").parse().unwrap_or(0),
            };
            let verified = f.str("VERIFIED") == "1";
            match svc.new_friendship(&friend, verified) {
                Ok(true) => success_result(""),
                _ => failure_result(""),
            }
        }
        "deletefriendship" => {
            let friend = HgFriendInfo {
                principal_id: Uuid::parse_str(f.str("PrincipalID")).unwrap_or(Uuid::nil()),
                friend: f.str("Friend").to_string(),
                my_flags: f.str("MyFlags").parse().unwrap_or(0),
                their_flags: f.str("TheirFlags").parse().unwrap_or(0),
            };
            let secret = f.str("SECRET");
            match svc.delete_friendship(&friend, secret) {
                Ok(success) => bool_result(success),
                _ => bool_result(false),
            }
        }
        "friendship_offered" => {
            let from_id = Uuid::parse_str(f.str("FromID")).unwrap_or(Uuid::nil());
            let from_name = f.str("FromName");
            let to_id = Uuid::parse_str(f.str("ToID")).unwrap_or(Uuid::nil());
            let message = f.str("Message");
            match svc.friendship_offered(from_id, from_name, to_id, message) {
                Ok(true) => bool_result(true),
                _ => bool_result(false),
            }
        }
        "validate_friendship_offered" => {
            let from_id = Uuid::parse_str(f.str("FromID")).unwrap_or(Uuid::nil());
            let to_id = Uuid::parse_str(f.str("ToID")).unwrap_or(Uuid::nil());
            match svc.validate_friendship_offered(from_id, to_id) {
                Ok(true) => bool_result(true),
                _ => bool_result(false),
            }
        }
        "statusnotification" => {
            let user_id = Uuid::parse_str(f.str("UserID")).unwrap_or(Uuid::nil());
            let online = f.str("Online") == "true" || f.str("Online") == "True";
            // Collect friend UUI strings (numbered keys: Friends0, Friends1, ...)
            let mut friend_list = Vec::new();
            for i in 0..100 {
                let key = format!("Friends{i}");
                let val = f.str(&key);
                if val.is_empty() {
                    break;
                }
                friend_list.push(val.to_string());
            }
            match svc.status_notification(&friend_list, user_id, online) {
                Ok(online_ids) => {
                    let mut xml = String::new();
                    for (i, id) in online_ids.iter().enumerate() {
                        let _ = write!(xml, "<id{i}>{id}</id{i}>");
                    }
                    if xml.is_empty() {
                        xml_response("<result>null</result>")
                    } else {
                        xml_response(&xml)
                    }
                }
                Err(_) => xml_response("<result>null</result>"),
            }
        }
        _ => {
            tracing::warn!(method, "unknown hgfriends method");
            failure_result("")
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_svc() -> HGFriendsServiceImpl {
        HGFriendsServiceImpl::new()
    }

    #[test]
    fn test_new_friendship_verified() {
        let svc = make_svc();
        let principal = Uuid::new_v4();
        let friend_id = Uuid::new_v4();
        let friend_uui = format!("{friend_id};http://foreign.example:9000/;John Doe;abc123");

        let info = HgFriendInfo {
            principal_id: principal,
            friend: friend_uui,
            my_flags: 0,
            their_flags: 0,
        };

        let ok = svc.new_friendship(&info, true).unwrap();
        assert!(ok, "verified friendship should succeed");

        // Now get_friend_perms should find it (their_flags is -1 from the
        // internal store_friend which sets it to -1 by default)
        let perms = svc.get_friend_perms(principal, friend_id).unwrap();
        assert_eq!(perms, -1);
    }

    #[test]
    fn test_delete_friendship() {
        let svc = make_svc();
        let principal = Uuid::new_v4();
        let friend_id = Uuid::new_v4();
        let secret = "secretXYZ";
        let friend_uui = format!("{friend_id};http://foreign.example:9000/;John Doe;{secret}");

        let info = HgFriendInfo {
            principal_id: principal,
            friend: friend_uui.clone(),
            my_flags: 0,
            their_flags: 0,
        };
        svc.new_friendship(&info, true).unwrap();

        // Delete by matching friend prefix + secret
        let del_info = HgFriendInfo {
            principal_id: principal,
            friend: friend_id.to_string(),
            my_flags: 0,
            their_flags: 0,
        };
        let deleted = svc.delete_friendship(&del_info, secret).unwrap();
        assert!(deleted, "should delete friendship");

        // Now get_friend_perms returns -1 (not found)
        let perms = svc.get_friend_perms(principal, friend_id).unwrap();
        assert_eq!(perms, -1);
    }

    #[test]
    fn test_validate_friendship_offered() {
        let svc = make_svc();
        let from_id = Uuid::new_v4();
        let to_id = Uuid::new_v4();

        // Store a pending friendship (their_flags == -1)
        svc.store_friend(to_id, &format!("{from_id};http://foreign:9000/;J D;sec"), 0);

        let valid = svc.validate_friendship_offered(from_id, to_id).unwrap();
        assert!(valid, "pending friendship should validate");
    }

    #[test]
    fn test_get_friend_perms_not_found() {
        let svc = make_svc();
        let perms = svc
            .get_friend_perms(Uuid::new_v4(), Uuid::new_v4())
            .unwrap();
        assert_eq!(perms, -1);
    }

    #[test]
    fn test_status_notification_empty() {
        let svc = make_svc();
        let result = svc.status_notification(&[], Uuid::new_v4(), true).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_new_friendship_duplicate_rejected() {
        let svc = make_svc();
        let principal = Uuid::new_v4();
        let friend_id = Uuid::new_v4();
        let friend_uui = format!("{friend_id};http://foreign.example:9000/;A B;sec");

        let info = HgFriendInfo {
            principal_id: principal,
            friend: friend_uui.clone(),
            my_flags: 0,
            their_flags: 0,
        };

        assert!(svc.new_friendship(&info, true).unwrap());
        // Second call should be rejected (duplicate)
        assert!(!svc.new_friendship(&info, true).unwrap());
    }
}
