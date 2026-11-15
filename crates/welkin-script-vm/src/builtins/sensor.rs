// SPDX-License-Identifier: GPL-3.0-or-later
//! Sensor ll* functions — llSensor, llSensorRepeat, llSensorRemove.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use super::ScriptContext;

pub const AGENT: i32 = 1;
pub const ACTIVE: i32 = 2;
pub const PASSIVE: i32 = 4;
pub const SCRIPTED: i32 = 8;

#[derive(Debug, Clone)]
pub enum SensorRequest {
    Scan {
        script_id: Uuid,
        name: String,
        key: Uuid,
        scan_type: i32,
        range: f64,
        arc: f64,
    },
    Repeat {
        script_id: Uuid,
        name: String,
        key: Uuid,
        scan_type: i32,
        range: f64,
        arc: f64,
        rate: f64,
    },
    Remove {
        script_id: Uuid,
    },
}

pub fn register_sensor(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llSensor(name, id, type, range, arc)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (name, id, scan_type, range, arc): (String, String, i32, f64, f64)| {
                let ctx = ctx.read();
                let key = Uuid::parse_str(&id).unwrap_or(Uuid::nil());
                if let Some(ref tx) = ctx.sensor_sender {
                    let _ = tx.send(SensorRequest::Scan {
                        script_id: ctx.script_id,
                        name,
                        key,
                        scan_type,
                        range,
                        arc,
                    });
                }
                Ok(())
            },
        )?;
        lua.globals().set("llSensor", f)?;
    }

    // llSensorRepeat(name, id, type, range, arc, rate)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_,
                  (name, id, scan_type, range, arc, rate): (
                String,
                String,
                i32,
                f64,
                f64,
                f64,
            )| {
                let ctx = ctx.read();
                let key = Uuid::parse_str(&id).unwrap_or(Uuid::nil());
                let rate = rate.max(0.2);
                if let Some(ref tx) = ctx.sensor_sender {
                    let _ = tx.send(SensorRequest::Repeat {
                        script_id: ctx.script_id,
                        name,
                        key,
                        scan_type,
                        range,
                        arc,
                        rate,
                    });
                }
                Ok(())
            },
        )?;
        lua.globals().set("llSensorRepeat", f)?;
    }

    // llSensorRemove()
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            if let Some(ref tx) = ctx.sensor_sender {
                let _ = tx.send(SensorRequest::Remove {
                    script_id: ctx.script_id,
                });
            }
            Ok(())
        })?;
        lua.globals().set("llSensorRemove", f)?;
    }

    Ok(())
}
