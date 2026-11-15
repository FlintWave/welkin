// SPDX-License-Identifier: GPL-3.0-or-later
//! Vehicle dynamics ll* functions — llSetVehicleType, llSetVehicleFloatParam, etc.
//!
//! Stores vehicle parameters in [`VehicleParams`] on [`ScriptContext`].
//! Type presets match OpenSim's ODEDynamics.cs defaults.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

// ---------------------------------------------------------------------------
// VEHICLE_* constants (values from OpenSim LSL_Constants.cs)
// ---------------------------------------------------------------------------

pub const VEHICLE_TYPE_NONE: i32 = 0;
pub const VEHICLE_TYPE_SLED: i32 = 1;
pub const VEHICLE_TYPE_CAR: i32 = 2;
pub const VEHICLE_TYPE_BOAT: i32 = 3;
pub const VEHICLE_TYPE_AIRPLANE: i32 = 4;
pub const VEHICLE_TYPE_BALLOON: i32 = 5;

pub const VEHICLE_LINEAR_FRICTION_TIMESCALE: i32 = 16;
pub const VEHICLE_ANGULAR_FRICTION_TIMESCALE: i32 = 17;
pub const VEHICLE_LINEAR_MOTOR_DIRECTION: i32 = 18;
pub const VEHICLE_ANGULAR_MOTOR_DIRECTION: i32 = 19;
pub const VEHICLE_LINEAR_MOTOR_OFFSET: i32 = 20;
pub const VEHICLE_HOVER_HEIGHT: i32 = 24;
pub const VEHICLE_HOVER_EFFICIENCY: i32 = 25;
pub const VEHICLE_HOVER_TIMESCALE: i32 = 26;
pub const VEHICLE_BUOYANCY: i32 = 27;
pub const VEHICLE_LINEAR_DEFLECTION_EFFICIENCY: i32 = 28;
pub const VEHICLE_LINEAR_DEFLECTION_TIMESCALE: i32 = 29;
pub const VEHICLE_LINEAR_MOTOR_TIMESCALE: i32 = 30;
pub const VEHICLE_LINEAR_MOTOR_DECAY_TIMESCALE: i32 = 31;
pub const VEHICLE_ANGULAR_DEFLECTION_EFFICIENCY: i32 = 32;
pub const VEHICLE_ANGULAR_DEFLECTION_TIMESCALE: i32 = 33;
pub const VEHICLE_ANGULAR_MOTOR_TIMESCALE: i32 = 34;
pub const VEHICLE_ANGULAR_MOTOR_DECAY_TIMESCALE: i32 = 35;
pub const VEHICLE_VERTICAL_ATTRACTION_EFFICIENCY: i32 = 36;
pub const VEHICLE_VERTICAL_ATTRACTION_TIMESCALE: i32 = 37;
pub const VEHICLE_BANKING_EFFICIENCY: i32 = 38;
pub const VEHICLE_BANKING_MIX: i32 = 39;
pub const VEHICLE_BANKING_TIMESCALE: i32 = 40;
pub const VEHICLE_REFERENCE_FRAME: i32 = 44;
pub const VEHICLE_RANGE_BLOCK: i32 = 45;
pub const VEHICLE_ROLL_FRAME: i32 = 46;

pub const VEHICLE_FLAG_NO_DEFLECTION_UP: i32 = 1;
pub const VEHICLE_FLAG_LIMIT_ROLL_ONLY: i32 = 2;
pub const VEHICLE_FLAG_HOVER_WATER_ONLY: i32 = 4;
pub const VEHICLE_FLAG_HOVER_TERRAIN_ONLY: i32 = 8;
pub const VEHICLE_FLAG_HOVER_GLOBAL_HEIGHT: i32 = 16;
pub const VEHICLE_FLAG_HOVER_UP_ONLY: i32 = 32;
pub const VEHICLE_FLAG_LIMIT_MOTOR_UP: i32 = 64;
pub const VEHICLE_FLAG_MOUSELOOK_STEER: i32 = 128;
pub const VEHICLE_FLAG_MOUSELOOK_BANK: i32 = 256;
pub const VEHICLE_FLAG_CAMERA_DECOUPLED: i32 = 512;
pub const VEHICLE_FLAG_NO_X: i32 = 1024;
pub const VEHICLE_FLAG_NO_Y: i32 = 2048;
pub const VEHICLE_FLAG_NO_Z: i32 = 4096;
pub const VEHICLE_FLAG_LOCK_HOVER_HEIGHT: i32 = 8192;
pub const VEHICLE_FLAG_NO_DEFLECTION: i32 = 16392;
pub const VEHICLE_FLAG_LOCK_ROTATION: i32 = 32784;

