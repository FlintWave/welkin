// SPDX-License-Identifier: GPL-3.0-or-later

//! HTTP client connector for a foreign grid's HGFriends service.
//!
//! Uses form-encoded POST with `<ServerResponse>` XML replies, matching
//! the C# `HGFriendsServicesConnector`.

use uuid::Uuid;

use crate::hypergrid::*;
use super::{form_post, parse_server_result, extract_tag_text};

/// Outbound HTTP client for a foreign HGFriends service.
pub struct HGFriendsServiceConnector {
    client: reqwest::Client,
    server_url: String,
}

impl HGFriendsServiceConnector {
    /// Create a new connector targeting the given HGFriends service URL.
    pub fn new(server_url: &str) -> Self {
        let mut url = server_url.to_string();
        if !url.ends_with('/') {
            url.push('/');
        }
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build reqwest client"),
            server_url: url,
        }
    }

    /// The endpoint URL for HGFriends form-post calls.
    fn endpoint(&self) -> String {
        format!("{}hgfriends", self.server_url)
    }

    /// Validate that a pending friendship offer exists between two users
    /// on the foreign grid.
    pub async fn validate_friendship_offered(
        &self,
        from_id: Uuid,
        to_id: Uuid,
    ) -> HgResult<bool> {
        let from_str = from_id.to_string();
        let to_str = to_id.to_string();
        let params: Vec<(&str, &str)> = vec![
            ("METHOD", "validate_friendship_offered"),
            ("FromID", &from_str),
            ("ToID", &to_str),
        ];
        let body = form_post(&self.client, &self.endpoint(), &params).await?;
        let result = parse_server_result(&body).unwrap_or_default();
        Ok(result.eq_ignore_ascii_case("success")
            || result.eq_ignore_ascii_case("true"))
    }

    /// Create a new cross-grid friendship record on the foreign grid.
    pub async fn new_friendship(
        &self,
        friend: &HgFriendInfo,
        secret: &str,
    ) -> HgResult<bool> {
        let principal = friend.principal_id.to_string();
        let friend_str = &friend.friend;
        let my_flags = friend.my_flags.to_string();
        let their_flags = friend.their_flags.to_string();
        let params: Vec<(&str, &str)> = vec![
            ("METHOD", "newfriendship"),
            ("PrincipalID", &principal),
            ("Friend", friend_str),
            ("MyFlags", &my_flags),
            ("TheirFlags", &their_flags),
            ("SECRET", secret),
        ];
        let body = form_post(&self.client, &self.endpoint(), &params).await?;
        let result = parse_server_result(&body).unwrap_or_default();
        Ok(result.eq_ignore_ascii_case("success")
            || result.eq_ignore_ascii_case("true"))
    }

    /// Delete a cross-grid friendship record, validated by the shared
    /// secret.
    pub async fn delete_friendship(
        &self,
        friend: &HgFriendInfo,
        secret: &str,
    ) -> HgResult<bool> {
        let principal = friend.principal_id.to_string();
        let friend_str = &friend.friend;
        let params: Vec<(&str, &str)> = vec![
            ("METHOD", "deletefriendship"),
            ("PrincipalID", &principal),
            ("Friend", friend_str),
            ("SECRET", secret),
        ];
        let body = form_post(&self.client, &self.endpoint(), &params).await?;
        let result = parse_server_result(&body).unwrap_or_default();
        Ok(result.eq_ignore_ascii_case("true"))
    }

    /// Relay online/offline status for a list of cross-grid friends.
    ///
    /// Returns the subset of friends that are online on the foreign grid.
    pub async fn status_notification(
        &self,
        friends: &[String],
        user_id: Uuid,
        online: bool,
    ) -> HgResult<Vec<Uuid>> {
        let uid = user_id.to_string();
        let online_str = if online { "true" } else { "false" };
        let mut params: Vec<(&str, &str)> = vec![
            ("METHOD", "statusnotification"),
            ("userID", &uid),
            ("online", online_str),
        ];

        // Friends are sent as numbered parameters: friend_0, friend_1, ...
        let friend_keys: Vec<String> = friends
            .iter()
            .enumerate()
            .map(|(i, _)| format!("friend_{i}"))
            .collect();
        for (i, f) in friends.iter().enumerate() {
            params.push((&friend_keys[i], f.as_str()));
        }

        let body =
            form_post(&self.client, &self.endpoint(), &params).await?;

        // Parse online friends from the response.
        // The response contains numbered elements: <friend_0>uuid</friend_0>...
        let mut online_friends = Vec::new();
        for i in 0..friends.len() {
            let tag = format!("friend_{i}");
            if let Some(val) = extract_tag_text(&body, &tag) {
                if let Ok(id) = Uuid::parse_str(&val) {
                    if !id.is_nil() {
                        online_friends.push(id);
                    }
                }
            }
        }

        Ok(online_friends)
    }

    /// Get the permission flags for a cross-grid friendship.
    pub async fn get_friend_perms(
        &self,
        user_id: Uuid,
        friend_id: Uuid,
    ) -> HgResult<i32> {
        let uid = user_id.to_string();
        let fid = friend_id.to_string();
        let params: Vec<(&str, &str)> = vec![
            ("METHOD", "getfriendperms"),
            ("PRINCIPALID", &uid),
            ("FRIENDID", &fid),
        ];
        let body = form_post(&self.client, &self.endpoint(), &params).await?;
        let result = parse_server_result(&body).unwrap_or_default();
        // Success result carries the perms as the Value element
        if let Some(val) = extract_tag_text(&body, "Value") {
            if let Ok(perms) = val.parse::<i32>() {
                return Ok(perms);
            }
        }
        // If we got a failure or can't parse, return -1
        if result.eq_ignore_ascii_case("failure") {
            return Ok(-1);
        }
        // Try parsing result itself as an integer (some responses embed it there)
        result.parse::<i32>().map_err(|_| {
            HypergridError::FriendsError("failed to parse friend perms".into())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_normalises_url() {
        let c = HGFriendsServiceConnector::new("http://example.com:9000");
        assert_eq!(c.endpoint(), "http://example.com:9000/hgfriends");
    }

    #[test]
    fn connector_with_trailing_slash() {
        let c = HGFriendsServiceConnector::new("http://example.com:9000/");
        assert_eq!(c.endpoint(), "http://example.com:9000/hgfriends");
    }
}
