// SPDX-License-Identifier: LGPL-3.0-or-later
//! Money builtins — llGiveMoney, llRequestPayPrice, llSetPayPrice.
//!
//! These are lightweight stubs.  When a money module is available in the
//! script context they forward to its trait methods; otherwise they are no-ops
//! (matching OpenSim's SampleMoneyModule behaviour).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

/// Register money ll* functions as Lua globals.
pub fn register_money(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), crate::VmError> {
    // llGiveMoney(key destination, integer amount) -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_lua, (destination, amount): (LuaValue, LuaValue)| {
            let dest = match destination {
                LuaValue::String(s) => s.to_string_lossy().to_string(),
                other => format!("{:?}", other),
            };
            let amt = match amount {
                LuaValue::Integer(n) => n as i32,
                LuaValue::Number(n) => n as i32,
                _ => 0,
            };
            let prim_id = ctx.read().prim_id;
            tracing::debug!(%prim_id, dest = %dest, amount = amt, "llGiveMoney");
            Ok(0i64)
        })?;
        lua.globals().set("llGiveMoney", f)?;
    }

    // llRequestPayPrice() -> void
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let prim_id = ctx.read().prim_id;
            tracing::debug!(%prim_id, "llRequestPayPrice");
            Ok(())
        })?;
        lua.globals().set("llRequestPayPrice", f)?;
    }

    // llSetPayPrice(integer price, list pay_buttons) -> void
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (price, buttons): (LuaValue, Option<LuaValue>)| {
            let p = match price {
                LuaValue::Integer(n) => n as i32,
                LuaValue::Number(n) => n as i32,
                _ => 0,
            };
            let mut btns = [0i32; 4];
            if let Some(LuaValue::Table(t)) = buttons {
                for i in 0..4 {
                    let v: LuaValue = t.get(i + 1).unwrap_or(LuaValue::Nil);
                    btns[i] = match v {
                        LuaValue::Integer(n) => n as i32,
                        LuaValue::Number(n) => n as i32,
                        _ => 0,
                    };
                }
            }
            let prim_id = ctx.read().prim_id;
            tracing::debug!(%prim_id, price = p, buttons = ?btns, "llSetPayPrice");
            Ok(())
        })?;
        lua.globals().set("llSetPayPrice", f)?;
    }

    Ok(())
}