// ---------------------------------------------------------------------------
// VehicleParams
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VehicleParams {
    pub vehicle_type: i32,
    pub flags: u32,
    pub linear_friction_timescale: [f64; 3],
    pub angular_friction_timescale: [f64; 3],
    pub linear_motor_direction: [f64; 3],
    pub linear_motor_offset: [f64; 3],
    pub angular_motor_direction: [f64; 3],
    pub hover_height: f64,
    pub hover_efficiency: f64,
    pub hover_timescale: f64,
    pub buoyancy: f64,
    pub linear_deflection_efficiency: f64,
    pub linear_deflection_timescale: f64,
    pub linear_motor_timescale: f64,
    pub linear_motor_decay_timescale: f64,
    pub angular_deflection_efficiency: f64,
    pub angular_deflection_timescale: f64,
    pub angular_motor_timescale: f64,
    pub angular_motor_decay_timescale: f64,
    pub vertical_attraction_efficiency: f64,
    pub vertical_attraction_timescale: f64,
    pub banking_efficiency: f64,
    pub banking_mix: f64,
    pub banking_timescale: f64,
    pub reference_frame: [f64; 4],
}

impl Default for VehicleParams {
    fn default() -> Self {
        Self {
            vehicle_type: VEHICLE_TYPE_NONE,
            flags: 0,
            linear_friction_timescale: [1000.0; 3],
            angular_friction_timescale: [1000.0; 3],
            linear_motor_direction: [0.0; 3],
            linear_motor_offset: [0.0; 3],
            angular_motor_direction: [0.0; 3],
            hover_height: 0.0,
            hover_efficiency: 1.0,
            hover_timescale: 1000.0,
            buoyancy: 0.0,
            linear_deflection_efficiency: 0.0,
            linear_deflection_timescale: 1000.0,
            linear_motor_timescale: 1000.0,
            linear_motor_decay_timescale: 120.0,
            angular_deflection_efficiency: 0.0,
            angular_deflection_timescale: 1000.0,
            angular_motor_timescale: 1000.0,
            angular_motor_decay_timescale: 1000.0,
            vertical_attraction_efficiency: 0.0,
            vertical_attraction_timescale: 1000.0,
            banking_efficiency: 0.0,
            banking_mix: 1.0,
            banking_timescale: 1000.0,
            reference_frame: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl VehicleParams {
    /// Apply preset defaults for a vehicle type (matches OpenSim ODEDynamics).
    pub fn apply_type_preset(&mut self, vtype: i32) {
        match vtype {
            VEHICLE_TYPE_NONE => {
                *self = Self::default();
            }
            VEHICLE_TYPE_SLED => {
                self.vehicle_type = VEHICLE_TYPE_SLED;
                self.linear_friction_timescale = [30.0, 1.0, 1000.0];
                self.angular_friction_timescale = [1000.0, 1000.0, 1000.0];
                self.linear_motor_timescale = 1000.0;
                self.linear_motor_decay_timescale = 120.0;
                self.angular_motor_timescale = 1000.0;
                self.angular_motor_decay_timescale = 120.0;
                self.hover_height = 0.0;
                self.hover_efficiency = 1.0;
                self.hover_timescale = 10.0;
                self.buoyancy = 0.0;
                self.linear_deflection_efficiency = 1.0;
                self.linear_deflection_timescale = 1.0;
                self.angular_deflection_efficiency = 0.0;
                self.angular_deflection_timescale = 10.0;
                self.vertical_attraction_efficiency = 1.0;
                self.vertical_attraction_timescale = 1000.0;
                self.banking_efficiency = 0.0;
                self.banking_mix = 1.0;
                self.banking_timescale = 10.0;
                self.flags &= !(VEHICLE_FLAG_HOVER_WATER_ONLY
                    | VEHICLE_FLAG_HOVER_TERRAIN_ONLY
                    | VEHICLE_FLAG_HOVER_GLOBAL_HEIGHT
                    | VEHICLE_FLAG_HOVER_UP_ONLY) as u32;
                self.flags |= (VEHICLE_FLAG_NO_DEFLECTION_UP
                    | VEHICLE_FLAG_LIMIT_ROLL_ONLY
                    | VEHICLE_FLAG_LIMIT_MOTOR_UP) as u32;
            }
            VEHICLE_TYPE_CAR => {
                self.vehicle_type = VEHICLE_TYPE_CAR;
                self.linear_friction_timescale = [100.0, 2.0, 1000.0];
                self.angular_friction_timescale = [1000.0, 1000.0, 1000.0];
                self.linear_motor_timescale = 1.0;
                self.linear_motor_decay_timescale = 60.0;
                self.angular_motor_timescale = 1.0;
                self.angular_motor_decay_timescale = 0.8;
                self.hover_height = 0.0;
                self.hover_efficiency = 0.0;
                self.hover_timescale = 1000.0;
                self.buoyancy = 0.0;
                self.linear_deflection_efficiency = 1.0;
                self.linear_deflection_timescale = 2.0;
                self.angular_deflection_efficiency = 0.0;
                self.angular_deflection_timescale = 10.0;
                self.vertical_attraction_efficiency = 1.0;
                self.vertical_attraction_timescale = 10.0;
                self.banking_efficiency = -0.2;
                self.banking_mix = 1.0;
                self.banking_timescale = 1.0;
                self.flags &= !(VEHICLE_FLAG_HOVER_WATER_ONLY
                    | VEHICLE_FLAG_HOVER_TERRAIN_ONLY
                    | VEHICLE_FLAG_HOVER_GLOBAL_HEIGHT) as u32;
                self.flags |= (VEHICLE_FLAG_NO_DEFLECTION_UP
                    | VEHICLE_FLAG_LIMIT_ROLL_ONLY
                    | VEHICLE_FLAG_LIMIT_MOTOR_UP
                    | VEHICLE_FLAG_HOVER_UP_ONLY) as u32;
            }
            VEHICLE_TYPE_BOAT => {
                self.vehicle_type = VEHICLE_TYPE_BOAT;
                self.linear_friction_timescale = [10.0, 3.0, 2.0];
                self.angular_friction_timescale = [10.0, 10.0, 10.0];
                self.linear_motor_timescale = 5.0;
                self.linear_motor_decay_timescale = 60.0;
                self.angular_motor_timescale = 4.0;
                self.angular_motor_decay_timescale = 4.0;
                self.hover_height = 0.0;
                self.hover_efficiency = 0.5;
                self.hover_timescale = 2.0;
                self.buoyancy = 1.0;
                self.linear_deflection_efficiency = 0.5;
                self.linear_deflection_timescale = 3.0;
                self.angular_deflection_efficiency = 0.5;
                self.angular_deflection_timescale = 5.0;
                self.vertical_attraction_efficiency = 0.5;
                self.vertical_attraction_timescale = 5.0;
                self.banking_efficiency = -0.3;
                self.banking_mix = 0.8;
                self.banking_timescale = 1.0;
                self.flags &= !(VEHICLE_FLAG_HOVER_TERRAIN_ONLY
                    | VEHICLE_FLAG_HOVER_GLOBAL_HEIGHT
                    | VEHICLE_FLAG_HOVER_UP_ONLY) as u32;
                self.flags |= (VEHICLE_FLAG_NO_DEFLECTION_UP
                    | VEHICLE_FLAG_LIMIT_MOTOR_UP
                    | VEHICLE_FLAG_HOVER_UP_ONLY
                    | VEHICLE_FLAG_HOVER_WATER_ONLY) as u32;
            }
            VEHICLE_TYPE_AIRPLANE => {
                self.vehicle_type = VEHICLE_TYPE_AIRPLANE;
                self.linear_friction_timescale = [200.0, 10.0, 5.0];
                self.angular_friction_timescale = [20.0, 20.0, 20.0];
                self.linear_motor_timescale = 2.0;
                self.linear_motor_decay_timescale = 60.0;
                self.angular_motor_timescale = 4.0;
                self.angular_motor_decay_timescale = 8.0;
                self.hover_height = 0.0;
                self.hover_efficiency = 0.5;
                self.hover_timescale = 1000.0;
                self.buoyancy = 0.0;
                self.linear_deflection_efficiency = 0.5;
                self.linear_deflection_timescale = 0.5;
                self.angular_deflection_efficiency = 1.0;
                self.angular_deflection_timescale = 2.0;
                self.vertical_attraction_efficiency = 0.9;
                self.vertical_attraction_timescale = 2.0;
                self.banking_efficiency = 1.0;
                self.banking_mix = 0.7;
                self.banking_timescale = 2.0;
                self.flags &= !(VEHICLE_FLAG_HOVER_WATER_ONLY
                    | VEHICLE_FLAG_HOVER_TERRAIN_ONLY
                    | VEHICLE_FLAG_HOVER_GLOBAL_HEIGHT
                    | VEHICLE_FLAG_HOVER_UP_ONLY
                    | VEHICLE_FLAG_NO_DEFLECTION_UP
                    | VEHICLE_FLAG_LIMIT_MOTOR_UP) as u32;
                self.flags |= VEHICLE_FLAG_LIMIT_ROLL_ONLY as u32;
            }
            VEHICLE_TYPE_BALLOON => {
                self.vehicle_type = VEHICLE_TYPE_BALLOON;
                self.linear_friction_timescale = [5.0, 5.0, 5.0];
                self.angular_friction_timescale = [10.0, 10.0, 10.0];
                self.linear_motor_timescale = 5.0;
                self.linear_motor_decay_timescale = 60.0;
                self.angular_motor_timescale = 6.0;
                self.angular_motor_decay_timescale = 10.0;
                self.hover_height = 5.0;
                self.hover_efficiency = 0.8;
                self.hover_timescale = 10.0;
                self.buoyancy = 1.0;
                self.linear_deflection_efficiency = 0.0;
                self.linear_deflection_timescale = 5.0;
                self.angular_deflection_efficiency = 0.0;
                self.angular_deflection_timescale = 5.0;
                self.vertical_attraction_efficiency = 1.0;
                self.vertical_attraction_timescale = 1000.0;
                self.banking_efficiency = 0.0;
                self.banking_mix = 0.7;
                self.banking_timescale = 5.0;
                self.flags &= !(VEHICLE_FLAG_HOVER_WATER_ONLY
                    | VEHICLE_FLAG_HOVER_TERRAIN_ONLY
                    | VEHICLE_FLAG_HOVER_UP_ONLY
                    | VEHICLE_FLAG_NO_DEFLECTION_UP
                    | VEHICLE_FLAG_LIMIT_MOTOR_UP
                    | VEHICLE_FLAG_LIMIT_ROLL_ONLY
                    | VEHICLE_FLAG_HOVER_GLOBAL_HEIGHT) as u32;
            }
            _ => {}
        }
        // All types disable mouse steering by default
        self.flags &= !(VEHICLE_FLAG_MOUSELOOK_STEER
            | VEHICLE_FLAG_MOUSELOOK_BANK
            | VEHICLE_FLAG_CAMERA_DECOUPLED) as u32;
    }

    fn set_float_param(&mut self, param: i32, value: f64) {
        match param {
            VEHICLE_HOVER_HEIGHT => self.hover_height = value,
            VEHICLE_HOVER_EFFICIENCY => self.hover_efficiency = value.clamp(0.0, 1.0),
            VEHICLE_HOVER_TIMESCALE => self.hover_timescale = value.max(0.01),
            VEHICLE_BUOYANCY => self.buoyancy = value.clamp(-1.0, 1.0),
            VEHICLE_LINEAR_DEFLECTION_EFFICIENCY => {
                self.linear_deflection_efficiency = value.clamp(0.0, 1.0)
            }
            VEHICLE_LINEAR_DEFLECTION_TIMESCALE => {
                self.linear_deflection_timescale = value.max(0.01)
            }
            VEHICLE_LINEAR_MOTOR_TIMESCALE => self.linear_motor_timescale = value.max(0.01),
            VEHICLE_LINEAR_MOTOR_DECAY_TIMESCALE => {
                self.linear_motor_decay_timescale = value.max(0.01)
            }
            VEHICLE_ANGULAR_DEFLECTION_EFFICIENCY => {
                self.angular_deflection_efficiency = value.clamp(0.0, 1.0)
            }
            VEHICLE_ANGULAR_DEFLECTION_TIMESCALE => {
                self.angular_deflection_timescale = value.max(0.01)
            }
            VEHICLE_ANGULAR_MOTOR_TIMESCALE => self.angular_motor_timescale = value.max(0.01),
            VEHICLE_ANGULAR_MOTOR_DECAY_TIMESCALE => {
                self.angular_motor_decay_timescale = value.max(0.01)
            }
            VEHICLE_VERTICAL_ATTRACTION_EFFICIENCY => {
                self.vertical_attraction_efficiency = value.clamp(0.0, 1.0)
            }
            VEHICLE_VERTICAL_ATTRACTION_TIMESCALE => {
                self.vertical_attraction_timescale = value.max(0.01)
            }
            VEHICLE_BANKING_EFFICIENCY => self.banking_efficiency = value.clamp(-1.0, 1.0),
            VEHICLE_BANKING_MIX => self.banking_mix = value.clamp(0.0, 1.0),
            VEHICLE_BANKING_TIMESCALE => self.banking_timescale = value.max(0.01),
            _ => {}
        }
    }

    fn set_vector_param(&mut self, param: i32, value: [f64; 3]) {
        match param {
            VEHICLE_LINEAR_FRICTION_TIMESCALE => {
                self.linear_friction_timescale =
                    [value[0].max(0.01), value[1].max(0.01), value[2].max(0.01)]
            }
            VEHICLE_ANGULAR_FRICTION_TIMESCALE => {
                self.angular_friction_timescale =
                    [value[0].max(0.01), value[1].max(0.01), value[2].max(0.01)]
            }
            VEHICLE_LINEAR_MOTOR_DIRECTION => self.linear_motor_direction = value,
            VEHICLE_ANGULAR_MOTOR_DIRECTION => self.angular_motor_direction = value,
            VEHICLE_LINEAR_MOTOR_OFFSET => self.linear_motor_offset = value,
            _ => {}
        }
    }

    fn set_rotation_param(&mut self, param: i32, value: [f64; 4]) {
        match param {
            VEHICLE_REFERENCE_FRAME | VEHICLE_ROLL_FRAME => self.reference_frame = value,
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

fn read_vec3(table: &LuaTable) -> LuaResult<[f64; 3]> {
    Ok([
        table.get::<f64>("x").or_else(|_| table.get::<f64>(1))?,
        table.get::<f64>("y").or_else(|_| table.get::<f64>(2))?,
        table.get::<f64>("z").or_else(|_| table.get::<f64>(3))?,
    ])
}

fn read_rot4(table: &LuaTable) -> LuaResult<[f64; 4]> {
    Ok([
        table.get::<f64>("x").or_else(|_| table.get::<f64>(1))?,
        table.get::<f64>("y").or_else(|_| table.get::<f64>(2))?,
        table.get::<f64>("z").or_else(|_| table.get::<f64>(3))?,
        table.get::<f64>("w").or_else(|_| table.get::<f64>(4))?,
    ])
}

pub fn register_vehicle(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    let g = lua.globals();

    // Register all VEHICLE_* constants
    g.set("VEHICLE_TYPE_NONE", VEHICLE_TYPE_NONE)?;
    g.set("VEHICLE_TYPE_SLED", VEHICLE_TYPE_SLED)?;
    g.set("VEHICLE_TYPE_CAR", VEHICLE_TYPE_CAR)?;
    g.set("VEHICLE_TYPE_BOAT", VEHICLE_TYPE_BOAT)?;
    g.set("VEHICLE_TYPE_AIRPLANE", VEHICLE_TYPE_AIRPLANE)?;
    g.set("VEHICLE_TYPE_BALLOON", VEHICLE_TYPE_BALLOON)?;

    g.set(
        "VEHICLE_LINEAR_FRICTION_TIMESCALE",
        VEHICLE_LINEAR_FRICTION_TIMESCALE,
    )?;
    g.set(
        "VEHICLE_ANGULAR_FRICTION_TIMESCALE",
        VEHICLE_ANGULAR_FRICTION_TIMESCALE,
    )?;
    g.set(
        "VEHICLE_LINEAR_MOTOR_DIRECTION",
        VEHICLE_LINEAR_MOTOR_DIRECTION,
    )?;
    g.set(
        "VEHICLE_ANGULAR_MOTOR_DIRECTION",
        VEHICLE_ANGULAR_MOTOR_DIRECTION,
    )?;
    g.set("VEHICLE_LINEAR_MOTOR_OFFSET", VEHICLE_LINEAR_MOTOR_OFFSET)?;
    g.set("VEHICLE_HOVER_HEIGHT", VEHICLE_HOVER_HEIGHT)?;
    g.set("VEHICLE_HOVER_EFFICIENCY", VEHICLE_HOVER_EFFICIENCY)?;
    g.set("VEHICLE_HOVER_TIMESCALE", VEHICLE_HOVER_TIMESCALE)?;
    g.set("VEHICLE_BUOYANCY", VEHICLE_BUOYANCY)?;
    g.set(
        "VEHICLE_LINEAR_DEFLECTION_EFFICIENCY",
        VEHICLE_LINEAR_DEFLECTION_EFFICIENCY,
    )?;
    g.set(
        "VEHICLE_LINEAR_DEFLECTION_TIMESCALE",
        VEHICLE_LINEAR_DEFLECTION_TIMESCALE,
    )?;
    g.set(
        "VEHICLE_LINEAR_MOTOR_TIMESCALE",
        VEHICLE_LINEAR_MOTOR_TIMESCALE,
    )?;
    g.set(
        "VEHICLE_LINEAR_MOTOR_DECAY_TIMESCALE",
        VEHICLE_LINEAR_MOTOR_DECAY_TIMESCALE,
    )?;
    g.set(
        "VEHICLE_ANGULAR_DEFLECTION_EFFICIENCY",
        VEHICLE_ANGULAR_DEFLECTION_EFFICIENCY,
    )?;
    g.set(
        "VEHICLE_ANGULAR_DEFLECTION_TIMESCALE",
        VEHICLE_ANGULAR_DEFLECTION_TIMESCALE,
    )?;
    g.set(
        "VEHICLE_ANGULAR_MOTOR_TIMESCALE",
        VEHICLE_ANGULAR_MOTOR_TIMESCALE,
    )?;
    g.set(
        "VEHICLE_ANGULAR_MOTOR_DECAY_TIMESCALE",
        VEHICLE_ANGULAR_MOTOR_DECAY_TIMESCALE,
    )?;
    g.set(
        "VEHICLE_VERTICAL_ATTRACTION_EFFICIENCY",
        VEHICLE_VERTICAL_ATTRACTION_EFFICIENCY,
    )?;
    g.set(
        "VEHICLE_VERTICAL_ATTRACTION_TIMESCALE",
        VEHICLE_VERTICAL_ATTRACTION_TIMESCALE,
    )?;
    g.set("VEHICLE_BANKING_EFFICIENCY", VEHICLE_BANKING_EFFICIENCY)?;
    g.set("VEHICLE_BANKING_MIX", VEHICLE_BANKING_MIX)?;
    g.set("VEHICLE_BANKING_TIMESCALE", VEHICLE_BANKING_TIMESCALE)?;
    g.set("VEHICLE_REFERENCE_FRAME", VEHICLE_REFERENCE_FRAME)?;
    g.set("VEHICLE_RANGE_BLOCK", VEHICLE_RANGE_BLOCK)?;
    g.set("VEHICLE_ROLL_FRAME", VEHICLE_ROLL_FRAME)?;

    g.set(
        "VEHICLE_FLAG_NO_DEFLECTION_UP",
        VEHICLE_FLAG_NO_DEFLECTION_UP,
    )?;
    g.set(
        "VEHICLE_FLAG_LIMIT_ROLL_ONLY",
        VEHICLE_FLAG_LIMIT_ROLL_ONLY,
    )?;
    g.set(
        "VEHICLE_FLAG_HOVER_WATER_ONLY",
        VEHICLE_FLAG_HOVER_WATER_ONLY,
    )?;
    g.set(
        "VEHICLE_FLAG_HOVER_TERRAIN_ONLY",
        VEHICLE_FLAG_HOVER_TERRAIN_ONLY,
    )?;
    g.set(
        "VEHICLE_FLAG_HOVER_GLOBAL_HEIGHT",
        VEHICLE_FLAG_HOVER_GLOBAL_HEIGHT,
    )?;
    g.set("VEHICLE_FLAG_HOVER_UP_ONLY", VEHICLE_FLAG_HOVER_UP_ONLY)?;
    g.set("VEHICLE_FLAG_LIMIT_MOTOR_UP", VEHICLE_FLAG_LIMIT_MOTOR_UP)?;
    g.set(
        "VEHICLE_FLAG_MOUSELOOK_STEER",
        VEHICLE_FLAG_MOUSELOOK_STEER,
    )?;
    g.set(
        "VEHICLE_FLAG_MOUSELOOK_BANK",
        VEHICLE_FLAG_MOUSELOOK_BANK,
    )?;
    g.set(
        "VEHICLE_FLAG_CAMERA_DECOUPLED",
        VEHICLE_FLAG_CAMERA_DECOUPLED,
    )?;
    g.set("VEHICLE_FLAG_NO_X", VEHICLE_FLAG_NO_X)?;
    g.set("VEHICLE_FLAG_NO_Y", VEHICLE_FLAG_NO_Y)?;
    g.set("VEHICLE_FLAG_NO_Z", VEHICLE_FLAG_NO_Z)?;
    g.set(
        "VEHICLE_FLAG_LOCK_HOVER_HEIGHT",
        VEHICLE_FLAG_LOCK_HOVER_HEIGHT,
    )?;
    g.set("VEHICLE_FLAG_NO_DEFLECTION", VEHICLE_FLAG_NO_DEFLECTION)?;
    g.set("VEHICLE_FLAG_LOCK_ROTATION", VEHICLE_FLAG_LOCK_ROTATION)?;

    // llSetVehicleType(type_id)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, type_id: i64| {
            let mut ctx = ctx.write();
            ctx.vehicle.apply_type_preset(type_id as i32);
            Ok(())
        })?;
        g.set("llSetVehicleType", f)?;
    }

    // llSetVehicleFloatParam(param, value)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (param, value): (i64, f64)| {
            let mut ctx = ctx.write();
            ctx.vehicle.set_float_param(param as i32, value);
            Ok(())
        })?;
        g.set("llSetVehicleFloatParam", f)?;
    }

