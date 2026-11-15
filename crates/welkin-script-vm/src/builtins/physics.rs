// SPDX-License-Identifier: GPL-3.0-or-later
//! Physics ll* functions — send commands through a channel to the region's
//! physics engine (Rapier) and read back velocity/acceleration from context.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::{PositionTarget, RotationTarget, ScriptContext};
use crate::PhysicsCommand;

fn get_vec3(tbl: &LuaTable) -> LuaResult<[f64; 3]> {
    Ok([tbl.get("x")?, tbl.get("y")?, tbl.get("z")?])
}

fn get_rot4(tbl: &LuaTable) -> LuaResult<[f64; 4]> {
    Ok([tbl.get("x")?, tbl.get("y")?, tbl.get("z")?, tbl.get("w")?])
}

fn make_vec3(lua: &Lua, v: [f64; 3]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    t.set("z", v[2])?;
    Ok(t)
}

fn send_physics(ctx: &ScriptContext, cmd: PhysicsCommand) {
    if let Some(ref tx) = ctx.physics_sender {
        let _ = tx.send((ctx.prim_id, cmd));
    }
}

/// Register all physics ll* functions as Lua globals.
pub fn register_physics(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), crate::VmError> {
    // llSetForce(force, local)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (force_tbl, local): (LuaTable, i64)| {
            let force = get_vec3(&force_tbl)?;
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::SetForce { force, local: local != 0 });
            Ok(())
        })?;
        lua.globals().set("llSetForce", f)?;
    }

    // llSetTorque(torque, local)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (torque_tbl, local): (LuaTable, i64)| {
            let torque = get_vec3(&torque_tbl)?;
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::SetTorque { torque, local: local != 0 });
            Ok(())
        })?;
        lua.globals().set("llSetTorque", f)?;
    }

    // llApplyImpulse(impulse, local)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (impulse_tbl, local): (LuaTable, i64)| {
            let impulse = get_vec3(&impulse_tbl)?;
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::ApplyImpulse { impulse, local: local != 0 });
            Ok(())
        })?;
        lua.globals().set("llApplyImpulse", f)?;
    }

    // llApplyRotationalImpulse(impulse, local)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (impulse_tbl, local): (LuaTable, i64)| {
            let impulse = get_vec3(&impulse_tbl)?;
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::ApplyRotationalImpulse { impulse, local: local != 0 });
            Ok(())
        })?;
        lua.globals().set("llApplyRotationalImpulse", f)?;
    }

    // llSetBuoyancy(buoyancy)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, buoyancy: f64| {
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::SetBuoyancy { buoyancy });
            Ok(())
        })?;
        lua.globals().set("llSetBuoyancy", f)?;
    }

    // llSetStatus(status, value)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (status, value): (i32, i64)| {
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::SetStatus { status, value: value != 0 });
            Ok(())
        })?;
        lua.globals().set("llSetStatus", f)?;
    }

    // llGetVel() -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let ctx = ctx.read();
            make_vec3(lua, ctx.velocity)
        })?;
        lua.globals().set("llGetVel", f)?;
    }

    // llGetAccel() -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let ctx = ctx.read();
            make_vec3(lua, ctx.acceleration)
        })?;
        lua.globals().set("llGetAccel", f)?;
    }

    // llGetOmega() -> vector
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, ()| {
            let ctx = ctx.read();
            make_vec3(lua, ctx.angular_velocity)
        })?;
        lua.globals().set("llGetOmega", f)?;
    }

    // llGround(offset) -> float (terrain height at position+offset)
    {
        let f = lua.create_function(|_, _offset: LuaValue| {
            Ok(0.0f64)
        })?;
        lua.globals().set("llGround", f)?;
    }

    // llGroundNormal(offset) -> vector (terrain normal, default up)
    {
        let f = lua.create_function(|lua, _offset: LuaValue| {
            make_vec3(lua, [0.0, 0.0, 1.0])
        })?;
        lua.globals().set("llGroundNormal", f)?;
    }

    // llWater(offset) -> float (water height, default 20.0)
    {
        let f = lua.create_function(|_, _offset: LuaValue| {
            Ok(20.0f64)
        })?;
        lua.globals().set("llWater", f)?;
    }

    // llMoveToTarget(target, tau)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (target_tbl, tau): (LuaTable, f64)| {
            let target = get_vec3(&target_tbl)?;
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::MoveToTarget { target, tau });
            Ok(())
        })?;
        lua.globals().set("llMoveToTarget", f)?;
    }

    // llStopMoveToTarget()
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::StopMoveToTarget);
            Ok(())
        })?;
        lua.globals().set("llStopMoveToTarget", f)?;
    }

    // llSetHoverHeight(height, water, tau)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (height, water, tau): (f64, i32, f64)| {
            let ctx = ctx.read();
            send_physics(&ctx, PhysicsCommand::SetHoverHeight { height, water, tau });
            Ok(())
        })?;
        lua.globals().set("llSetHoverHeight", f)?;
    }

    // llTarget(position, range) -> integer handle
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (pos_tbl, range): (LuaTable, f64)| {
            let position = get_vec3(&pos_tbl)?;
            let mut ctx = ctx.write();
            let handle = ctx.next_target_handle;
            ctx.next_target_handle += 1;
            ctx.position_targets.push(PositionTarget { handle, position, range });
            Ok(handle as i64)
        })?;
        lua.globals().set("llTarget", f)?;
    }

    // llTargetRemove(handle)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, handle: i64| {
            let mut ctx = ctx.write();
            ctx.position_targets.retain(|t| t.handle != handle as i32);
            Ok(())
        })?;
        lua.globals().set("llTargetRemove", f)?;
    }

    // llRotTarget(rotation, range) -> integer handle
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (rot_tbl, range): (LuaTable, f64)| {
            let rotation = get_rot4(&rot_tbl)?;
            let mut ctx = ctx.write();
            let handle = ctx.next_target_handle;
            ctx.next_target_handle += 1;
            ctx.rotation_targets.push(RotationTarget { handle, rotation, range });
            Ok(handle as i64)
        })?;
        lua.globals().set("llRotTarget", f)?;
    }

    // llRotTargetRemove(handle)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, handle: i64| {
            let mut ctx = ctx.write();
            ctx.rotation_targets.retain(|t| t.handle != handle as i32);
            Ok(())
        })?;
        lua.globals().set("llRotTargetRemove", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use uuid::Uuid;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::nil(),
            prim_id: Uuid::from_u128(0xdead),
            prim_name: "Test Prim".into(),
            prim_desc: String::new(),
            owner_id: Uuid::nil(),
            position: [128.0, 128.0, 25.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            velocity: [1.5, 2.5, 3.5],
            angular_velocity: [0.1, 0.2, 0.3],
            acceleration: [0.0, 0.0, -9.8],
            ..Default::default()
        }))
    }

    fn ctx_with_physics() -> (Arc<RwLock<ScriptContext>>, mpsc::Receiver<(Uuid, PhysicsCommand)>) {
        let (tx, rx) = mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: Uuid::from_u128(0xdead),
            physics_sender: Some(tx),
            ..Default::default()
        }));
        (ctx, rx)
    }

    #[test]
    fn ll_get_vel_returns_velocity() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();

        let script = r#"
            local v = llGetVel()
            return v.x, v.y, v.z
        "#;
        let (x, y, z): (f64, f64, f64) = lua.load(script).eval().unwrap();
        assert!((x - 1.5).abs() < f64::EPSILON);
        assert!((y - 2.5).abs() < f64::EPSILON);
        assert!((z - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_get_omega_returns_angular_velocity() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();

        let script = r#"
            local v = llGetOmega()
            return v.x, v.y, v.z
        "#;
        let (x, y, z): (f64, f64, f64) = lua.load(script).eval().unwrap();
        assert!((x - 0.1).abs() < f64::EPSILON);
        assert!((y - 0.2).abs() < f64::EPSILON);
        assert!((z - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_get_accel_returns_acceleration() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();

        let z: f64 = lua.load("return llGetAccel().z").eval().unwrap();
        assert!((z - (-9.8)).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_set_force_sends_command() {
        let (ctx, rx) = ctx_with_physics();
        let lua = Lua::new();
        register_physics(&lua, &ctx).unwrap();

        lua.load("llSetForce({x=10, y=0, z=0}, 1)").exec().unwrap();

        let (prim_id, cmd) = rx.recv().unwrap();
        assert_eq!(prim_id, Uuid::from_u128(0xdead));
        match cmd {
            PhysicsCommand::SetForce { force, local } => {
                assert!((force[0] - 10.0).abs() < f64::EPSILON);
                assert!(local);
            }
            _ => panic!("expected SetForce"),
        }
    }

    #[test]
    fn ll_apply_impulse_sends_command() {
        let (ctx, rx) = ctx_with_physics();
        let lua = Lua::new();
        register_physics(&lua, &ctx).unwrap();

        lua.load("llApplyImpulse({x=5, y=3, z=1}, 0)").exec().unwrap();

        let (_id, cmd) = rx.recv().unwrap();
        match cmd {
            PhysicsCommand::ApplyImpulse { impulse, local } => {
                assert!((impulse[0] - 5.0).abs() < f64::EPSILON);
                assert!((impulse[1] - 3.0).abs() < f64::EPSILON);
                assert!((impulse[2] - 1.0).abs() < f64::EPSILON);
                assert!(!local);
            }
            _ => panic!("expected ApplyImpulse"),
        }
    }

    #[test]
    fn ll_move_to_target_sends_command() {
        let (ctx, rx) = ctx_with_physics();
        let lua = Lua::new();
        register_physics(&lua, &ctx).unwrap();

        lua.load("llMoveToTarget({x=100, y=200, z=50}, 0.5)").exec().unwrap();

        let (_id, cmd) = rx.recv().unwrap();
        match cmd {
            PhysicsCommand::MoveToTarget { target, tau } => {
                assert!((target[0] - 100.0).abs() < f64::EPSILON);
                assert!((target[1] - 200.0).abs() < f64::EPSILON);
                assert!((target[2] - 50.0).abs() < f64::EPSILON);
                assert!((tau - 0.5).abs() < f64::EPSILON);
            }
            _ => panic!("expected MoveToTarget"),
        }
    }

    #[test]
    fn ll_ground_returns_zero() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();
        let r: f64 = lua.load("return llGround({x=0, y=0, z=0})").eval().unwrap();
        assert!((r - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_ground_normal_returns_up() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();
        let z: f64 = lua.load("return llGroundNormal({x=0, y=0, z=0}).z").eval().unwrap();
        assert!((z - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_water_returns_default_height() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();
        let r: f64 = lua.load("return llWater({x=0, y=0, z=0})").eval().unwrap();
        assert!((r - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_set_buoyancy_sends_command() {
        let (ctx, rx) = ctx_with_physics();
        let lua = Lua::new();
        register_physics(&lua, &ctx).unwrap();

        lua.load("llSetBuoyancy(0.75)").exec().unwrap();

        let (_id, cmd) = rx.recv().unwrap();
        match cmd {
            PhysicsCommand::SetBuoyancy { buoyancy } => {
                assert!((buoyancy - 0.75).abs() < f64::EPSILON);
            }
            _ => panic!("expected SetBuoyancy"),
        }
    }

    #[test]
    fn ll_stop_move_to_target_sends_command() {
        let (ctx, rx) = ctx_with_physics();
        let lua = Lua::new();
        register_physics(&lua, &ctx).unwrap();

        lua.load("llStopMoveToTarget()").exec().unwrap();

        let (_id, cmd) = rx.recv().unwrap();
        assert!(matches!(cmd, PhysicsCommand::StopMoveToTarget));
    }

    #[test]
    fn ll_set_hover_height_sends_command() {
        let (ctx, rx) = ctx_with_physics();
        let lua = Lua::new();
        register_physics(&lua, &ctx).unwrap();

        lua.load("llSetHoverHeight(10.0, 1, 2.0)").exec().unwrap();

        let (_id, cmd) = rx.recv().unwrap();
        match cmd {
            PhysicsCommand::SetHoverHeight { height, water, tau } => {
                assert!((height - 10.0).abs() < f64::EPSILON);
                assert_eq!(water, 1);
                assert!((tau - 2.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected SetHoverHeight"),
        }
    }

    #[test]
    fn ll_target_registers_and_removes() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_physics(&lua, &ctx).unwrap();

        let h: i64 = lua
            .load("return llTarget({x=100, y=200, z=50}, 5.0)")
            .eval()
            .unwrap();
        assert!(h >= 1);
        assert_eq!(ctx.read().position_targets.len(), 1);
        assert!((ctx.read().position_targets[0].range - 5.0).abs() < f64::EPSILON);

        lua.load(&format!("llTargetRemove({h})")).exec().unwrap();
        assert_eq!(ctx.read().position_targets.len(), 0);
    }

    #[test]
    fn ll_rot_target_registers_and_removes() {
        let ctx = test_ctx();
        let lua = Lua::new();
        register_physics(&lua, &ctx).unwrap();

        let h: i64 = lua
            .load("return llRotTarget({x=0, y=0, z=0, w=1}, 0.1)")
            .eval()
            .unwrap();
        assert!(h >= 1);
        assert_eq!(ctx.read().rotation_targets.len(), 1);
        assert!((ctx.read().rotation_targets[0].range - 0.1).abs() < f64::EPSILON);

        lua.load(&format!("llRotTargetRemove({h})")).exec().unwrap();
        assert_eq!(ctx.read().rotation_targets.len(), 0);
    }
}
