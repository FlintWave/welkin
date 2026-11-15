//! Capabilities (Seed/EventQueue) client.
//!
//! Handles fetching capability URLs from the seed capability endpoint and
//! long-polling the EventQueueGet capability for server-pushed events.

use std::collections::HashMap;

use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};

use crate::error::RealmWalkerError;

/// Fetch the list of capability URLs from the seed capability endpoint.
///
/// The seed cap URL comes from `LoginResponse.seed_capability`. The viewer
/// POSTs an LLSD XML body listing the capability names it wants, and the
/// server responds with a map of name -> URL.
pub async fn fetch_seed_caps(
    http: &HttpClient,
    seed_url: &str,
    requested: &[&str],
) -> Result<HashMap<String, String>, RealmWalkerError> {
    let body = build_llsd_request(requested);

    tracing::debug!(url = %seed_url, count = requested.len(), "fetching seed capabilities");

    let resp = http
        .post(seed_url)
        .header("Content-Type", "application/llsd+xml")
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

    parse_llsd_map(&text)
}

/// A single event from the EventQueueGet response.
#[derive(Debug, Clone)]
pub struct EqgEvent {
    /// The event name (e.g. "EstablishAgentCommunication", "TeleportFinish").
    pub message: String,
    /// The event body as a JSON value.
    pub body: serde_json::Value,
}

/// Long-poll the EventQueueGet capability.
///
/// The viewer sends a POST with `{"ack": <id>, "done": false}` and the server
/// holds the connection until events are available or a timeout occurs.
/// Returns the events and the ack ID to send on the next poll.
pub async fn poll_event_queue(
    http: &HttpClient,
    eqg_url: &str,
    ack_id: Option<u64>,
) -> Result<(Vec<EqgEvent>, u64), RealmWalkerError> {
    let req = EqgRequest {
        ack: ack_id.unwrap_or(0),
        done: false,
    };

    let resp = http
        .post(eqg_url)
        .json(&req)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(RealmWalkerError::CircuitFailed(format!(
            "EQG returned status {}",
            resp.status()
        )));
    }

    let body: EqgResponse = resp.json().await?;

    let events = body
        .events
        .unwrap_or_default()
        .into_iter()
        .map(|raw| EqgEvent {
            message: raw.message,
            body: raw.body,
        })
        .collect();

    Ok((events, body.id))
}

/// Request body for EventQueueGet.
#[derive(Serialize)]
struct EqgRequest {
    ack: u64,
    done: bool,
}

/// Response body from EventQueueGet.
#[derive(Deserialize)]
struct EqgResponse {
    events: Option<Vec<EqgEventRaw>>,
    id: u64,
}

#[derive(Deserialize)]
struct EqgEventRaw {
    message: String,
    body: serde_json::Value,
}

// ---------------------------------------------------------------------------
// LLSD XML helpers
// ---------------------------------------------------------------------------

/// Build an LLSD XML request body listing the requested capability names.
///
/// The format is:
/// ```xml
/// <llsd><array>
///   <string>CapName1</string>
///   <string>CapName2</string>
/// </array></llsd>
/// ```
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
/// The expected format is:
/// ```xml
/// <llsd><map>
///   <key>CapName</key><string>URL</string>
///   ...
/// </map></llsd>
/// ```
fn parse_llsd_map(xml: &str) -> Result<HashMap<String, String>, RealmWalkerError> {
    let mut reader = Reader::from_str(xml);
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
        // An unexpected structure still returns an empty map.
        let xml = "<llsd><array><string>hello</string></array></llsd>";
        let map = parse_llsd_map(xml).expect("should parse");
        assert!(map.is_empty());
    }

    #[test]
    fn parse_llsd_map_ignores_non_string_values() {
        // If a value inside the map is not a <string>, the key is discarded
        // because in_string never fires.
        let xml = r#"<llsd><map>
            <key>SomeCap</key><integer>42</integer>
            <key>GoodCap</key><string>http://ok</string>
        </map></llsd>"#;
        let map = parse_llsd_map(xml).expect("should parse");
        // SomeCap had an <integer> value, so it was skipped.
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
