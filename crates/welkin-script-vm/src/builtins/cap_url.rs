// SPDX-License-Identifier: GPL-3.0-or-later
//! Cap URL ll* functions — llRequestURL, llRequestSecureURL, llReleaseURL.
//!
//! Scripts request inbound HTTP URLs from the region's cap URL server.
//! Requests go through `ScriptContext::cap_url_sender`; responses arrive
//! as `ScriptEvent::HttpRequest`.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use super::ScriptContext;

#[derive(Debug, Clone)]
pub enum CapUrlRequest {
    Register {
        script_id: Uuid,
        prim_id: Uuid,
        request_id: Uuid,
        secure: bool,
    },
    Release {
        url: String,
    },
}

pub fn register_cap_url(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llRequestURL() -> key
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let request_id = Uuid::new_v4();
            let ctx_r = ctx.read();
            if let Some(ref tx) = ctx_r.cap_url_sender {
                let req = CapUrlRequest::Register {
                    script_id: ctx_r.script_id,
                    prim_id: ctx_r.prim_id,
                    request_id,
                    secure: false,
                };
                let _ = tx.send(req);
            }
            Ok(request_id.to_string())
        })?;
        lua.globals().set("llRequestURL", f)?;
    }

    // llRequestSecureURL() -> key
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let request_id = Uuid::new_v4();
            let ctx_r = ctx.read();
            if let Some(ref tx) = ctx_r.cap_url_sender {
                let req = CapUrlRequest::Register {
                    script_id: ctx_r.script_id,
                    prim_id: ctx_r.prim_id,
                    request_id,
                    secure: true,
                };
                let _ = tx.send(req);
            }
            Ok(request_id.to_string())
        })?;
        lua.globals().set("llRequestSecureURL", f)?;
    }

    // llReleaseURL(url)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, url: String| {
            let mut ctx = ctx.write();
            if let Some(ref tx) = ctx.cap_url_sender {
                let req = CapUrlRequest::Release { url: url.clone() };
                let _ = tx.send(req);
            }
            ctx.active_urls.retain(|u| u != &url);
            Ok(())
        })?;
        lua.globals().set("llReleaseURL", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_url_sends_register() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            cap_url_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_cap_url(&lua, &ctx).unwrap();
        let returned: String = lua.load("return llRequestURL()").eval().unwrap();
        assert!(Uuid::parse_str(&returned).is_ok());
        match rx.try_recv().unwrap() {
            CapUrlRequest::Register {
                secure,
                request_id,
                ..
            } => {
                assert!(!secure);
                assert_eq!(request_id.to_string(), returned);
            }
            _ => panic!("expected Register"),
        }
    }

    #[test]
    fn request_secure_url_sends_register() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            cap_url_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_cap_url(&lua, &ctx).unwrap();
        let returned: String = lua.load("return llRequestSecureURL()").eval().unwrap();
        assert!(Uuid::parse_str(&returned).is_ok());
        match rx.try_recv().unwrap() {
            CapUrlRequest::Register { secure, .. } => assert!(secure),
            _ => panic!("expected Register"),
        }
    }

    #[test]
    fn release_url_sends_release_and_removes() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            cap_url_sender: Some(tx),
            active_urls: vec!["http://example.com/cap".to_string()],
            ..Default::default()
        }));
        let lua = Lua::new();
        register_cap_url(&lua, &ctx).unwrap();
        lua.load(r#"llReleaseURL("http://example.com/cap")"#)
            .exec()
            .unwrap();
        match rx.try_recv().unwrap() {
            CapUrlRequest::Release { url } => {
                assert_eq!(url, "http://example.com/cap");
            }
            _ => panic!("expected Release"),
        }
        assert!(ctx.read().active_urls.is_empty());
    }
}
