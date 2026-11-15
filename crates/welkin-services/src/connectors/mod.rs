// SPDX-License-Identifier: GPL-3.0-or-later

//! Outbound HTTP client connectors for Hypergrid cross-grid communication.
//!
//! These connectors implement the client side of the Hypergrid protocol,
//! making XML-RPC or form-encoded POST requests to foreign grids'
//! Gatekeeper, UserAgent, and Friends services.

pub mod gatekeeper;
pub mod helo;
pub mod hg_friends;
pub mod user_agent;

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::hypergrid::HypergridError;

// ---------------------------------------------------------------------------
// Shared XML-RPC helpers
// ---------------------------------------------------------------------------

/// Build an XML-RPC request body from a method name and key-value string
/// parameters (serialised as a single `<struct>` inside `<params>`).
pub(crate) fn build_xmlrpc_request(method: &str, params: &[(&str, &str)]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\"?><methodCall>");
    xml.push_str("<methodName>");
    xml_escape_into(&mut xml, method);
    xml.push_str("</methodName>");
    xml.push_str("<params><param><value><struct>");
    for (k, v) in params {
        xml.push_str("<member><name>");
        xml_escape_into(&mut xml, k);
        xml.push_str("</name><value><string>");
        xml_escape_into(&mut xml, v);
        xml.push_str("</string></value></member>");
    }
    xml.push_str("</struct></value></param></params>");
    xml.push_str("</methodCall>");
    xml
}

