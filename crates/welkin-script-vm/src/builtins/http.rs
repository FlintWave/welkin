// SPDX-License-Identifier: LGPL-3.0-or-later
//! HTTP ll* functions — llHTTPRequest, llHTTPResponse, llSetContentType.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use super::ScriptContext;

pub const HTTP_METHOD: i32 = 0;
pub const HTTP_MIMETYPE: i32 = 1;
pub const HTTP_BODY_MAXLENGTH: i32 = 2;
pub const HTTP_VERIFY_CERT: i32 = 3;
pub const HTTP_VERBOSE_THROTTLE: i32 = 4;
pub const HTTP_CUSTOM_HEADER: i32 = 5;
pub const HTTP_PRAGMA_NO_CACHE: i32 = 6;
pub const HTTP_BODY_TRUNCATED: i32 = 0;

#[derive(Debug, Clone)]
pub struct HttpRequestConfig {
    pub method: String,
    pub mimetype: String,
    pub body_maxlength: i32,
    pub verify_cert: bool,
    pub verbose_throttle: bool,
    pub custom_headers: Vec<(String, String)>,
    pub pragma_no_cache: bool,
}

impl Default for HttpRequestConfig {
    fn default() -> Self {
        Self {
            method: "GET".into(),
            mimetype: "text/plain;charset=utf-8".into(),
            body_maxlength: 2048,
            verify_cert: true,
            verbose_throttle: true,
            custom_headers: Vec::new(),
            pragma_no_cache: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpOutboundRequest {
    pub script_id: Uuid,
    pub prim_id: Uuid,
    pub request_id: Uuid,
    pub url: String,
    pub config: HttpRequestConfig,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub request_id: String,
    pub status: i32,
    pub body: String,
}

pub fn register_http(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    let globals = lua.globals();
    globals.set("HTTP_METHOD", HTTP_METHOD)?;
    globals.set("HTTP_MIMETYPE", HTTP_MIMETYPE)?;
    globals.set("HTTP_BODY_MAXLENGTH", HTTP_BODY_MAXLENGTH)?;
    globals.set("HTTP_VERIFY_CERT", HTTP_VERIFY_CERT)?;
    globals.set("HTTP_VERBOSE_THROTTLE", HTTP_VERBOSE_THROTTLE)?;
    globals.set("HTTP_CUSTOM_HEADER", HTTP_CUSTOM_HEADER)?;
    globals.set("HTTP_PRAGMA_NO_CACHE", HTTP_PRAGMA_NO_CACHE)?;
    globals.set("HTTP_BODY_TRUNCATED", HTTP_BODY_TRUNCATED)?;

    // llHTTPRequest(url, params, body) -> key
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (url, params, body): (String, LuaTable, String)| {
                let mut ctx = ctx.write();
                if ctx.http_pending_count >= 25 {
                    return Ok(String::new());
                }

                let mut config = HttpRequestConfig::default();
                let len = params.raw_len() as u32;
                let mut i = 1u32;
                let mut pending_header_name: Option<String> = None;
                while i <= len {
                    let code: i64 = params.raw_get(i)?;
                    let value: String = params.raw_get(i + 1)?;
                    i += 2;

                    match code as i32 {
                        HTTP_METHOD => config.method = value,
                        HTTP_MIMETYPE => config.mimetype = value,
                        HTTP_BODY_MAXLENGTH => {
                            config.body_maxlength = value.parse().unwrap_or(2048);
                        }
                        HTTP_VERIFY_CERT => config.verify_cert = value != "0",
                        HTTP_VERBOSE_THROTTLE => config.verbose_throttle = value != "0",
                        HTTP_CUSTOM_HEADER => {
                            if let Some(name) = pending_header_name.take() {
                                config.custom_headers.push((name, value));
                            } else {
                                pending_header_name = Some(value);
                            }
                        }
                        HTTP_PRAGMA_NO_CACHE => config.pragma_no_cache = value != "0",
                        _ => {}
                    }
                }

                let request_id = Uuid::new_v4();
                ctx.http_pending_count += 1;

                if let Some(ref tx) = ctx.http_sender {
                    let req = HttpOutboundRequest {
                        script_id: ctx.script_id,
                        prim_id: ctx.prim_id,
                        request_id,
                        url,
                        config,
                        body,
                    };
                    let _ = tx.send(req);
                }

                Ok(request_id.to_string())
            },
        )?;
        globals.set("llHTTPRequest", f)?;
    }

    // llHTTPResponse(request_id, status, body)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (request_id, status, body): (String, i64, String)| {
            let ctx = ctx.read();
            if let Some(ref tx) = ctx.http_response_sender {
                let resp = HttpResponse {
                    request_id,
                    status: status as i32,
                    body,
                };
                let _ = tx.send(resp);
            }
            Ok(())
        })?;
        globals.set("llHTTPResponse", f)?;
    }

    // llSetContentType(request_id, content_type)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_request_id, content_type): (String, i64)| {
            let mut ctx = ctx.write();
            ctx.http_content_type = match content_type {
                0 => "text/plain".into(),
                1 => "text/html".into(),
                2 => "application/xml".into(),
                3 => "application/json".into(),
                4 => "application/x-www-form-urlencoded".into(),
                _ => "text/plain".into(),
            };
            Ok(())
        })?;
        globals.set("llSetContentType", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx_with_http() -> (
        Arc<RwLock<ScriptContext>>,
        std::sync::mpsc::Receiver<HttpOutboundRequest>,
        std::sync::mpsc::Receiver<HttpResponse>,
    ) {
        let (tx_req, rx_req) = std::sync::mpsc::channel();
        let (tx_resp, rx_resp) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::nil(),
            prim_id: Uuid::from_u128(0xabcd),
            prim_name: "Test Prim".into(),
            owner_id: Uuid::from_u128(0x1234),
            position: [128.0, 128.0, 25.0],
            http_sender: Some(tx_req),
            http_response_sender: Some(tx_resp),
            ..Default::default()
        }));
        (ctx, rx_req, rx_resp)
    }

    #[test]
    fn ll_http_request_sends_through_channel() {
        let (ctx, rx_req, _rx_resp) = test_ctx_with_http();
        let lua = Lua::new();
        register_http(&lua, &ctx).unwrap();

        let key: String = lua
            .load(r#"return llHTTPRequest("http://example.com", {}, "hello")"#)
            .eval()
            .unwrap();
        assert!(!key.is_empty());

        let req = rx_req.try_recv().unwrap();
        assert_eq!(req.url, "http://example.com");
        assert_eq!(req.body, "hello");
        assert_eq!(req.config.method, "GET");
        assert_eq!(req.request_id.to_string(), key);
    }

    #[test]
    fn ll_http_request_custom_method_and_mimetype() {
        let (ctx, rx_req, _rx_resp) = test_ctx_with_http();
        let lua = Lua::new();
        register_http(&lua, &ctx).unwrap();

        let script = r#"
            return llHTTPRequest("http://example.com/api", {
                HTTP_METHOD, "POST",
                HTTP_MIMETYPE, "application/json"
            }, '{"key":"value"}')
        "#;
        let key: String = lua.load(script).eval().unwrap();
        assert!(!key.is_empty());

        let req = rx_req.try_recv().unwrap();
        assert_eq!(req.config.method, "POST");
        assert_eq!(req.config.mimetype, "application/json");
    }

    #[test]
    fn ll_http_request_custom_headers() {
        let (ctx, rx_req, _rx_resp) = test_ctx_with_http();
        let lua = Lua::new();
        register_http(&lua, &ctx).unwrap();

        let script = r#"
            return llHTTPRequest("http://example.com", {
                HTTP_CUSTOM_HEADER, "X-My-Header",
                HTTP_CUSTOM_HEADER, "my-value"
            }, "")
        "#;
        let _key: String = lua.load(script).eval().unwrap();

        let req = rx_req.try_recv().unwrap();
        assert_eq!(req.config.custom_headers.len(), 1);
        assert_eq!(req.config.custom_headers[0].0, "X-My-Header");
        assert_eq!(req.config.custom_headers[0].1, "my-value");
    }

    #[test]
    fn ll_http_request_max_pending() {
        let (ctx, _rx_req, _rx_resp) = test_ctx_with_http();
        ctx.write().http_pending_count = 25;
        let lua = Lua::new();
        register_http(&lua, &ctx).unwrap();

        let key: String = lua
            .load(r#"return llHTTPRequest("http://example.com", {}, "")"#)
            .eval()
            .unwrap();
        assert!(key.is_empty(), "should return empty key when at max pending");
    }

    #[test]
    fn ll_http_response_sends_through_channel() {
        let (ctx, _rx_req, rx_resp) = test_ctx_with_http();
        let lua = Lua::new();
        register_http(&lua, &ctx).unwrap();

        lua.load(r#"llHTTPResponse("req-123", 200, "OK")"#)
            .exec()
            .unwrap();

        let resp = rx_resp.try_recv().unwrap();
        assert_eq!(resp.request_id, "req-123");
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "OK");
    }
}
