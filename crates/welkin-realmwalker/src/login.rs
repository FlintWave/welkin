// SPDX-License-Identifier: LGPL-3.0-or-later
//! XMLRPC login to an OpenSimulator grid.
//!
//! Performs an HTTP POST to the grid's login URI with method
//! `login_to_simulator`. The password is sent as `$1$` + hex MD5.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use uuid::Uuid;

use crate::config::GridConfig;
use crate::error::{LoginFailure, RealmWalkerError};

/// Parsed fields from a successful XMLRPC login response.
#[derive(Debug, Clone)]
pub struct LoginResponse {
    pub agent_id: Uuid,
    pub session_id: Uuid,
    pub secure_session_id: Uuid,
    pub circuit_code: u32,
    pub sim_ip: String,
    pub sim_port: u16,
    pub seed_capability: String,
    pub region_x: u32,
    pub region_y: u32,
    pub first_name: String,
    pub last_name: String,
    pub look_at: String,
    pub message: String,
}

/// Perform an XMLRPC `login_to_simulator` call against the grid.
///
/// On success, returns the parsed `LoginResponse` with the circuit details
/// needed to open a UDP connection to the assigned simulator.
pub async fn login(
    config: &GridConfig,
    http: &reqwest::Client,
) -> Result<LoginResponse, RealmWalkerError> {
    let body = build_login_xml(config);

    tracing::debug!(uri = %config.login_uri, first = %config.first_name, last = %config.last_name, "sending XMLRPC login");

    let resp = http
        .post(&config.login_uri)
        .header("Content-Type", "text/xml")
        .body(body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        return Err(LoginFailure::GridUnavailable.into());
    }

    let text = resp.text().await?;
    tracing::debug!(len = text.len(), "received login response");

    let fields = parse_xmlrpc_response(&text)?;
    build_login_response(&fields)
}

/// Construct the XMLRPC request body.
fn build_login_xml(config: &GridConfig) -> String {
    let hashed = config.hashed_password();
    let start = config.start.to_login_string();
    let platform = if cfg!(target_os = "linux") {
        "Lin"
    } else if cfg!(target_os = "macos") {
        "Mac"
    } else {
        "Win"
    };

    let mut xml = String::with_capacity(1024);
    xml.push_str("<?xml version=\"1.0\"?>\n<methodCall>\n<methodName>login_to_simulator</methodName>\n<params><param><value><struct>\n");

    write_member(&mut xml, "first", &config.first_name);
    write_member(&mut xml, "last", &config.last_name);
    write_member(&mut xml, "passwd", &hashed);
    write_member(&mut xml, "start", &start);
    write_member(&mut xml, "channel", "WelkinRealmWalker");
    write_member(&mut xml, "version", env!("CARGO_PKG_VERSION"));
    write_member(&mut xml, "platform", platform);
    write_member(&mut xml, "mac", "00:00:00:00:00:00");
    write_member(&mut xml, "id0", "00000000-0000-0000-0000-000000000000");
    write_member(&mut xml, "agree_to_tos", "true");
    write_member(&mut xml, "read_critical", "true");

    xml.push_str("</struct></value></param></params>\n</methodCall>");
    xml
}

