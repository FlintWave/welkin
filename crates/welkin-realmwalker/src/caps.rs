// SPDX-License-Identifier: LGPL-3.0-or-later
//! Capability URL management and EventQueueGet long-poll client.
//!
//! After login, the simulator provides a seed capability URL. POSTing an LLSD
//! array of capability names to that URL returns a map of name to URL for each
//! available capability. The `EventQueueGet` capability is a long-poll endpoint
//! that delivers server-initiated events to the client.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader as XmlReader;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::RealmWalkerError;
use crate::events::{Event, EventBus};

/// Standard capabilities a bot requests from the seed URL. Requesting a broad
/// set ensures we discover every URL the simulator is willing to hand out.
const REQUESTED_CAPS: &[&str] = &[
    "EventQueueGet",
    "FetchInventory2",
    "FetchLib2",
    "GetTexture",
    "GetMesh",
    "GetMesh2",
    "UpdateScriptAgent",
    "NewFileAgentInventory",
    "UpdateAgentLanguage",
    "UpdateAgentInformation",
    "AvatarPickerSearch",
    "ViewerStats",
    "SimulatorFeatures",
    "AgentState",
    "ObjectMedia",
    "ObjectMediaNavigate",
    "RenderMaterials",
    "GetDisplayNames",
    "EnvironmentSettings",
];

/// Manages capability URLs obtained from a region's seed capability and runs
/// the EventQueueGet long-poll loop.
pub struct Capabilities {
    caps: Arc<RwLock<HashMap<String, String>>>,
    http: reqwest::Client,
}

