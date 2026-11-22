// SPDX-License-Identifier: LGPL-3.0-or-later

//! Remote HTTP connectors for talking to an external grid's Robust services.

pub mod accounts;
pub mod asset;
pub mod avatar;
pub mod gandalf;
pub mod grid;
pub mod friends;
pub mod groups;
pub mod inventory;
pub mod network;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RemoteError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server returned error: {0}")]
    Server(String),
    #[error("failed to parse response: {0}")]
    Parse(String),
}

/// Extract the text content of a single child element by tag name from a
/// `<ServerResponse>` XML body.  Returns `None` when the element is absent or
/// its text is empty.
pub(crate) fn extract_text(xml: &str, tag: &str) -> Option<String> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(xml);
    let mut depth: Vec<String> = Vec::new();
    let mut capture = false;
    let mut capture_depth = 0usize;
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if !capture && name == tag {
                    capture = true;
                    capture_depth = depth.len() + 1;
                }
                depth.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if !capture && name == tag {
                    return None;
                }
            }
            Ok(Event::End(_)) => {
                let closes_target = capture && depth.len() == capture_depth;
                depth.pop();
                if closes_target {
                    return if buf.is_empty() { None } else { Some(buf) };
                }
            }
            Ok(Event::Text(t)) if capture => {
                if let Ok(text) = t.unescape() {
                    buf.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract_text;

    #[test]
    fn nested_result_list_finds_first_and_last_name() {
        let xml = r#"<ServerResponse><result type="List"><FirstName>Ada</FirstName><LastName>Lovelace</LastName></result></ServerResponse>"#;
        assert_eq!(extract_text(xml, "FirstName"), Some("Ada".to_string()));
        assert_eq!(extract_text(xml, "LastName"), Some("Lovelace".to_string()));
    }

    #[test]
    fn flat_reply() {
        let xml = r#"<ServerResponse><Result>Success</Result></ServerResponse>"#;
        assert_eq!(extract_text(xml, "Result"), Some("Success".to_string()));
    }

    #[test]
    fn first_of_duplicate_tags_wins() {
        let xml = r#"<root><Item>first</Item><Item>second</Item></root>"#;
        assert_eq!(extract_text(xml, "Item"), Some("first".to_string()));
    }

    #[test]
    fn absent_tag_returns_none() {
        let xml = r#"<ServerResponse><Result>Success</Result></ServerResponse>"#;
        assert_eq!(extract_text(xml, "Missing"), None);
    }

    #[test]
    fn empty_elements_return_none() {
        assert_eq!(extract_text("<Message></Message>", "Message"), None);
        assert_eq!(extract_text("<Message/>", "Message"), None);
    }

    #[test]
    fn unescapes_entities() {
        let xml = r#"<X><Message>Ada &amp; Bob</Message></X>"#;
        assert_eq!(extract_text(xml, "Message"), Some("Ada & Bob".to_string()));
    }

    #[test]
    fn tag_name_must_match_exactly() {
        let xml = r#"<X><FirstName>Ada</FirstName></X>"#;
        assert_eq!(extract_text(xml, "Name"), None);
    }

    #[test]
    fn malformed_or_truncated_xml_does_not_panic() {
        assert_eq!(extract_text("<ServerResponse><Result>Success", "Result"), None);
    }
}