/// Append a single `<member><name>...</name><value><string>...</string></value></member>` entry.
fn write_member(xml: &mut String, name: &str, value: &str) {
    xml.push_str("<member><name>");
    xml.push_str(name);
    xml.push_str("</name><value><string>");
    xml_escape_into(xml, value);
    xml.push_str("</string></value></member>\n");
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

/// Parse the XMLRPC response XML into a flat map of member name -> string value.
///
/// The response structure is:
/// ```xml
/// <methodResponse><params><param><value><struct>
///   <member><name>key</name><value><type>val</type></value></member>
///   ...
/// </struct></value></param></params></methodResponse>
/// ```
///
/// We extract every `<member>` pair regardless of the value's inner type tag
/// (`<string>`, `<i4>`, `<int>`, etc.) and store the text content as a string.
fn parse_xmlrpc_response(xml: &str) -> Result<HashMap<String, String>, RealmWalkerError> {
    let mut reader = Reader::from_str(xml);
    let mut map = HashMap::new();

    // State machine: we track when we're inside a <member>, collecting the
    // <name> text and then the value text.
    let mut in_member = false;
    let mut in_name = false;
    let mut in_value = false;
    let mut value_depth: u32 = 0;
    let mut current_name = String::new();
    let mut current_value = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag = e.local_name();
                let tag_bytes = tag.as_ref();
                if tag_bytes == b"member" {
                    in_member = true;
                    current_name.clear();
                    current_value.clear();
                } else if in_member && tag_bytes == b"name" {
                    in_name = true;
                } else if in_member && tag_bytes == b"value" {
                    in_value = true;
                    value_depth = 0;
                } else if in_value {
                    // Inside the <value>, there may be a type tag like
                    // <string>, <i4>, <int>, etc. We just track depth and
                    // collect text.
                    value_depth += 1;
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = e.local_name();
                let tag_bytes = tag.as_ref();
                if tag_bytes == b"member" {
                    if !current_name.is_empty() {
                        map.insert(
                            std::mem::take(&mut current_name),
                            std::mem::take(&mut current_value),
                        );
                    }
                    in_member = false;
                    in_name = false;
                    in_value = false;
                } else if in_name && tag_bytes == b"name" {
                    in_name = false;
                } else if in_value && tag_bytes == b"value" {
                    in_value = false;
                } else if in_value && value_depth > 0 {
                    value_depth -= 1;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_name {
                    match e.unescape() {
                        Ok(cow) => current_name.push_str(&cow),
                        Err(err) => {
                            return Err(LoginFailure::XmlParseError(
                                format!("name unescape: {err}"),
                            )
                            .into());
                        }
                    }
                } else if in_value {
                    match e.unescape() {
                        Ok(cow) => current_value.push_str(&cow),
                        Err(err) => {
                            return Err(LoginFailure::XmlParseError(
                                format!("value unescape: {err}"),
                            )
                            .into());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(
                    LoginFailure::XmlParseError(format!("xml parse: {err}")).into()
                );
            }
            _ => {}
        }
    }

    Ok(map)
}

/// Build a `LoginResponse` from the parsed XMLRPC fields.
fn build_login_response(
    fields: &HashMap<String, String>,
) -> Result<LoginResponse, RealmWalkerError> {
    // Check for explicit login failure.
    let login_flag = fields.get("login").map(|s| s.as_str()).unwrap_or("false");
    if login_flag != "true" {
        let message = fields
            .get("message")
            .cloned()
            .unwrap_or_default();
        let reason = fields
            .get("reason")
            .map(|s| s.as_str())
            .unwrap_or("");
        if !reason.is_empty() {
            return Err(LoginFailure::from_reason(reason).into());
        }
        if !message.is_empty() {
            return Err(LoginFailure::from_reason(&message).into());
        }
        return Err(LoginFailure::Other("login returned false with no reason".into()).into());
    }

    let agent_id = parse_uuid(fields, "agent_id")?;
    let session_id = parse_uuid(fields, "session_id")?;
    let secure_session_id = parse_uuid(fields, "secure_session_id")?;
    let circuit_code = parse_u32(fields, "circuit_code")?;
    let sim_ip = get_field(fields, "sim_ip")?;
    let sim_port = parse_u32(fields, "sim_port")? as u16;
    let seed_capability = get_field(fields, "seed_capability")?;
    let region_x = parse_u32(fields, "region_x")?;
    let region_y = parse_u32(fields, "region_y")?;
    let first_name = fields
        .get("first_name")
        .cloned()
        .unwrap_or_default();
    let last_name = fields
        .get("last_name")
        .cloned()
        .unwrap_or_default();
    let look_at = fields
        .get("look_at")
        .cloned()
        .unwrap_or_default();
    let message = fields
        .get("message")
        .cloned()
        .unwrap_or_default();

    tracing::debug!(
        %agent_id,
        %session_id,
        %sim_ip,
        sim_port,
        circuit_code,
        "login successful"
    );

    Ok(LoginResponse {
        agent_id,
        session_id,
        secure_session_id,
        circuit_code,
        sim_ip,
        sim_port,
        seed_capability,
        region_x,
        region_y,
        first_name,
        last_name,
        look_at,
        message,
    })
}

/// Extract a required string field.
fn get_field(
    fields: &HashMap<String, String>,
    name: &str,
) -> Result<String, RealmWalkerError> {
    fields
        .get(name)
        .cloned()
        .ok_or_else(|| {
            LoginFailure::XmlParseError(format!("missing field: {name}")).into()
        })
}

/// Parse a UUID field.
fn parse_uuid(
    fields: &HashMap<String, String>,
    name: &str,
) -> Result<Uuid, RealmWalkerError> {
    let s = get_field(fields, name)?;
    Uuid::parse_str(&s).map_err(|e| {
        RealmWalkerError::LoginFailed(LoginFailure::XmlParseError(format!(
            "{name}: invalid uuid: {e}"
        )))
    })
}

/// Parse a u32 field.
///
/// OpenSim XMLRPC uses `<i4>` (signed 32-bit) for values like `circuit_code`
/// that are logically unsigned on the wire. Negative values (e.g. `-660431009`)
/// are valid -- they're the signed representation of the unsigned circuit code.
/// We try u32 first, then fall back to i32 and reinterpret as u32.
fn parse_u32(
    fields: &HashMap<String, String>,
    name: &str,
) -> Result<u32, RealmWalkerError> {
    let s = get_field(fields, name)?;
    s.parse::<u32>().or_else(|_| {
        s.parse::<i32>().map(|v| v as u32)
    }).map_err(|e| {
        RealmWalkerError::LoginFailed(LoginFailure::XmlParseError(format!(
            "{name}: invalid u32: {e}"
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_login_xml_contains_required_fields() {
        let config = GridConfig::new(
            "http://localhost:8002",
            "Test",
            "User",
            "password",
        );
        let xml = build_login_xml(&config);
        assert!(xml.contains("<methodName>login_to_simulator</methodName>"));
        assert!(xml.contains("<name>first</name>"));
        assert!(xml.contains("<string>Test</string>"));
        assert!(xml.contains("<name>last</name>"));
        assert!(xml.contains("<string>User</string>"));
        assert!(xml.contains("<name>passwd</name>"));
        assert!(xml.contains("$1$"));
        assert!(xml.contains("<name>start</name>"));
        assert!(xml.contains("<string>last</string>"));
        assert!(xml.contains("<name>channel</name>"));
        assert!(xml.contains("<string>WelkinRealmWalker</string>"));
        assert!(xml.contains("<name>agree_to_tos</name>"));
        assert!(xml.contains("<name>read_critical</name>"));
    }

    #[test]
    fn xml_escape_handles_special_chars() {
        let mut out = String::new();
        xml_escape_into(&mut out, "a<b>c&d\"e");
        assert_eq!(out, "a&lt;b&gt;c&amp;d&quot;e");
    }

    #[test]
    fn parse_successful_response() {
        let xml = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>login</name><value><string>true</string></value></member>
<member><name>agent_id</name><value><string>12345678-1234-1234-1234-123456789abc</string></value></member>
<member><name>session_id</name><value><string>abcdef01-2345-6789-abcd-ef0123456789</string></value></member>
<member><name>secure_session_id</name><value><string>00000000-0000-0000-0000-000000000001</string></value></member>
<member><name>circuit_code</name><value><i4>987654</i4></value></member>
<member><name>sim_ip</name><value><string>192.168.1.100</string></value></member>
<member><name>sim_port</name><value><i4>9000</i4></value></member>
<member><name>seed_capability</name><value><string>http://192.168.1.100:9000/CAPS/seed</string></value></member>
<member><name>region_x</name><value><i4>256000</i4></value></member>
<member><name>region_y</name><value><i4>256000</i4></value></member>
<member><name>first_name</name><value><string>Test</string></value></member>
<member><name>last_name</name><value><string>User</string></value></member>
<member><name>look_at</name><value><string>[r0,r1,r0]</string></value></member>
<member><name>message</name><value><string>Welcome</string></value></member>
</struct></value></param></params></methodResponse>"#;

        let fields = parse_xmlrpc_response(xml).expect("parse should succeed");
        let resp = build_login_response(&fields).expect("build should succeed");
        assert_eq!(
            resp.agent_id,
            Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
        );
        assert_eq!(
            resp.session_id,
            Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789").unwrap()
        );
        assert_eq!(resp.circuit_code, 987654);
        assert_eq!(resp.sim_ip, "192.168.1.100");
        assert_eq!(resp.sim_port, 9000);
        assert_eq!(resp.region_x, 256000);
        assert_eq!(resp.region_y, 256000);
        assert_eq!(resp.first_name, "Test");
        assert_eq!(resp.last_name, "User");
        assert_eq!(resp.look_at, "[r0,r1,r0]");
        assert_eq!(resp.message, "Welcome");
        assert_eq!(
            resp.seed_capability,
            "http://192.168.1.100:9000/CAPS/seed"
        );
    }

    #[test]
    fn parse_failed_login() {
        let xml = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>login</name><value><string>false</string></value></member>
<member><name>reason</name><value><string>Could not authenticate your avatar. Please check your username and password, and check the grid if problems persist.</string></value></member>
<member><name>message</name><value><string>Error: bad password</string></value></member>
</struct></value></param></params></methodResponse>"#;

        let fields = parse_xmlrpc_response(xml).expect("parse should succeed");
        let err = build_login_response(&fields).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("login failed"),
            "error should indicate login failure: {msg}"
        );
    }

    #[test]
    fn parse_response_missing_login_field() {
        let xml = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>message</name><value><string>oops</string></value></member>
</struct></value></param></params></methodResponse>"#;

        let fields = parse_xmlrpc_response(xml).expect("parse should succeed");
        let err = build_login_response(&fields).unwrap_err();
        // login field missing defaults to "false", so it should fail with the message
        let msg = err.to_string();
        assert!(msg.contains("login failed"), "got: {msg}");
    }

    #[test]
    fn parse_integer_value_types() {
        // OpenSim may use <i4> or <int> for integer values
        let xml = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>login</name><value><string>true</string></value></member>
<member><name>agent_id</name><value><string>00000000-0000-0000-0000-000000000001</string></value></member>
<member><name>session_id</name><value><string>00000000-0000-0000-0000-000000000002</string></value></member>
<member><name>secure_session_id</name><value><string>00000000-0000-0000-0000-000000000003</string></value></member>
<member><name>circuit_code</name><value><int>42</int></value></member>
<member><name>sim_ip</name><value><string>127.0.0.1</string></value></member>
<member><name>sim_port</name><value><int>9000</int></value></member>
<member><name>seed_capability</name><value><string>http://127.0.0.1:9000/CAPS/seed</string></value></member>
<member><name>region_x</name><value><int>256000</int></value></member>
<member><name>region_y</name><value><int>256000</int></value></member>
</struct></value></param></params></methodResponse>"#;

        let fields = parse_xmlrpc_response(xml).expect("parse should succeed");
        let resp = build_login_response(&fields).expect("build should succeed");
        assert_eq!(resp.circuit_code, 42);
        assert_eq!(resp.sim_port, 9000);
    }

    #[test]
    fn parse_negative_circuit_code() {
        // OpenSim returns circuit_code as signed i4; -660431009 is a valid
        // signed representation of unsigned 3634536287.
        let xml = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
<member><name>login</name><value><string>true</string></value></member>
<member><name>agent_id</name><value><string>00000000-0000-0000-0000-000000000001</string></value></member>
<member><name>session_id</name><value><string>00000000-0000-0000-0000-000000000002</string></value></member>
<member><name>secure_session_id</name><value><string>00000000-0000-0000-0000-000000000003</string></value></member>
<member><name>circuit_code</name><value><i4>-660431009</i4></value></member>
<member><name>sim_ip</name><value><string>127.0.0.1</string></value></member>
<member><name>sim_port</name><value><i4>9000</i4></value></member>
<member><name>seed_capability</name><value><string>http://127.0.0.1:9000/CAPS/seed</string></value></member>
<member><name>region_x</name><value><i4>256000</i4></value></member>
<member><name>region_y</name><value><i4>256000</i4></value></member>
</struct></value></param></params></methodResponse>"#;

        let fields = parse_xmlrpc_response(xml).expect("parse should succeed");
        let resp = build_login_response(&fields).expect("build should succeed");
        // -660431009 as u32 = 3634536287
        assert_eq!(resp.circuit_code, (-660431009_i32) as u32);
    }
}
