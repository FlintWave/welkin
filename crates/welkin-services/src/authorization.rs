// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    Router,
    body::Bytes,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use quick_xml::{Reader, events::Event};

#[derive(Clone)]
pub struct AuthorizationServiceState;

pub fn router(state: AuthorizationServiceState) -> Router {
    Router::new()
        .route("/authorization", post(handle_authorization))
        .with_state(state)
}

fn xml_auth_response(authorized: bool, message: &str) -> Response {
    let auth_str = if authorized { "true" } else { "false" };
    let msg_escaped = quick_xml::escape::escape(message);
    let body = format!(
        "<?xml version=\"1.0\"?><AuthorizationResponse>\
         <IsAuthorized>{auth_str}</IsAuthorized>\
         <Message>{msg_escaped}</Message>\
         </AuthorizationResponse>"
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

struct AuthRequest {
    id: String,
    first_name: String,
    surname: String,
    region_id: String,
}

fn parse_auth_request(body: &[u8]) -> Option<AuthRequest> {
    let mut reader = Reader::from_reader(body);
    let mut buf = Vec::new();
    let mut current_tag = String::new();
    let mut req = AuthRequest {
        id: String::new(),
        first_name: String::new(),
        surname: String::new(),
        region_id: String::new(),
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "ID" => req.id = text,
                    "FirstName" => req.first_name = text,
                    "SurName" => req.surname = text,
                    "RegionID" => req.region_id = text,
                    _ => {}
                }
            }
            Ok(Event::End(_)) => {
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }

    Some(req)
}

async fn handle_authorization(body: Bytes) -> Response {
    let Some(_req) = parse_auth_request(&body) else {
        return xml_auth_response(false, "Invalid request");
    };

    tracing::debug!(id = %_req.id, name = %_req.first_name, "authorization request");

    xml_auth_response(true, "")
}