    // llSetVehicleVectorParam(param, value)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (param, value): (i64, LuaTable)| {
            let vec = read_vec3(&value)?;
            let mut ctx = ctx.write();
            ctx.vehicle.set_vector_param(param as i32, vec);
            Ok(())
        })?;
        g.set("llSetVehicleVectorParam", f)?;
    }

    // llSetVehicleRotationParam(param, value)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (param, value): (i64, LuaTable)| {
            let rot = read_rot4(&value)?;
            let mut ctx = ctx.write();
            ctx.vehicle.set_rotation_param(param as i32, rot);
            Ok(())
        })?;
        g.set("llSetVehicleRotationParam", f)?;
    }

    // llSetVehicleFlags(flags)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, flags: i64| {
            let mut ctx = ctx.write();
            ctx.vehicle.flags |= flags as u32;
            Ok(())
        })?;
        g.set("llSetVehicleFlags", f)?;
    }

    // llRemoveVehicleFlags(flags)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, flags: i64| {
            let mut ctx = ctx.write();
            ctx.vehicle.flags &= !(flags as u32);
            Ok(())
        })?;
        g.set("llRemoveVehicleFlags", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::nil(),
            prim_id: Uuid::new_v4(),
            ..Default::default()
        }))
    }

    #[test]
    fn set_vehicle_type_car() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_vehicle(&lua, &ctx).unwrap();
        lua.load("llSetVehicleType(VEHICLE_TYPE_CAR)").exec().unwrap();
        let c = ctx.read();
        assert_eq!(c.vehicle.vehicle_type, VEHICLE_TYPE_CAR);
        assert!((c.vehicle.linear_motor_timescale - 1.0).abs() < f64::EPSILON);
        assert!((c.vehicle.banking_efficiency - (-0.2)).abs() < f64::EPSILON);
        assert!(c.vehicle.flags & VEHICLE_FLAG_LIMIT_ROLL_ONLY as u32 != 0);
    }

    #[test]
    fn set_vehicle_type_airplane() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_vehicle(&lua, &ctx).unwrap();
        lua.load("llSetVehicleType(VEHICLE_TYPE_AIRPLANE)")
            .exec()
            .unwrap();
        let c = ctx.read();
        assert_eq!(c.vehicle.vehicle_type, VEHICLE_TYPE_AIRPLANE);
        assert!((c.vehicle.buoyancy - 0.0).abs() < f64::EPSILON);
        assert!((c.vehicle.banking_efficiency - 1.0).abs() < f64::EPSILON);
        assert!(c.vehicle.flags & VEHICLE_FLAG_LIMIT_ROLL_ONLY as u32 != 0);
        assert_eq!(c.vehicle.flags & VEHICLE_FLAG_NO_DEFLECTION_UP as u32, 0);
    }

    #[test]
    fn set_vehicle_float_param() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_vehicle(&lua, &ctx).unwrap();
        lua.load("llSetVehicleFloatParam(VEHICLE_BUOYANCY, 0.75)")
            .exec()
            .unwrap();
        let c = ctx.read();
        assert!((c.vehicle.buoyancy - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn set_vehicle_vector_param() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_vehicle(&lua, &ctx).unwrap();
        lua.load("llSetVehicleVectorParam(VEHICLE_LINEAR_MOTOR_DIRECTION, {x=10, y=0, z=0})")
            .exec()
            .unwrap();
        let c = ctx.read();
        assert!((c.vehicle.linear_motor_direction[0] - 10.0).abs() < f64::EPSILON);
        assert!((c.vehicle.linear_motor_direction[1] - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_vehicle_rotation_param() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_vehicle(&lua, &ctx).unwrap();
        lua.load(
            "llSetVehicleRotationParam(VEHICLE_REFERENCE_FRAME, {x=0, y=0, z=0.707, w=0.707})",
        )
        .exec()
        .unwrap();
        let c = ctx.read();
        assert!((c.vehicle.reference_frame[2] - 0.707).abs() < 0.001);
        assert!((c.vehicle.reference_frame[3] - 0.707).abs() < 0.001);
    }

    #[test]
    fn set_and_remove_vehicle_flags() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_vehicle(&lua, &ctx).unwrap();
        lua.load("llSetVehicleFlags(VEHICLE_FLAG_HOVER_WATER_ONLY + VEHICLE_FLAG_NO_X)")
            .exec()
            .unwrap();
        {
            let c = ctx.read();
            assert!(c.vehicle.flags & VEHICLE_FLAG_HOVER_WATER_ONLY as u32 != 0);
            assert!(c.vehicle.flags & VEHICLE_FLAG_NO_X as u32 != 0);
        }
        lua.load("llRemoveVehicleFlags(VEHICLE_FLAG_HOVER_WATER_ONLY)")
            .exec()
            .unwrap();
        let c = ctx.read();
        assert_eq!(c.vehicle.flags & VEHICLE_FLAG_HOVER_WATER_ONLY as u32, 0);
        assert!(c.vehicle.flags & VEHICLE_FLAG_NO_X as u32 != 0);
    }

    #[test]
    fn type_none_resets_all() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_vehicle(&lua, &ctx).unwrap();
        lua.load("llSetVehicleType(VEHICLE_TYPE_BOAT)").exec().unwrap();
        lua.load("llSetVehicleType(VEHICLE_TYPE_NONE)").exec().unwrap();
        let c = ctx.read();
        assert_eq!(c.vehicle.vehicle_type, VEHICLE_TYPE_NONE);
        assert_eq!(c.vehicle.flags, 0);
        assert!((c.vehicle.buoyancy - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn float_param_clamping() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_vehicle(&lua, &ctx).unwrap();
        lua.load("llSetVehicleFloatParam(VEHICLE_BUOYANCY, 5.0)")
            .exec()
            .unwrap();
        assert!((ctx.read().vehicle.buoyancy - 1.0).abs() < f64::EPSILON);
        lua.load("llSetVehicleFloatParam(VEHICLE_BUOYANCY, -5.0)")
            .exec()
            .unwrap();
        assert!((ctx.read().vehicle.buoyancy - (-1.0)).abs() < f64::EPSILON);
    }
}
