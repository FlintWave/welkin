// SPDX-License-Identifier: LGPL-3.0-or-later

//! Remote HTTP connectors for talking to an external grid's Robust services.

pub mod accounts;
pub mod asset;
pub mod avatar;
pub mod grid;
pub mod groups;
pub mod inventory;

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
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if depth.len() == 1 && name == tag {
                    capture = true;
                }
                depth.push(name);
            }
            Ok(Event::End(_)) => {
                depth.pop();
                if capture && depth.len() == 1 {
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
