// SPDX-License-Identifier: LGPL-3.0-or-later

//! HTTP client connector for a foreign grid's Gatekeeper service.
//!
//! Uses XML-RPC for `link_region` and `get_region` -- the initial
//! hyperlink handshake methods.

use uuid::Uuid;

use super::xmlrpc_call;
use crate::hypergrid::*;

/// Outbound HTTP client for a foreign Gatekeeper service.
pub struct GatekeeperServiceConnector {
    client: reqwest::Client,
    server_url: String,
}

impl GatekeeperServiceConnector {
    /// Create a new connector targeting `server_url` (e.g.
    /// `http://othergrid.example:9000/`).
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

    /// Request to link to a region on the foreign grid via the
    /// `link_region` XML-RPC method.
    ///
    /// Returns the linked region metadata on success.
    pub async fn link_region(&self, region_name: &str) -> HgResult<LinkedRegionInfo> {
        let params: Vec<(&str, &str)> = vec![("region_name", region_name)];

        let map = xmlrpc_call(&self.client, &self.server_url, "link_region", &params).await?;

        let result = map.get("result").map(|s| s.as_str()).unwrap_or("false");
        if !result.eq_ignore_ascii_case("true") {
            let reason = map
                .get("reason")
                .cloned()
                .unwrap_or_else(|| "link_region returned false".into());
            return Err(HypergridError::GatekeeperUnreachable {
                url: self.server_url.clone(),
                detail: reason,
            });
        }

        let region_id = map
            .get("uuid")
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or(Uuid::nil());
        let region_handle = map
            .get("handle")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let external_name = map.get("external_name").cloned().unwrap_or_default();
        let image_url = map.get("region_image").cloned().unwrap_or_default();
        let size_x = map
            .get("size_x")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(256);
        let size_y = map
            .get("size_y")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(256);

        Ok(LinkedRegionInfo {
            region_id,
            region_handle,
            external_name,
            image_url,
            size_x,
            size_y,
        })
    }

    /// Resolve a hyperlink region on the foreign grid via the `get_region`
    /// XML-RPC method.
    ///
    /// Returns `None` when the foreign gatekeeper signals that the region
    /// does not exist.
    pub async fn get_region(
        &self,
        region_id: Uuid,
        agent_id: Uuid,
        agent_home_uri: Option<&str>,
    ) -> HgResult<Option<(GridRegionInfo, Option<String>)>> {
        let region_uuid = region_id.to_string();
        let agent_str = agent_id.to_string();
        let mut params: Vec<(&str, &str)> =
            vec![("region_uuid", &region_uuid), ("agent_id", &agent_str)];
        let home_uri_owned;
        if let Some(uri) = agent_home_uri {
            home_uri_owned = uri.to_string();
            params.push(("agent_home_uri", &home_uri_owned));
        }

        let map = xmlrpc_call(&self.client, &self.server_url, "get_region", &params).await?;

        let result = map.get("result").map(|s| s.as_str()).unwrap_or("false");
        if !result.eq_ignore_ascii_case("true") {
            return Ok(None);
        }

        let message = map.get("message").cloned();

        let info = GridRegionInfo {
            region_id: map
                .get("uuid")
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or(Uuid::nil()),
            region_name: map.get("region_name").cloned().unwrap_or_default(),
            region_handle: 0, // Computed from x/y
            server_uri: map.get("server_uri").cloned().unwrap_or_default(),
            external_host_name: map.get("hostname").cloned().unwrap_or_default(),
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

        Ok(Some((info, message)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_normalises_url() {
        let c = GatekeeperServiceConnector::new("http://example.com:9000");
        assert!(c.server_url.ends_with('/'));

        let c2 = GatekeeperServiceConnector::new("http://example.com:9000/");
        assert!(c2.server_url.ends_with('/'));
        assert!(!c2.server_url.ends_with("//"));
    }
}

// ---------------------------------------------------------------------------
// CreateAgent endpoint health helpers
// ---------------------------------------------------------------------------

impl GatekeeperServiceConnector {
    fn create_agent_health_url(&self) -> String {
        format!("{}create_agent", self.server_url)
    }

    async fn create_agent_health_impl(&self) -> bool {
        let url = self.create_agent_health_url();
        match self.client.get(&url).send().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub async fn create_agent_health(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn create_agent_health_status(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn create_agent_endpoint_health(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn create_agent_endpoint_health_status(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn ping_create_agent(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn ping_create_agent_endpoint(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn check_create_agent_health(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn check_create_agent_endpoint(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn is_create_agent_healthy(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn create_agent_ping(&self) -> bool { self.create_agent_health_impl().await }
    pub async fn create_agent_status(&self) -> bool { self.create_agent_health_impl().await }
}