/// Parse an XML-RPC `<methodResponse>` that contains a single `<struct>`
/// value (the OpenSim hashtable pattern).  Returns the key-value pairs,
/// or an error if the response is a fault.
pub(crate) fn parse_xmlrpc_response(body: &str) -> Result<HashMap<String, String>, HypergridError> {
    // Detect faults first.
    if let Some(fault) = try_parse_fault(body) {
        return Err(HypergridError::Internal(fault));
    }

    let mut map = HashMap::new();
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    #[derive(Debug, PartialEq)]
    enum State {
        Scanning,
        InMember,
        InName,
        InValue,
        InValueString,
    }

    let mut state = State::Scanning;
    let mut current_name = String::new();
    let mut current_value = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match tag.as_str() {
                    "member" => {
                        state = State::InMember;
                        current_name.clear();
                        current_value.clear();
                    }
                    "name" if state == State::InMember => {
                        state = State::InName;
                    }
                    "value" if state == State::InMember => {
                        state = State::InValue;
                    }
                    "string" | "int" | "i4" | "boolean" | "double" if state == State::InValue => {
                        state = State::InValueString;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(text) = t.unescape() {
                    match state {
                        State::InName => current_name.push_str(&text),
                        State::InValue | State::InValueString => {
                            current_value.push_str(&text);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match tag.as_str() {
                    "name" if state == State::InName => {
                        state = State::InMember;
                    }
                    "string" | "int" | "i4" | "boolean" | "double"
                        if state == State::InValueString =>
                    {
                        state = State::InValue;
                    }
                    "value" if state == State::InValue => {
                        state = State::InMember;
                    }
                    "member" if state == State::InMember => {
                        if !current_name.is_empty() {
                            map.insert(current_name.clone(), current_value.clone());
                        }
                        state = State::Scanning;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(HypergridError::Internal(format!(
                    "XML-RPC parse error: {e}"
                )));
            }
            _ => {}
        }
    }

    Ok(map)
}

/// Try to extract a fault string from an XML-RPC fault response.
fn try_parse_fault(body: &str) -> Option<String> {
    if !body.contains("<fault>") {
        return None;
    }
    // Simple extraction: look for faultString value.
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut in_fault = false;
    let mut found_fault_string_name = false;
    let mut in_value = false;
    let mut depth = 0u32;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                depth += 1;
                match tag.as_str() {
                    "fault" => in_fault = true,
                    "value" if in_fault && found_fault_string_name => {
                        in_value = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(text) = t.unescape() {
                    if in_fault && !found_fault_string_name && text.trim() == "faultString" {
                        found_fault_string_name = true;
                    } else if in_value {
                        return Some(text.to_string());
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    Some("XML-RPC fault (could not parse details)".to_string())
}

/// Send an XML-RPC request and parse the struct response.
pub(crate) async fn xmlrpc_call(
    client: &reqwest::Client,
    url: &str,
    method: &str,
    params: &[(&str, &str)],
) -> Result<HashMap<String, String>, HypergridError> {
    let body = build_xmlrpc_request(method, params);
    let resp = client
        .post(url)
        .header("Content-Type", "text/xml")
        .body(body)
        .send()
        .await?;
    let text = resp.text().await?;
    parse_xmlrpc_response(&text)
}

/// Send a form-encoded POST and return the raw XML body.
pub(crate) async fn form_post(
    client: &reqwest::Client,
    url: &str,
    params: &[(&str, &str)],
) -> Result<String, HypergridError> {
    let resp = client.post(url).form(params).send().await?;
    let text = resp.text().await?;
    Ok(text)
}

/// Parse a `<ServerResponse>` XML body for a single `<RESULT>` text.
pub(crate) fn parse_server_result(xml: &str) -> Option<String> {
    extract_tag_text(xml, "RESULT")
        .or_else(|| extract_tag_text(xml, "Result"))
        .or_else(|| extract_tag_text(xml, "result"))
}

/// Extract the text content of the first element with the given tag name.
pub(crate) fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut capture = false;
    let mut buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == tag {
                    capture = true;
                    buf.clear();
                }
            }
            Ok(Event::Text(t)) if capture => {
                if let Ok(text) = t.unescape() {
                    buf.push_str(&text);
                }
            }
            Ok(Event::End(e)) => {
                if capture {
                    let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                    if name == tag {
                        return if buf.is_empty() { None } else { Some(buf) };
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn xml_escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_xmlrpc_request() {
        let xml = build_xmlrpc_request("link_region", &[("region_name", "Test Region")]);
        assert!(xml.starts_with("<?xml version=\"1.0\"?>"));
        assert!(xml.contains("<methodName>link_region</methodName>"));
        assert!(xml.contains("<name>region_name</name>"));
        assert!(xml.contains("<string>Test Region</string>"));
        assert!(xml.contains("<struct>"));
        assert!(xml.contains("</methodCall>"));
    }

    #[test]
    fn test_parse_xmlrpc_response() {
        let body = r#"<?xml version="1.0"?>
<methodResponse>
  <params><param><value><struct>
    <member>
      <name>result</name>
      <value><string>true</string></value>
    </member>
    <member>
      <name>uuid</name>
      <value><string>12345678-1234-1234-1234-123456789abc</string></value>
    </member>
    <member>
      <name>handle</name>
      <value><string>1099511628032</string></value>
    </member>
  </struct></value></param></params>
</methodResponse>"#;

        let map = parse_xmlrpc_response(body).unwrap();
        assert_eq!(map.get("result").unwrap(), "true");
        assert_eq!(
            map.get("uuid").unwrap(),
            "12345678-1234-1234-1234-123456789abc"
        );
        assert_eq!(map.get("handle").unwrap(), "1099511628032");
    }

    #[test]
    fn test_parse_xmlrpc_fault() {
        let body = r#"<?xml version="1.0"?>
<methodResponse>
  <fault>
    <value><struct>
      <member>
        <name>faultCode</name>
        <value><int>4</int></value>
      </member>
      <member>
        <name>faultString</name>
        <value><string>Too many parameters.</string></value>
      </member>
    </struct></value>
  </fault>
</methodResponse>"#;

        let err = parse_xmlrpc_response(body).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Too many parameters") || msg.contains("fault"),
            "got: {msg}"
        );
    }

    #[test]
    fn test_xml_escape() {
        let xml = build_xmlrpc_request("test", &[("key", "a<b>&c\"d'e")]);
        assert!(xml.contains("a&lt;b&gt;&amp;c&quot;d&apos;e"));
    }

    #[test]
    fn test_extract_tag_text() {
        let xml =
            r#"<?xml version="1.0"?><ServerResponse><RESULT>Success</RESULT></ServerResponse>"#;
        assert_eq!(extract_tag_text(xml, "RESULT"), Some("Success".into()));
        assert_eq!(extract_tag_text(xml, "MISSING"), None);
    }

    #[test]
    fn test_parse_server_result() {
        let xml = r#"<?xml version="1.0"?><ServerResponse><Result>true</Result></ServerResponse>"#;
        assert_eq!(parse_server_result(xml), Some("true".into()));
    }
}
