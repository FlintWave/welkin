// SPDX-License-Identifier: GPL-3.0-or-later

//! HTTP client connector for a foreign grid's UserAgent service.
//!
//! Uses XML-RPC for all calls (verify_agent, verify_client, get_home_region,
//! agent_is_coming_home, get_server_urls, get_user_info, locate_user,
//! get_uui, get_uuid, logout_agent).

use std::collections::HashMap;

use uuid::Uuid;

use crate::hypergrid::*;
use super::xmlrpc_call;

/// Outbound HTTP client for a foreign UserAgent service.
pub struct UserAgentServiceConnector {
    client: reqwest::Client,
    server_url: String,
}

impl UserAgentServiceConnector {
    /// Create a new connector targeting the given UserAgent service URL.
    pub fn new(server_url: &str) -> Self {
        let mut url = server_url.to_lowercase();
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

    /// Low-level helper: call the given XML-RPC method and return the
    /// response hashtable.
    async fn call(
        &self,
        method: &str,
        params: &[(&str, &str)],
    ) -> HgResult<HashMap<String, String>> {
        xmlrpc_call(&self.client, &self.server_url, method, params).await
    }

    /// Verify that a session token matches the traveling-agent record on
    /// the home grid.
    pub async fn verify_agent(
        &self,
        session_id: Uuid,
        token: &str,
    ) -> HgResult<bool> {
        let sid = session_id.to_string();
        let map = self
            .call("verify_agent", &[("sessionID", &sid), ("token", token)])
            .await?;
        Ok(map
            .get("result")
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(false))
    }

    /// Verify that the reported client IP matches the one stored at login.
    pub async fn verify_client(
        &self,
        session_id: Uuid,
        reported_ip: &str,
    ) -> HgResult<bool> {
        let sid = session_id.to_string();
        let map = self
            .call(
                "verify_client",
                &[("sessionID", &sid), ("reportedIP", reported_ip)],
            )
            .await?;
        Ok(map
            .get("result")
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(false))
    }

    /// Get the home region for a user on the foreign grid.
    ///
    /// Returns `None` when the foreign service reports no home region.
    pub async fn get_home_region(
        &self,
        user_id: Uuid,
    ) -> HgResult<Option<(GridRegionInfo, Vector3, Vector3)>> {
        let uid = user_id.to_string();
        let map = self
            .call("get_home_region", &[("userID", &uid)])
            .await?;

        let result = map
            .get("result")
            .map(|s| s.as_str())
            .unwrap_or("false");
        if !result.eq_ignore_ascii_case("true") {
            return Ok(None);
        }

        let region = GridRegionInfo {
            region_id: map
                .get("uuid")
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or(Uuid::nil()),
            region_name: map
                .get("region_name")
                .cloned()
                .unwrap_or_default(),
            region_handle: 0,
            server_uri: map
                .get("server_uri")
                .cloned()
                .unwrap_or_default(),
            external_host_name: map
                .get("hostname")
                .cloned()
                .unwrap_or_default(),
            internal_port: map
                .get("internal_port")
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(0),
            region_loc_x: map
                .get("x")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
            region_loc_y: map
                .get("y")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
            region_size_x: map
                .get("size_x")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(256),
            region_size_y: map
                .get("size_y")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(256),
        };

        let position = parse_vector3(
            map.get("position").map(|s| s.as_str()).unwrap_or(""),
        );
        let look_at = parse_vector3(
            map.get("lookAt").map(|s| s.as_str()).unwrap_or(""),
        );

        Ok(Some((region, position, look_at)))
    }

    /// Check whether a session belongs to a user genuinely returning to
    /// the foreign grid.
    pub async fn is_agent_coming_home(
        &self,
        session_id: Uuid,
        external_name: &str,
    ) -> HgResult<bool> {
        let sid = session_id.to_string();
        let map = self
            .call(
                "agent_is_coming_home",
                &[("sessionID", &sid), ("externalName", external_name)],
            )
            .await?;
        Ok(map
            .get("result")
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(false))
    }

    /// Get a user's service URL map (HomeURI, InventoryServerURI, etc.).
    pub async fn get_server_urls(
        &self,
        user_id: Uuid,
    ) -> HgResult<HashMap<String, String>> {
        let uid = user_id.to_string();
        let map = self
            .call("get_server_urls", &[("userID", &uid)])
            .await?;
        // The response contains service URLs as direct key-value pairs.
        // Filter out the "result" key.
        Ok(map
            .into_iter()
            .filter(|(k, _)| k != "result")
            .collect())
    }

    /// Get profile information about a user.
    pub async fn get_user_info(
        &self,
        user_id: Uuid,
    ) -> HgResult<HashMap<String, String>> {
        let uid = user_id.to_string();
        self.call("get_user_info", &[("userID", &uid)]).await
    }

    /// Locate a user -- returns the grid URI they are visiting, or empty
    /// string if they are home.
    pub async fn locate_user(&self, user_id: Uuid) -> HgResult<String> {
        let uid = user_id.to_string();
        let map = self
            .call("locate_user", &[("userID", &uid)])
            .await?;
        Ok(map.get("URL").cloned().unwrap_or_default())
    }

    /// Get the Universal User Identifier for a target user on behalf of
    /// the requester.
    pub async fn get_uui(
        &self,
        requester_id: Uuid,
        target_user_id: Uuid,
    ) -> HgResult<String> {
        let rid = requester_id.to_string();
        let tid = target_user_id.to_string();
        let map = self
            .call("get_uui", &[("userID", &rid), ("targetUserID", &tid)])
            .await?;
        Ok(map.get("UUI").cloned().unwrap_or_default())
    }

    /// Look up a user by first and last name.
    pub async fn get_uuid(
        &self,
        first: &str,
        last: &str,
    ) -> HgResult<Option<Uuid>> {
        let map = self
            .call("get_uuid", &[("first", first), ("last", last)])
            .await?;
        Ok(map
            .get("UUID")
            .and_then(|s| Uuid::parse_str(s).ok())
            .filter(|u| !u.is_nil()))
    }

    /// Notify the foreign grid that a user has logged out.
    pub async fn logout_agent(
        &self,
        user_id: Uuid,
        session_id: Uuid,
    ) -> HgResult<()> {
        let uid = user_id.to_string();
        let sid = session_id.to_string();
        let _ = self
            .call("logout_agent", &[("userID", &uid), ("sessionID", &sid)])
            .await?;
        Ok(())
    }
}

/// Parse an OpenMetaverse-style Vector3 string: `<x, y, z>`.
fn parse_vector3(s: &str) -> Vector3 {
    let s = s.trim().trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() == 3 {
        let x = parts[0].trim().parse::<f32>().unwrap_or(128.0);
        let y = parts[1].trim().parse::<f32>().unwrap_or(128.0);
        let z = parts[2].trim().parse::<f32>().unwrap_or(0.0);
        Vector3 { x, y, z }
    } else {
        Vector3::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_normalises_url() {
        let c = UserAgentServiceConnector::new("http://example.com:9000");
        assert!(c.server_url.ends_with('/'));
    }

    #[test]
    fn parse_vector3_valid() {
        let v = parse_vector3("<128.5, 64.0, 25.3>");
        assert!((v.x - 128.5).abs() < 0.01);
        assert!((v.y - 64.0).abs() < 0.01);
        assert!((v.z - 25.3).abs() < 0.01);
    }

    #[test]
    fn parse_vector3_empty() {
        let v = parse_vector3("");
        assert!((v.x - 128.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_vector3_malformed() {
        let v = parse_vector3("not a vector");
        assert!((v.x - 128.0).abs() < f32::EPSILON);
    }
}
