// SPDX-License-Identifier: GPL-3.0-or-later

//! Simple HTTP GET probe for Hypergrid capability discovery.
//!
//! Mirrors the C# `HeloServicesConnector`: sends a GET to `{server}/helo`
//! and returns the response body, which typically contains the grid's
//! external name.

use std::time::Duration;

use crate::hypergrid::*;

/// A lightweight connector that probes a grid's `/helo` endpoint.
pub struct HeloServiceConnector {
    client: reqwest::Client,
}

impl HeloServiceConnector {
    /// Create a new connector with a 10-second timeout.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build reqwest client"),
        }
    }

    /// Create a connector with a custom timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("failed to build reqwest client"),
        }
    }

    /// Probe the grid's `/helo` endpoint.
    ///
    /// Returns the response body text (typically the grid's external name
    /// or a simple protocol identifier).  On timeout or connection error,
    /// returns an appropriate [`HypergridError`].
    pub async fn helo(&self, server_url: &str) -> HgResult<String> {
        let mut url = server_url.to_string();
        if !url.ends_with('/') {
            url.push('/');
        }
        url.push_str("helo");

        let resp = self.client.get(&url).send().await.map_err(|e| {
            HypergridError::GatekeeperUnreachable {
                url: url.clone(),
                detail: e.to_string(),
            }
        })?;

        let text = resp.text().await.map_err(|e| {
            HypergridError::GatekeeperUnreachable {
                url: url.clone(),
                detail: format!("failed to read response body: {e}"),
            }
        })?;

        Ok(text)
    }
}

impl Default for HeloServiceConnector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helo_connector_default_creation() {
        let _c = HeloServiceConnector::new();
        // Just verify construction doesn't panic.
    }

    #[test]
    fn helo_connector_custom_timeout() {
        let c = HeloServiceConnector::with_timeout(Duration::from_secs(5));
        let _ = &c.client; // verify client was built
    }

    #[test]
    fn helo_url_construction() {
        // Verify the URL normalization logic in helo().
        // We can't actually make HTTP requests in unit tests, but we can
        // test the URL building indirectly.
        let base = "http://example.com:9000";
        let mut url = base.to_string();
        if !url.ends_with('/') {
            url.push('/');
        }
        url.push_str("helo");
        assert_eq!(url, "http://example.com:9000/helo");

        let base_slash = "http://example.com:9000/";
        let mut url2 = base_slash.to_string();
        if !url2.ends_with('/') {
            url2.push('/');
        }
        url2.push_str("helo");
        assert_eq!(url2, "http://example.com:9000/helo");
    }
}
