//! Server-side XML-RPC codec mapping values to and from LLSD.
//!
//! The viewer sends a `methodCall` (for example `login_to_simulator`) and the
//! server answers with a `methodResponse` or a fault. [`parse_call`] decodes
//! the request, [`build_response`] and [`build_fault`] encode the answer.

use aurora_llsd::xml::{self, Element};
use aurora_llsd::{Llsd, Map};
use base64::Engine as _;

#[derive(Debug, Clone, PartialEq)]
pub struct XmlRpcCall {
    pub method: String,
    pub params: Vec<Llsd>,
}

#[derive(Debug, thiserror::Error)]
pub enum XmlRpcError {
    #[error("xml parse error: {0}")]
    Xml(#[from] aurora_llsd::xml::XmlError),
    #[error("malformed call")]
    Malformed,
}

fn write_value(v: &Llsd, out: &mut String) {
    out.push_str("<value>");
    match v {
        Llsd::Undef => out.push_str("<string></string>"),
        Llsd::Boolean(b) => {
            out.push_str("<boolean>");
            out.push_str(if *b { "1" } else { "0" });
            out.push_str("</boolean>");
        }
        Llsd::Integer(i) => {
            out.push_str("<int>");
            out.push_str(&i.to_string());
            out.push_str("</int>");
        }
        Llsd::Real(r) => {
            out.push_str("<double>");
            out.push_str(&r.to_string());
            out.push_str("</double>");
        }
        Llsd::String(s) | Llsd::Uri(s) => {
            out.push_str("<string>");
            xml::escape(s, out);
            out.push_str("</string>");
        }
        Llsd::Uuid(u) => {
            out.push_str("<string>");
            out.push_str(&u.to_string());
            out.push_str("</string>");
        }
        Llsd::Date(d) => {
            out.push_str("<double>");
            out.push_str(&d.to_string());
            out.push_str("</double>");
        }
        Llsd::Binary(b) => {
            out.push_str("<base64>");
            out.push_str(&base64::engine::general_purpose::STANDARD.encode(b));
            out.push_str("</base64>");
        }
        Llsd::Array(a) => {
            out.push_str("<array><data>");
            for e in a {
                write_value(e, out);
            }
            out.push_str("</data></array>");
        }
        Llsd::Map(m) => {
            out.push_str("<struct>");
            for (k, e) in m {
                out.push_str("<member><name>");
                xml::escape(k, out);
                out.push_str("</name>");
                write_value(e, out);
                out.push_str("</member>");
            }
            out.push_str("</struct>");
        }
    }
    out.push_str("</value>");
}

fn parse_value(el: &Element) -> Llsd {
    let Some(child) = el.elements().next() else {
        return Llsd::String(el.text());
    };
    match child.name.as_str() {
        "string" => Llsd::String(child.text()),
        "int" | "i4" | "i8" => Llsd::Integer(child.text().trim().parse().unwrap_or(0)),
        "boolean" => Llsd::Boolean(child.text().trim() == "1"),
        "double" => Llsd::Real(child.text().trim().parse().unwrap_or(0.0)),
        "base64" => Llsd::Binary(
            base64::engine::general_purpose::STANDARD
                .decode(child.text().split_whitespace().collect::<String>())
                .unwrap_or_default(),
        ),
        "dateTime.iso8601" => Llsd::String(child.text()),
        "array" => {
            let mut a = Vec::new();
            if let Some(data) = child.child("data") {
                for v in data.elements().filter(|e| e.name == "value") {
                    a.push(parse_value(v));
                }
            }
            Llsd::Array(a)
        }
        "struct" => {
            let mut m = Map::new();
            for member in child.elements().filter(|e| e.name == "member") {
                let name = member.child("name").map(|n| n.text()).unwrap_or_default();
                let v = member.child("value").map(parse_value).unwrap_or_default();
                m.insert(name, v);
            }
            Llsd::Map(m)
        }
        "nil" => Llsd::Undef,
        _ => Llsd::String(child.text()),
    }
}

/// Parse a `methodCall` document into its method name and parameters.
pub fn parse_call(body: &[u8]) -> Result<XmlRpcCall, XmlRpcError> {
    let root = xml::parse(body)?;
    if root.name != "methodCall" {
        return Err(XmlRpcError::Malformed);
    }
    let method = root
        .child("methodName")
        .map(|m| m.text().trim().to_string())
        .filter(|m| !m.is_empty())
        .ok_or(XmlRpcError::Malformed)?;
    let params = root
        .child("params")
        .map(|p| {
            p.elements()
                .filter(|e| e.name == "param")
                .filter_map(|e| e.child("value"))
                .map(parse_value)
                .collect()
        })
        .unwrap_or_default();
    Ok(XmlRpcCall { method, params })
}

/// Build a `methodResponse` document with a single parameter.
pub fn build_response(value: &Llsd) -> String {
    let mut s = String::from("<?xml version=\"1.0\"?><methodResponse><params><param>");
    write_value(value, &mut s);
    s.push_str("</param></params></methodResponse>");
    s
}

/// Build a fault `methodResponse` document.
pub fn build_fault(code: i32, message: &str) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\"?><methodResponse><fault><value><struct>\
         <member><name>faultCode</name><value><int>",
    );
    s.push_str(&code.to_string());
    s.push_str("</int></value></member><member><name>faultString</name><value><string>");
    xml::escape(message, &mut s);
    s.push_str("</string></value></member></struct></value></fault></methodResponse>");
    s
}

