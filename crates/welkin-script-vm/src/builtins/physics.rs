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
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, offset_tbl: LuaTable| {
            let offset = get_vec3(&offset_tbl)?;
            let ctx = ctx.read();
            let qx = (ctx.position[0] + offset[0]).clamp(0.0, 255.0) as usize;
            let qy = (ctx.position[1] + offset[1]).clamp(0.0, 255.0) as usize;
            match ctx.terrain_heights {
                Some(ref heights) => {
                    let idx = qy * 256 + qx;
                    Ok(heights.get(idx).copied().unwrap_or(0.0) as f64)
                }
                None => Ok(0.0f64),
            }
        })?;
        lua.globals().set("llGround", f)?;
    }

    // llGroundNormal(offset) -> vector (terrain normal at position+offset)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, offset_tbl: LuaTable| {
            let offset = get_vec3(&offset_tbl)?;
            let ctx = ctx.read();
            let heights = match ctx.terrain_heights {
                Some(ref h) => h,
                None => return make_vec3(lua, [0.0, 0.0, 1.0]),
            };
            let qx = (ctx.position[0] + offset[0]).clamp(0.0, 255.0) as usize;
            let qy = (ctx.position[1] + offset[1]).clamp(0.0, 255.0) as usize;
            let sample = |x: usize, y: usize| -> f64 {
                let cx = x.min(255);
                let cy = y.min(255);
                heights.get(cy * 256 + cx).copied().unwrap_or(0.0) as f64
            };
            let h_l = sample(qx.saturating_sub(1), qy);
            let h_r = sample((qx + 1).min(255), qy);
            let h_d = sample(qx, qy.saturating_sub(1));
            let h_u = sample(qx, (qy + 1).min(255));
            let nx = h_l - h_r;
            let ny = h_d - h_u;
            let nz = 2.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > f64::EPSILON {
                make_vec3(lua, [nx / len, ny / len, nz / len])
            } else {
                make_vec3(lua, [0.0, 0.0, 1.0])
            }
        })?;
        lua.globals().set("llGroundNormal", f)?;
    }

    // llWater(offset) -> float (water height)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _offset: LuaValue| {
            let ctx = ctx.read();
            Ok(ctx.water_height)
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
    fn ll_ground_no_terrain_returns_zero() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();
        let r: f64 = lua.load("return llGround({x=0, y=0, z=0})").eval().unwrap();
        assert!((r - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_ground_queries_terrain_heightmap() {
        let lua = Lua::new();
        let ctx = test_ctx();
        {
            let mut c = ctx.write();
            // Position is (128, 128, 25). Offset (0,0,0) queries (128, 128).
            let mut heights = vec![0.0f32; 256 * 256];
            heights[128 * 256 + 128] = 42.5;
            c.terrain_heights = Some(heights);
        }
        register_physics(&lua, &ctx).unwrap();
        let r: f64 = lua.load("return llGround({x=0, y=0, z=0})").eval().unwrap();
        assert!((r - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_ground_applies_offset() {
        let lua = Lua::new();
        let ctx = test_ctx();
        {
            let mut c = ctx.write();
            let mut heights = vec![0.0f32; 256 * 256];
            // Position (128,128) + offset (2,3) = (130, 131)
            heights[131 * 256 + 130] = 17.0;
            c.terrain_heights = Some(heights);
        }
        register_physics(&lua, &ctx).unwrap();
        let r: f64 = lua.load("return llGround({x=2, y=3, z=0})").eval().unwrap();
        assert!((r - 17.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_ground_clamps_to_bounds() {
        let lua = Lua::new();
        let ctx = test_ctx();
        {
            let mut c = ctx.write();
            let mut heights = vec![0.0f32; 256 * 256];
            // Position (128,128) + offset (500,500) should clamp to (255, 255)
            heights[255 * 256 + 255] = 99.0;
            c.terrain_heights = Some(heights);
        }
        register_physics(&lua, &ctx).unwrap();
        let r: f64 = lua.load("return llGround({x=500, y=500, z=0})").eval().unwrap();
        assert!((r - 99.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_ground_normal_no_terrain_returns_up() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_physics(&lua, &ctx).unwrap();
        let (x, y, z): (f64, f64, f64) = lua
            .load("local n = llGroundNormal({x=0, y=0, z=0}); return n.x, n.y, n.z")
            .eval()
            .unwrap();
        assert!(x.abs() < f64::EPSILON);
        assert!(y.abs() < f64::EPSILON);
        assert!((z - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_ground_normal_flat_terrain_returns_up() {
        let lua = Lua::new();
        let ctx = test_ctx();
        {
            let mut c = ctx.write();
            // Flat terrain at height 10 everywhere
            c.terrain_heights = Some(vec![10.0f32; 256 * 256]);
        }
        register_physics(&lua, &ctx).unwrap();
        let (x, y, z): (f64, f64, f64) = lua
            .load("local n = llGroundNormal({x=0, y=0, z=0}); return n.x, n.y, n.z")
            .eval()
            .unwrap();
        assert!(x.abs() < f64::EPSILON);
        assert!(y.abs() < f64::EPSILON);
        assert!((z - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ll_ground_normal_sloped_terrain() {
        let lua = Lua::new();
        let ctx = test_ctx();
        {
            let mut c = ctx.write();
            // Create terrain that slopes in x: height = x
            let mut heights = vec![0.0f32; 256 * 256];
            for y in 0..256usize {
                for x in 0..256usize {
                    heights[y * 256 + x] = x as f32;
                }
            }
            c.terrain_heights = Some(heights);
        }
        register_physics(&lua, &ctx).unwrap();
        // At position (128,128), hL=127, hR=129, hD=128, hU=128
        // normal = normalize(127-129, 128-128, 2) = normalize(-2, 0, 2)
        let (x, y, z): (f64, f64, f64) = lua
            .load("local n = llGroundNormal({x=0, y=0, z=0}); return n.x, n.y, n.z")
            .eval()
            .unwrap();
        // Expected: (-2, 0, 2) normalized = (-1/sqrt(2), 0, 1/sqrt(2))
        let expected_len = (2.0f64 * 2.0 + 2.0 * 2.0).sqrt();
        assert!((x - (-2.0 / expected_len)).abs() < 1e-10);
        assert!(y.abs() < 1e-10);
        assert!((z - (2.0 / expected_len)).abs() < 1e-10);
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
    fn ll_water_returns_configured_height() {
        let lua = Lua::new();
        let ctx = test_ctx();
        {
            ctx.write().water_height = 35.5;
        }
        register_physics(&lua, &ctx).unwrap();
        let r: f64 = lua.load("return llWater({x=0, y=0, z=0})").eval().unwrap();
        assert!((r - 35.5).abs() < f64::EPSILON);
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
