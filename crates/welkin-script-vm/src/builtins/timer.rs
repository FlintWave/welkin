// SPDX-License-Identifier: LGPL-3.0-or-later
//! Timer ll* functions — llSetTimerEvent, llGetTime, llResetTime.

use std::sync::Arc;
use std::time::Instant;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

pub fn register_timer(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llSetTimerEvent(interval)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, interval: f64| {
            let ctx = ctx.read();
            if let Some(ref tx) = ctx.timer_sender {
                let _ = tx.send((ctx.script_id, interval));
            }
            Ok(())
        })?;
        lua.globals().set("llSetTimerEvent", f)?;
    }

    // llGetTime() -> float
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.start_time.elapsed().as_secs_f64())
        })?;
        lua.globals().set("llGetTime", f)?;
    }

    // llResetTime()
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let mut ctx = ctx.write();
            ctx.start_time = Instant::now();
            Ok(())
        })?;
        lua.globals().set("llResetTime", f)?;
    }

    Ok(())
}