impl Capabilities {
    /// Create a new empty capability manager.
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            caps: Arc::new(RwLock::new(HashMap::new())),
            http,
        }
    }

    /// Fetch capabilities from the seed URL provided at login.
    ///
    /// POSTs an LLSD XML array of requested capability names and stores every
    /// non-empty URL returned in the response map.
    pub async fn fetch_seed(&self, seed_url: &str) -> Result<(), RealmWalkerError> {
        let body = build_llsd_request(REQUESTED_CAPS);

        tracing::debug!(url = %seed_url, count = REQUESTED_CAPS.len(), "fetching seed capabilities");

        let resp = self
            .http
            .post(seed_url)
            .header("Content-Type", "application/llsd+xml")
            .timeout(Duration::from_secs(15))
            .body(body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(RealmWalkerError::CircuitFailed(format!(
                "seed cap returned status {}",
                resp.status()
            )));
        }

        let text = resp.text().await?;
        tracing::debug!(len = text.len(), "received seed cap response");

        let map = parse_llsd_map(&text)?;

        let mut caps = self.caps.write().await;
        for (name, url) in map {
            if !url.is_empty() {
                caps.insert(name, url);
            }
        }

        tracing::info!(count = caps.len(), "seed capabilities stored");
        Ok(())
    }

    /// Look up a capability URL by name.
    pub async fn get_cap(&self, name: &str) -> Option<String> {
        self.caps.read().await.get(name).cloned()
    }

    /// Run the EventQueueGet long-poll loop.
    ///
    /// This method blocks asynchronously and should be spawned as a background
    /// task. It repeatedly POSTs ack/done requests to the EQ capability URL,
    /// parses returned events, and dispatches them through the [`EventBus`].
    /// HTTP 502, 504, and client timeouts are retried transparently. The loop
    /// exits when the capability returns 404 (expired) or after too many
    /// consecutive errors.
    pub async fn run_event_queue(
        &self,
        event_bus: &EventBus,
    ) -> Result<(), RealmWalkerError> {
        let eq_url = match self.get_cap("EventQueueGet").await {
            Some(url) => url,
            None => {
                tracing::warn!("no EventQueueGet capability available");
                return Ok(());
            }
        };

        let mut ack_id: Option<u64> = None;
        let mut consecutive_errors: u32 = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 10;

        tracing::info!("starting event queue long-poll");

        loop {
            let req = EqgRequest {
                ack: ack_id.unwrap_or(0),
                done: false,
            };

            let result = self
                .http
                .post(&eq_url)
                .json(&req)
                .timeout(Duration::from_secs(30))
                .send()
                .await;

            let resp = match result {
                Ok(r) => r,
                Err(e) if e.is_timeout() || e.is_connect() => {
                    consecutive_errors += 1;
                    if consecutive_errors > MAX_CONSECUTIVE_ERRORS {
                        return Err(RealmWalkerError::CircuitFailed(format!(
                            "EQ failed after {MAX_CONSECUTIVE_ERRORS} consecutive errors: {e}"
                        )));
                    }
                    tracing::debug!(
                        errors = consecutive_errors,
                        "EQ poll timeout/connect, retrying"
                    );
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => {
                    return Err(RealmWalkerError::CircuitFailed(format!(
                        "EQ request error: {e}"
                    )));
                }
            };

            let status = resp.status();

            // 502/504 are normal long-poll timeouts from the server side.
            if status == reqwest::StatusCode::BAD_GATEWAY
                || status == reqwest::StatusCode::GATEWAY_TIMEOUT
            {
                consecutive_errors += 1;
                if consecutive_errors > MAX_CONSECUTIVE_ERRORS {
                    return Err(RealmWalkerError::CircuitFailed(format!(
                        "EQ returned {status} {MAX_CONSECUTIVE_ERRORS} times in a row"
                    )));
                }
                tracing::debug!(%status, "EQ server timeout, retrying");
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            // 404 means the capability has expired (region shutdown, etc.).
            if status == reqwest::StatusCode::NOT_FOUND {
                tracing::info!("EQ returned 404, capability expired");
                return Ok(());
            }

            if !status.is_success() {
                return Err(RealmWalkerError::CircuitFailed(format!(
                    "EQ returned HTTP {status}"
                )));
            }

            let body: EqgResponse = match resp.json().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to parse EQ response");
                    consecutive_errors += 1;
                    if consecutive_errors > MAX_CONSECUTIVE_ERRORS {
                        return Err(RealmWalkerError::CircuitFailed(format!(
                            "EQ parse failures exceeded limit: {e}"
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            // Update ack for the next request.
            ack_id = Some(body.id);

            // Dispatch events through the bus.
            if let Some(events) = body.events {
                for raw in events {
                    tracing::debug!(event = %raw.message, "EQ event received");
                    event_bus.send(Event::EventQueueEvent(raw.message, raw.body));
                }
            }

            consecutive_errors = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// EQ JSON types
// ---------------------------------------------------------------------------

/// Request body for the EventQueueGet long-poll.
#[derive(Serialize)]
struct EqgRequest {
    ack: u64,
    done: bool,
}

/// Response body from the EventQueueGet long-poll.
#[derive(Deserialize)]
struct EqgResponse {
    events: Option<Vec<EqgEventRaw>>,
    id: u64,
}

/// A single raw event inside an EQ response.
#[derive(Deserialize)]
struct EqgEventRaw {
    message: String,
    body: serde_json::Value,
}

// ---------------------------------------------------------------------------
// LLSD XML helpers
// ---------------------------------------------------------------------------

/// Build an LLSD XML request body listing the requested capability names.
fn build_llsd_request(names: &[&str]) -> String {
    let mut xml = String::with_capacity(128 + names.len() * 40);
    xml.push_str("<llsd><array>");
    for name in names {
        xml.push_str("<string>");
        xml_escape_into(&mut xml, name);
        xml.push_str("</string>");
    }
    xml.push_str("</array></llsd>");
    xml
}

/// Parse an LLSD XML map response into a `HashMap<String, String>`.
///
/// Expects the standard seed capability response format:
/// ```xml
/// <llsd><map>
///   <key>CapName</key><string>URL</string>
///   ...
/// </map></llsd>
/// ```
fn parse_llsd_map(xml: &str) -> Result<HashMap<String, String>, RealmWalkerError> {
    let mut reader = XmlReader::from_str(xml);
    let mut map = HashMap::new();

    let mut in_map = false;
    let mut in_key = false;
    let mut in_string = false;
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut have_key = false;

    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(ref e)) => {
                let tag = e.local_name();
                let tag_bytes = tag.as_ref();
                match tag_bytes {
                    b"map" => {
                        in_map = true;
                    }
                    b"key" if in_map => {
                        in_key = true;
                        current_key.clear();
                    }
                    b"string" if in_map && have_key => {
                        in_string = true;
                        current_value.clear();
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::End(ref e)) => {
                let tag = e.local_name();
                let tag_bytes = tag.as_ref();
                match tag_bytes {
                    b"map" => {
                        in_map = false;
                    }
                    b"key" if in_key => {
                        in_key = false;
                        have_key = true;
                    }
                    b"string" if in_string => {
                        in_string = false;
                        if have_key {
                            map.insert(
                                std::mem::take(&mut current_key),
                                std::mem::take(&mut current_value),
                            );
                            have_key = false;
                        }
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Text(ref e)) => {
                if in_key {
                    match e.unescape() {
                        Ok(cow) => current_key.push_str(&cow),
                        Err(err) => {
                            return Err(RealmWalkerError::CircuitFailed(format!(
                                "LLSD key unescape: {err}"
                            )));
                        }
                    }
                } else if in_string {
                    match e.unescape() {
                        Ok(cow) => current_value.push_str(&cow),
                        Err(err) => {
                            return Err(RealmWalkerError::CircuitFailed(format!(
                                "LLSD value unescape: {err}"
                            )));
                        }
                    }
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(err) => {
                return Err(RealmWalkerError::CircuitFailed(format!(
                    "LLSD xml parse: {err}"
                )));
            }
            _ => {}
        }
    }

    Ok(map)
}

/// Minimal XML text-node escaping.
fn xml_escape_into(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
}

// ---------------------------------------------------------------------------
// Standalone helpers used by client.rs
// ---------------------------------------------------------------------------

/// An event returned by [`poll_event_queue`].
pub struct EventQueueEvent {
    /// The event name (e.g. "TeleportFinish").
    pub message: String,
    /// The event body as free-form JSON.
    pub body: serde_json::Value,
}

/// Fetch seed capabilities from `seed_url`, requesting only the given `names`.
///
/// Returns a map of capability name to capability URL.
pub async fn fetch_seed_caps(
    http: &reqwest::Client,
    seed_url: &str,
    names: &[&str],
) -> Result<HashMap<String, String>, RealmWalkerError> {
    let body = build_llsd_request(names);

    let resp = http
        .post(seed_url)
        .header("Content-Type", "application/llsd+xml")
        .timeout(Duration::from_secs(15))
        .body(body)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(RealmWalkerError::CircuitFailed(format!(
            "seed cap returned status {}",
            resp.status()
        )));
    }

    let text = resp.text().await?;
    let map = parse_llsd_map(&text)?;

    // Filter out empty URLs.
    Ok(map.into_iter().filter(|(_, v)| !v.is_empty()).collect())
}

/// Long-poll the EventQueueGet capability once and return the events and ack id.
///
/// `ack_id` should be `None` for the first request, then `Some(id)` from the
/// previous response's `id` field.
pub async fn poll_event_queue(
    http: &reqwest::Client,
    eq_url: &str,
    ack_id: Option<u64>,
) -> Result<(Vec<EventQueueEvent>, u64), RealmWalkerError> {
    let req = EqgRequest {
        ack: ack_id.unwrap_or(0),
        done: false,
    };

    let resp = http
        .post(eq_url)
        .json(&req)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| RealmWalkerError::CircuitFailed(format!("EQ request: {e}")))?;

    if !resp.status().is_success() {
        return Err(RealmWalkerError::CircuitFailed(format!(
            "EQ returned HTTP {}",
            resp.status()
        )));
    }

    let body: EqgResponse = resp
        .json()
        .await
        .map_err(|e| RealmWalkerError::CircuitFailed(format!("EQ parse: {e}")))?;

    let events = body
        .events
        .unwrap_or_default()
        .into_iter()
        .map(|raw| EventQueueEvent {
            message: raw.message,
            body: raw.body,
        })
        .collect();

    Ok((events, body.id))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_llsd_request_single_cap() {
        let xml = build_llsd_request(&["EventQueueGet"]);
        assert_eq!(
            xml,
            "<llsd><array><string>EventQueueGet</string></array></llsd>"
        );
    }

    #[test]
    fn build_llsd_request_multiple_caps() {
        let xml = build_llsd_request(&["EventQueueGet", "FetchInventory2", "GetTexture"]);
        assert!(xml.starts_with("<llsd><array>"));
        assert!(xml.ends_with("</array></llsd>"));
        assert!(xml.contains("<string>EventQueueGet</string>"));
        assert!(xml.contains("<string>FetchInventory2</string>"));
        assert!(xml.contains("<string>GetTexture</string>"));
    }

    #[test]
    fn build_llsd_request_empty() {
        let xml = build_llsd_request(&[]);
        assert_eq!(xml, "<llsd><array></array></llsd>");
    }

    #[test]
    fn build_llsd_request_escapes_special_chars() {
        let xml = build_llsd_request(&["Test<Cap>"]);
        assert!(xml.contains("<string>Test&lt;Cap&gt;</string>"));
    }

    #[test]
    fn parse_llsd_map_basic() {
        let xml = r#"<llsd><map>
            <key>EventQueueGet</key><string>http://sim:9000/CAPS/eqg</string>
            <key>FetchInventory2</key><string>http://sim:9000/CAPS/inv</string>
        </map></llsd>"#;

        let map = parse_llsd_map(xml).expect("should parse");
        assert_eq!(map.len(), 2);
        assert_eq!(map["EventQueueGet"], "http://sim:9000/CAPS/eqg");
        assert_eq!(map["FetchInventory2"], "http://sim:9000/CAPS/inv");
    }

    #[test]
    fn parse_llsd_map_empty() {
        let xml = "<llsd><map></map></llsd>";
        let map = parse_llsd_map(xml).expect("should parse");
        assert!(map.is_empty());
    }

    #[test]
    fn parse_llsd_map_with_xml_entities() {
        let xml = r#"<llsd><map>
            <key>Cap&amp;Name</key><string>http://host/a&amp;b</string>
        </map></llsd>"#;
        let map = parse_llsd_map(xml).expect("should parse");
        assert_eq!(map.len(), 1);
        assert_eq!(map["Cap&Name"], "http://host/a&b");
    }

    #[test]
    fn parse_llsd_map_no_map_element() {
        let xml = "<llsd><array><string>hello</string></array></llsd>";
        let map = parse_llsd_map(xml).expect("should parse");
        assert!(map.is_empty());
    }

    #[test]
    fn parse_llsd_map_ignores_non_string_values() {
        let xml = r#"<llsd><map>
            <key>SomeCap</key><integer>42</integer>
            <key>GoodCap</key><string>http://ok</string>
        </map></llsd>"#;
        let map = parse_llsd_map(xml).expect("should parse");
        assert_eq!(map.len(), 1);
        assert_eq!(map["GoodCap"], "http://ok");
    }

    #[test]
    fn xml_escape_handles_all_special_chars() {
        let mut out = String::new();
        xml_escape_into(&mut out, "a<b>c&d\"e");
        assert_eq!(out, "a&lt;b&gt;c&amp;d&quot;e");
    }

    #[test]
    fn xml_escape_plain_string_unchanged() {
        let mut out = String::new();
        xml_escape_into(&mut out, "hello world");
        assert_eq!(out, "hello world");
    }

    #[test]
    fn eqg_request_serializes_correctly() {
        let req = EqgRequest { ack: 5, done: false };
        let json = serde_json::to_string(&req).expect("should serialize");
        assert!(json.contains("\"ack\":5"));
        assert!(json.contains("\"done\":false"));
    }

    #[test]
    fn eqg_response_deserializes_with_events() {
        let json = r#"{"events":[{"message":"TeleportFinish","body":{"Info":[]}}],"id":42}"#;
        let resp: EqgResponse = serde_json::from_str(json).expect("should parse");
        assert_eq!(resp.id, 42);
        let events = resp.events.expect("should have events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "TeleportFinish");
    }

    #[test]
    fn eqg_response_deserializes_without_events() {
        let json = r#"{"id":7}"#;
        let resp: EqgResponse = serde_json::from_str(json).expect("should parse");
        assert_eq!(resp.id, 7);
        assert!(resp.events.is_none());
    }

    #[test]
    fn eqg_response_deserializes_empty_events() {
        let json = r#"{"events":[],"id":10}"#;
        let resp: EqgResponse = serde_json::from_str(json).expect("should parse");
        assert_eq!(resp.id, 10);
        assert_eq!(resp.events.unwrap().len(), 0);
    }
}
