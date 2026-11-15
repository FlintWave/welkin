// SPDX-License-Identifier: GPL-3.0-or-later
//! Physics ll* function stubs (llApplyImpulse, llSetForce, llGetVel, etc.).
//!
//! These are placeholder implementations that log calls via `tracing::debug!`
//! and return sensible defaults.  Real implementations will integrate with the
//! physics engine (Rapier).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::ScriptContext;

/// Create a vector table `{x, y, z}` with the given components.
fn make_vec3(lua: &Lua, x: f64, y: f64, z: f64) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", x)?;
    t.set("y", y)?;
    t.set("z", z)?;
    Ok(t)
}

/// Register all physics ll* functions as Lua globals.
pub fn register_physics(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), crate::VmError> {
    // llApplyImpulse(force, local)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_force, local): (LuaValue, i64)| {
            let _ctx = ctx.read();
            tracing::debug!(local, "llApplyImpulse");
            Ok(())
        })?;
        lua.globals().set("llApplyImpulse", f)?;
    }

    // llApplyRotationalImpulse(force, local)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_force, local): (LuaValue, i64)| {
            let _ctx = ctx.read();
            tracing::debug!(local, "llApplyRotationalImpulse");
            Ok(())
        })?;
        lua.globals().set("llApplyRotationalImpulse", f)?;
    }

    // llSetForce(force, local)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_force, local): (LuaValue, i64)| {
            let _ctx = ctx.read();
            tracing::debug!(local, "llSetForce");
            Ok(())
        })?;
        lua.globals().set("llSetForce", f)?;
    }

    // llSetTorque(torque, local)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_torque, local): (LuaValue, i64)| {
            let _ctx = ctx.read();
            tracing::debug!(local, "llSetTorque");
            Ok(())
        })?;
        lua.globals().set("llSetTorque", f)?;
    }

    // llSetBuoyancy(buoyancy)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, buoyancy: f64| {
            let _ctx = ctx.read();
            tracing::debug!(buoyancy, "llSetBuoyancy");
            Ok(())
        })?;
        lua.globals().set("llSetBuoyancy", f)?;
    }

    // llSetStatus(status, value)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (status, value): (i64, i64)| {
            let _ctx = ctx.read();
            tracing::debug!(status, value, "llSetStatus");
            Ok(())
        })?;
        lua.globals().set("llSetStatus", f)?;
    }

    // llGetVel() -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let _ctx = ctx.read();
            tracing::debug!("llGetVel");
            make_vec3(lua, 0.0, 0.0, 0.0)
        })?;
        lua.globals().set("llGetVel", f)?;
    }

    // llGetAccel() -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let _ctx = ctx.read();
            tracing::debug!("llGetAccel");
            make_vec3(lua, 0.0, 0.0, 0.0)
        })?;
        lua.globals().set("llGetAccel", f)?;
    }

    // llGetOmega() -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let _ctx = ctx.read();
            tracing::debug!("llGetOmega");
            make_vec3(lua, 0.0, 0.0, 0.0)
        })?;
        lua.globals().set("llGetOmega", f)?;
    }

    // llGroundNormal(offset) -> vector (default: up = <0,0,1>)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, _offset: LuaValue| {
            let _ctx = ctx.read();
            tracing::debug!("llGroundNormal");
            make_vec3(lua, 0.0, 0.0, 1.0)
        })?;
        lua.globals().set("llGroundNormal", f)?;
    }

    // llGround(offset) -> float
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _offset: LuaValue| {
            let _ctx = ctx.read();
            tracing::debug!("llGround");
            Ok(0.0f64)
        })?;
        lua.globals().set("llGround", f)?;
    }

    // llWater(offset) -> float (default water height = 20.0)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _offset: LuaValue| {
            let _ctx = ctx.read();
            tracing::debug!("llWater");
            Ok(20.0f64)
        })?;
        lua.globals().set("llWater", f)?;
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
            prim_id: Uuid::nil(),
            prim_name: "Test Prim".into(),
            prim_desc: String::new(),
            owner_id: Uuid::nil(),
            position: [128.0, 128.0, 25.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            ..Default::default()
        }))
    }

    #[test]
    fn ll_get_vel_returns_zero_vector() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();

        let script = r#"
            local v = llGetVel()
            return v.x + v.y + v.z
        "#;
        let result: f64 = lua.load(script).eval().unwrap();
        assert!((result - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_ground_normal_returns_up() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();

        let script = r#"
            local n = llGroundNormal({x=0, y=0, z=0})
            return n.z
        "#;
        let result: f64 = lua.load(script).eval().unwrap();
        assert!((result - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_water_returns_default_height() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();

        let result: f64 = lua
            .load(r#"return llWater({x=0, y=0, z=0})"#)
            .eval()
            .unwrap();
        assert!((result - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_apply_impulse_does_not_panic() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();
        lua.load(r#"llApplyImpulse({x=1, y=0, z=0}, 0)"#)
            .exec()
            .unwrap();
    }

    #[test]
    fn ll_set_buoyancy_does_not_panic() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();
        lua.load("llSetBuoyancy(1.0)").exec().unwrap();
    }
}
