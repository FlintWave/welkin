// SPDX-License-Identifier: LGPL-3.0-or-later
//! Money builtins — llGiveMoney, llRequestPayPrice, llSetPayPrice.
//!
//! When a money module is held in the script context they forward to its
//! trait methods; otherwise they are no-ops (matching OpenSim's
//! SampleMoneyModule behaviour).  The region binds the module with
//! [`set_money_module`].

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use super::ScriptContext;
use welkin_modules::economy::MoneyModule;

pub type MoneyModuleHandle = Arc<dyn MoneyModule + Send + Sync>;

/// Bind (or clear) the money module held by a script context.
pub fn set_money_module(
    ctx: &Arc<RwLock<ScriptContext>>,
    module: Option<MoneyModuleHandle>,
) {
    ctx.write().money_module = module;
}

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
            let dest_uuid = Uuid::parse_str(&dest).unwrap_or(Uuid::nil());
            let prim_id = ctx.read().prim_id;
            let owner_id = ctx.read().owner_id;
            let mm = ctx.read().money_module.clone();
            if let Some(mm) = mm {
                match mm.script_give_money(prim_id, owner_id, dest_uuid, amt) {
                    Ok(()) => tracing::debug!(%prim_id, dest = %dest, amount = amt, "llGiveMoney"),
                    Err(e) => tracing::warn!(%prim_id, dest = %dest, amount = amt, error = %e, "llGiveMoney failed"),
                }
            } else {
                tracing::debug!(%prim_id, dest = %dest, amount = amt, "llGiveMoney (no money module)");
            }
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
            let mm = ctx.read().money_module.clone();
            if let Some(mm) = mm {
                mm.set_pay_price(prim_id, p, btns);
            }
            tracing::debug!(%prim_id, price = p, buttons = ?btns, "llSetPayPrice");
            Ok(())
        })?;
        lua.globals().set("llSetPayPrice", f)?;
    }

    Ok(())
}
