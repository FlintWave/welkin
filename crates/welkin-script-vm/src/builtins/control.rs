// SPDX-License-Identifier: LGPL-3.0-or-later
//! Control ll* functions — llTakeControls, llReleaseControls.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use super::ScriptContext;

#[derive(Debug, Clone)]
pub enum ControlRequest {
    Take {
        script_id: Uuid,
        controls: u32,
        accept: bool,
        pass_on: bool,
    },
    Release {
        script_id: Uuid,
    },
}

pub const CONTROL_FWD: u32 = 0x00000001;
pub const CONTROL_BACK: u32 = 0x00000002;
pub const CONTROL_LEFT: u32 = 0x00000004;
pub const CONTROL_RIGHT: u32 = 0x00000008;
pub const CONTROL_UP: u32 = 0x00000010;
pub const CONTROL_DOWN: u32 = 0x00000020;
pub const CONTROL_ROT_LEFT: u32 = 0x00000100;
pub const CONTROL_ROT_RIGHT: u32 = 0x00000200;
pub const CONTROL_LBUTTON: u32 = 0x10000000;
pub const CONTROL_ML_LBUTTON: u32 = 0x40000000;

pub const PERMISSION_TAKE_CONTROLS: u32 = 0x00000004;

pub fn register_control(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llTakeControls(controls, accept, pass_on)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (controls, accept, pass_on): (i64, i64, i64)| {
            let ctx = ctx.read();
            if ctx.permissions_granted & PERMISSION_TAKE_CONTROLS == 0 {
                return Ok(());
            }
            if let Some(ref tx) = ctx.control_sender {
                let _ = tx.send(ControlRequest::Take {
                    script_id: ctx.script_id,
                    controls: controls as u32,
                    accept: accept != 0,
                    pass_on: pass_on != 0,
                });
            }
            Ok(())
        })?;
        lua.globals().set("llTakeControls", f)?;
    }

    // llReleaseControls()
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            if let Some(ref tx) = ctx.control_sender {
                let _ = tx.send(ControlRequest::Release {
                    script_id: ctx.script_id,
                });
            }
            Ok(())
        })?;
        lua.globals().set("llReleaseControls", f)?;
    }

    // Register CONTROL_* constants as Lua globals
    lua.globals().set("CONTROL_FWD", CONTROL_FWD as i64)?;
    lua.globals().set("CONTROL_BACK", CONTROL_BACK as i64)?;
    lua.globals().set("CONTROL_LEFT", CONTROL_LEFT as i64)?;
    lua.globals().set("CONTROL_RIGHT", CONTROL_RIGHT as i64)?;
    lua.globals().set("CONTROL_UP", CONTROL_UP as i64)?;
    lua.globals().set("CONTROL_DOWN", CONTROL_DOWN as i64)?;
    lua.globals()
        .set("CONTROL_ROT_LEFT", CONTROL_ROT_LEFT as i64)?;
    lua.globals()
        .set("CONTROL_ROT_RIGHT", CONTROL_ROT_RIGHT as i64)?;
    lua.globals()
        .set("CONTROL_LBUTTON", CONTROL_LBUTTON as i64)?;
    lua.globals()
        .set("CONTROL_ML_LBUTTON", CONTROL_ML_LBUTTON as i64)?;

    Ok(())
}