#[derive(Debug)]
pub struct XmlRpcFault {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for XmlRpcFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "XML-RPC fault {}: {}", self.code, self.message)
    }
}

impl std::error::Error for XmlRpcFault {}

/// Build a `methodCall` document from method name and parameters.
pub fn build_call(method: &str, params: &[Llsd]) -> String {
    let mut s = String::from("<?xml version=\"1.0\"?><methodCall><methodName>");
    xml::escape(method, &mut s);
    s.push_str("</methodName><params>");
    for p in params {
        s.push_str("<param>");
        write_value(p, &mut s);
        s.push_str("</param>");
    }
    s.push_str("</params></methodCall>");
    s
}

/// Parse a `methodResponse` document, returning the result value or a fault.
pub fn parse_response(body: &[u8]) -> Result<Llsd, XmlRpcFault> {
    let root = xml::parse(body).map_err(|e| XmlRpcFault {
        code: -1,
        message: e.to_string(),
    })?;
    if let Some(fault) = root.child("fault") {
        if let Some(val) = fault.child("value") {
            let v = parse_value(val);
            return Err(XmlRpcFault {
                code: v["faultCode"].as_i32(),
                message: v["faultString"].to_string_value(),
            });
        }
        return Err(XmlRpcFault {
            code: -1,
            message: "malformed fault".into(),
        });
    }
    root.child("params")
        .and_then(|p| p.child("param"))
        .and_then(|p| p.child("value"))
        .map(parse_value)
        .ok_or_else(|| XmlRpcFault {
            code: -1,
            message: "no result in response".into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_llsd::llsd_map;

    #[test]
    fn roundtrip_call() {
        let p = llsd_map! {"first" => "Test", "n" => 3, "opts" => Llsd::Array(vec!["a".into()]), "b" => true};
        let doc = build_call("login_to_simulator", std::slice::from_ref(&p));
        let call = parse_call(doc.as_bytes()).unwrap();
        assert_eq!(call.method, "login_to_simulator");
        assert_eq!(call.params.len(), 1);
        assert_eq!(call.params[0]["first"].as_str(), "Test");
        assert_eq!(call.params[0]["n"].as_i32(), 3);
        assert_eq!(call.params[0]["opts"][0].as_str(), "a");
        assert!(call.params[0]["b"].as_bool());
    }

    #[test]
    fn real_login_request() {
        let body = br#"<?xml version="1.0"?>
<methodCall>
  <methodName>login_to_simulator</methodName>
  <params>
    <param>
      <value>
        <struct>
          <member><name>first</name><value><string>Test</string></value></member>
          <member><name>last</name><value><string>User</string></value></member>
          <member><name>passwd</name><value><string>$1$abcdef</string></value></member>
          <member><name>start</name><value><string>last</string></value></member>
          <member><name>channel</name><value><string>Aurora</string></value></member>
          <member><name>agree_to_tos</name><value><boolean>1</boolean></value></member>
          <member><name>read_critical</name><value><boolean>0</boolean></value></member>
          <member><name>last_exec_event</name><value><int>0</int></value></member>
          <member><name>options</name><value><array><data>
            <value><string>inventory-root</string></value>
            <value><string>buddy-list</string></value>
          </data></array></value></member>
        </struct>
      </value>
    </param>
  </params>
</methodCall>"#;
        let call = parse_call(body).unwrap();
        assert_eq!(call.method, "login_to_simulator");
        assert_eq!(call.params.len(), 1);
        let p = &call.params[0];
        assert_eq!(p["first"].as_str(), "Test");
        assert_eq!(p["last"].as_str(), "User");
        assert_eq!(p["passwd"].as_str(), "$1$abcdef");
        assert!(p["agree_to_tos"].as_bool());
        assert!(!p["read_critical"].as_bool());
        assert_eq!(p["options"][1].as_str(), "buddy-list");
    }

    #[test]
    fn response_roundtrip() {
        let v = llsd_map! {"login" => "true", "region_x" => 256000, "ok" => true, "pos" => 1.5};
        let doc = build_response(&v);
        let back = parse_response(doc.as_bytes()).unwrap();
        assert_eq!(back["login"].as_str(), "true");
        assert_eq!(back["region_x"].as_i32(), 256000);
        assert!(back["ok"].as_bool());
        assert_eq!(back["pos"].as_f64(), 1.5);
    }

    #[test]
    fn fault_is_valid() {
        let doc = build_fault(4, "Too <many> & \"bad\"");
        let err = parse_response(doc.as_bytes()).unwrap_err();
        assert_eq!(err.code, 4);
        assert_eq!(err.message, "Too <many> & \"bad\"");
    }

    #[test]
    fn empty_params() {
        let doc = "<methodCall><methodName>ping</methodName><params></params></methodCall>";
        let call = parse_call(doc.as_bytes()).unwrap();
        assert_eq!(call.method, "ping");
        assert!(call.params.is_empty());
        let doc = "<methodCall><methodName>ping</methodName></methodCall>";
        assert!(parse_call(doc.as_bytes()).unwrap().params.is_empty());
    }

    #[test]
    fn multiple_params() {
        let doc = build_call("m", &[Llsd::Integer(1), "two".into(), Llsd::Boolean(true)]);
        let call = parse_call(doc.as_bytes()).unwrap();
        assert_eq!(call.params.len(), 3);
        assert_eq!(call.params[0].as_i32(), 1);
        assert_eq!(call.params[1].as_str(), "two");
        assert!(call.params[2].as_bool());
    }

    #[test]
    fn nested_structs() {
        let inner = llsd_map! {"x" => 1, "y" => Llsd::Array(vec![llsd_map! {"z" => "deep"}])};
        let outer = llsd_map! {"inner" => inner};
        let doc = build_call("m", &[outer]);
        let call = parse_call(doc.as_bytes()).unwrap();
        assert_eq!(call.params[0]["inner"]["x"].as_i32(), 1);
        assert_eq!(call.params[0]["inner"]["y"][0]["z"].as_str(), "deep");
    }

    #[test]
    fn binary_values() {
        let bytes = vec![0u8, 1, 2, 254, 255];
        let p = llsd_map! {"blob" => Llsd::Binary(bytes.clone())};
        let doc = build_call("m", std::slice::from_ref(&p));
        let call = parse_call(doc.as_bytes()).unwrap();
        assert_eq!(call.params[0]["blob"], Llsd::Binary(bytes.clone()));
        let resp = build_response(&p);
        assert_eq!(
            parse_response(resp.as_bytes()).unwrap()["blob"],
            Llsd::Binary(bytes)
        );
    }

    #[test]
    fn rejects_malformed() {
        assert!(matches!(
            parse_call(b"<methodResponse></methodResponse>"),
            Err(XmlRpcError::Malformed)
        ));
        assert!(matches!(
            parse_call(b"<methodCall><params></params></methodCall>"),
            Err(XmlRpcError::Malformed)
        ));
        assert!(matches!(
            parse_call(b"<methodCall><oops"),
            Err(XmlRpcError::Xml(_))
        ));
    }
}
