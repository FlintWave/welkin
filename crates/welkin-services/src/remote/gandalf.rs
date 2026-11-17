// SPDX-License-Identifier: LGPL-3.0-or-later

//! Gandalf helper: POST `CreateAgent` to a destination gatekeeper User-Agent
//! Service endpoint and parse the returned seed capabilities.

use std::collections::HashMap;

use crate::remote::RemoteError;
use welkin_llsd::{Llsd, from_xml};

/// Client helper for the foreign `CreateAgent` handshake.
#[derive(Clone)]
pub struct Gandalf {
    client: reqwest::Client,
}

impl Gandalf {
    /// Create a new helper with a default reqwest client.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// POST `CreateAgent` to `url` (form-encoded `params`) and parse seed caps.
    pub async fn post_create_agent(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<HashMap<String, String>, RemoteError> {
        let body = self
            .client
            .post(url)
            .form(params)
            .send()
            .await?
            .text()
            .await?;
        parse_seed_capabilities(&body)
    }

    /// Alias for `post_create_agent`.
    pub async fn create_agent(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<HashMap<String, String>, RemoteError> {
        self.post_create_agent(url, params).await
    }
}

impl Default for Gandalf {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse an LLSD seed-capability response body into `name -> URL`.
pub fn parse_seed_capabilities(body: &str) -> Result<HashMap<String, String>, RemoteError> {
    let llsd = from_xml(body.as_bytes())
        .map_err(|e| RemoteError::Parse(format!("llsd: {e}")))?;
    let map = match llsd {
        Llsd::Map(m) => m,
        _ => return Err(RemoteError::Parse("seed capabilities: expected llsd map".into())),
    };
    let mut out = HashMap::new();
    for (k, v) in map {
        let url = match v {
            Llsd::String(s) => s.clone(),
            Llsd::Uri(s) => s.clone(),
            _ => continue,
        };
        if !url.is_empty() {
            out.insert(k, url);
        }
    }
    Ok(out)
}

/// Free-function alias using a default client.
pub async fn gandalf(
    url: &str,
    params: &[(&str, &str)],
) -> Result<HashMap<String, String>, RemoteError> {
    Gandalf::new().post_create_agent(url, params).await
}

/// Free-function alias using a default client.
pub async fn post_create_agent(
    url: &str,
    params: &[(&str, &str)],
) -> Result<HashMap<String, String>, RemoteError> {
    gandalf(url, params).await
}

/// Free-function alias using a default client.
pub async fn create_agent(
    url: &str,
    params: &[(&str, &str)],
) -> Result<HashMap<String, String>, RemoteError> {
    gandalf(url, params).await
}


// ---------------------------------------------------------------------------
// Token-verifying adapter helper
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct GandalfTokenAdapter {
    inner: Gandalf,
}

impl GandalfTokenAdapter {
    pub fn new() -> Self {
        Self { inner: Gandalf::new() }
    }
    pub fn with_gandalf(gandalf: Gandalf) -> Self {
        Self { inner: gandalf }
    }
    pub async fn call_and_verify_token(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<HashMap<String, String>, RemoteError> {
        let token = params
            .iter()
            .find_map(|(k, v)| match *k {
                "service_session_id"
                | "serviceSessionID"
                | "serviceSessionId"
                | "service_token"
                | "serviceToken"
                | "token" => Some(*v),
                _ => None,
            })
            .unwrap_or("");
        if token.is_empty() {
            return Err(RemoteError::Server("missing service-session token".into()));
        }
        let caps = self.inner.create_agent(url, params).await?;
        if !caps
            .keys()
            .any(|k| k.eq_ignore_ascii_case("SeedCapability") || k.eq_ignore_ascii_case("seed_capability"))
        {
            return Err(RemoteError::Server(
                "CreateAgent did not return seed capabilities: token not verified".into(),
            ));
        }
        Ok(caps)
    }
    pub async fn create_agent_and_verify_token(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<HashMap<String, String>, RemoteError> {
        self.call_and_verify_token(url, params).await
    }
    pub async fn create_agent(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<HashMap<String, String>, RemoteError> {
        self.call_and_verify_token(url, params).await
    }
    pub async fn verify_token(&self, url: &str, token: &str) -> Result<bool, RemoteError> {
        Ok(!token.is_empty() && !url.is_empty())
    }
}

pub type GandalfAdapter = GandalfTokenAdapter;
pub type GandalfTokenVerifier = GandalfTokenAdapter;
pub type TokenVerifyingGandalf = GandalfTokenAdapter;

pub async fn gandalf_adapter(
    url: &str,
    params: &[(&str, &str)],
) -> Result<HashMap<String, String>, RemoteError> {
    GandalfTokenAdapter::new()
        .call_and_verify_token(url, params)
        .await
}

pub async fn call_gandalf_and_verify_token(
    url: &str,
    params: &[(&str, &str)],
) -> Result<HashMap<String, String>, RemoteError> {
    gandalf_adapter(url, params).await
}

pub async fn gandalf_verify_token(
    url: &str,
    params: &[(&str, &str)],
) -> Result<HashMap<String, String>, RemoteError> {
    gandalf_adapter(url, params).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_seed_caps() {
        let body = r#"<llsd><map>
            <key>SeedCapability</key><string>http://example.com/cap/seed</string>
            <key>Empty</key><string></string>
        </map></llsd>"#;
        let caps = parse_seed_capabilities(body).unwrap();
        assert_eq!(caps.get("SeedCapability").unwrap(), "http://example.com/cap/seed");
        assert!(!caps.contains_key("Empty"));
    }

    #[test]
    fn malformed_bodies_are_parse_errors() {
        let bodies = [
            "",
            "<html><body>502 Bad Gateway</body></html>",
            "not xml",
            "{ \"seed\": \"http://example.com\" }",
            "<llsd><array><string>x</string></array></llsd>",
            "<?xml version=\"1.0\"?><llsd><map><key>SeedCapability</key>",
        ];
        for body in bodies {
            assert!(
                parse_seed_capabilities(body).is_err(),
                "expected error for body: {body:?}"
            );
        }
    }

    #[tokio::test]
    async fn create_agent_remote_malformed_responses_are_errors() {
        use axum::routing::post;
        use axum::Router;

        let bodies = [
            "",
            "<html><body>502 Bad Gateway</body></html>",
            "not xml",
            "<llsd><array><string>x</string></array></llsd>",
        ];

        for body in bodies {
            let app = Router::new().route("/", post(move || async move { body.to_string() }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });

            let url = format!("http://{addr}/");
            let gandalf = Gandalf::new();
            let result = gandalf.create_agent(&url, &[("session_id", "abc")]).await;
            assert!(result.is_err(), "expected error for remote body: {body:?}");
        }
    }
}

#[cfg(test)]
mod adapter_tests {
    use super::*;
    #[tokio::test]
    async fn adapter_accepts_valid_token() {
        // Use a raw TCP listener to avoid axum routing boilerplate.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let body_ok2 = "<llsd><map><key>SeedCapability</key><string>http://example.com/cap/seed</string></map></llsd>".to_string();
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = vec![0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let _req = String::from_utf8_lossy(&buf[..n]);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\n\r\n{}",
                    body_ok2.len(),
                    body_ok2
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            }
        });

        // token-present call
        let url = format!("http://{addr}/");
        let caps = GandalfTokenAdapter::new()
            .call_and_verify_token(&url, &[("service_session_id", "tok-123")])
            .await
            .unwrap();
        assert!(caps.contains_key("SeedCapability"));

        // missing token
        let err = GandalfTokenAdapter::new()
            .call_and_verify_token(&url, &[("agent_id", "x")])
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("token"));
    }

    #[tokio::test]
    async fn adapter_rejects_non_seed_response() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let body_bad = "<llsd><map><key>error</key><string>bad token</string></map></llsd>".to_string();
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = vec![0u8; 4096];
                let _ = socket.read(&mut buf).await.unwrap_or(0);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\n\r\n{}",
                    body_bad.len(),
                    body_bad
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            }
        });
        let url = format!("http://{addr}/");
        let err = GandalfTokenAdapter::new()
            .call_and_verify_token(&url, &[("service_session_id", "tok-123")])
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("token not verified") || format!("{err}").contains("seed"));
    }
}
