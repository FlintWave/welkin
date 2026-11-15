// SPDX-License-Identifier: GPL-3.0-or-later
//! Task 4.2: A scripted vehicle crosses a region border and keeps running.
//!
//! This is a conceptual proof that the checkpoint-based state transfer works
//! for a vehicle script that maintains position, velocity and heading in
//! globals. The flow:
//!
//! 1. Create a script with vehicle state in globals.
//! 2. Run phase 1 (accelerate): advance it a few ticks, verify state.
//! 3. Serialize the script state with `serialize_script_state()`.
//! 4. Deserialize into a new VM (destination region) with `restore_script_state()`.
//! 5. Continue the script (phase 2), verify it picks up where it left off.
//! 6. Verify vehicle state globals are correct across the transfer.

use welkin_script_vm::{
    compile_source, restore_script_state, serialize_script_state, ScriptState, VmError,
};

/// The vehicle script uses a checkpoint pattern to survive serialization.
///
/// Globals:
///   _checkpoint -- which phase to resume from
///   pos_x, pos_y -- position
///   vel_x, vel_y -- velocity
///   heading      -- direction angle (radians)
const VEHICLE_SCRIPT: &str = r#"
-- Vehicle state globals (initialized on first run if nil)
if _checkpoint == nil then _checkpoint = 0 end
if pos_x == nil then pos_x = 128.0 end
if pos_y == nil then pos_y = 128.0 end
if vel_x == nil then vel_x = 5.0 end
if vel_y == nil then vel_y = 0.0 end
if heading == nil then heading = 0.0 end

if _checkpoint == 0 then
    -- Phase 1: accelerate
    vel_x = vel_x + 1.0
    pos_x = pos_x + vel_x
    _checkpoint = 1
    coroutine.yield(pos_x)
end

if _checkpoint == 1 then
    -- Phase 2: continue after region transfer
    pos_x = pos_x + vel_x
    heading = heading + 0.1
    _checkpoint = 2
end

return pos_x
"#;

/// Global names the vehicle script uses.
const VEHICLE_GLOBALS: &[&str] = &[
    "_checkpoint",
    "pos_x",
    "pos_y",
    "vel_x",
    "vel_y",
    "heading",
];

// ===================================================================
// Main proof: vehicle crosses a region border
// ===================================================================

#[test]
fn vehicle_crosses_region_border() -> Result<(), VmError> {
    let tolerance = 0.000001;

    // --- Phase 1: Source region (accelerate) ---

    let bytecode = compile_source(VEHICLE_SCRIPT)?;
    let lua1 = mlua::Lua::new();

    let func1 = lua1.load(&bytecode).into_function()?;
    let thread1 = lua1.create_thread(func1)?;

    // First resume: runs phase 1, yields pos_x
    let yielded: f64 = thread1.resume(())?;

    // After phase 1: vel_x was 5.0 + 1.0 = 6.0, pos_x was 128.0 + 6.0 = 134.0
    assert!(
        (yielded - 134.0).abs() < tolerance,
        "phase 1 pos_x: expected 134.0, got {yielded}"
    );

    // Verify globals on source VM
    let cp: i64 = lua1.globals().get("_checkpoint")?;
    assert_eq!(cp, 1, "checkpoint should be 1 after phase 1");

    let px: f64 = lua1.globals().get("pos_x")?;
    assert!((px - 134.0).abs() < tolerance, "pos_x should be 134.0");

    let vx: f64 = lua1.globals().get("vel_x")?;
    assert!((vx - 6.0).abs() < tolerance, "vel_x should be 6.0");

    let h: f64 = lua1.globals().get("heading")?;
    assert!((h - 0.0).abs() < tolerance, "heading should be 0.0");

    // --- Serialize (simulating region border crossing) ---

    let saved = serialize_script_state(&lua1, bytecode.clone(), VEHICLE_GLOBALS)?;

    // Verify the serialized state can round-trip through JSON
    let json_bytes = serde_json::to_vec(&saved).map_err(VmError::Serde)?;
    assert!(!json_bytes.is_empty(), "serialized state should not be empty");

    let restored_state: ScriptState =
        serde_json::from_slice(&json_bytes).map_err(VmError::Serde)?;
    assert_eq!(
        restored_state.bytecode, bytecode,
        "bytecode should survive round-trip"
    );

    // --- Phase 2: Destination region (continue driving) ---

    let lua2 = restore_script_state(&restored_state)?;

    // Verify globals were restored before re-execution
    let cp2: i64 = lua2.globals().get("_checkpoint")?;
    assert_eq!(cp2, 1, "restored checkpoint should be 1");

    let px2: f64 = lua2.globals().get("pos_x")?;
    assert!(
        (px2 - 134.0).abs() < tolerance,
        "restored pos_x should be 134.0"
    );

    let vx2: f64 = lua2.globals().get("vel_x")?;
    assert!((vx2 - 6.0).abs() < tolerance, "restored vel_x should be 6.0");

    // Re-load bytecode and run from the top.
    // The script checks _checkpoint and skips phase 1, entering phase 2.
    let func2 = lua2.load(&restored_state.bytecode).into_function()?;
    let result: f64 = func2.call(())?;

    // Phase 2: pos_x = 134.0 + 6.0 = 140.0
    assert!(
        (result - 140.0).abs() < tolerance,
        "phase 2 pos_x: expected 140.0, got {result}"
    );

    // Verify all vehicle state after phase 2
    let final_px: f64 = lua2.globals().get("pos_x")?;
    assert!(
        (final_px - 140.0).abs() < tolerance,
        "final pos_x should be 140.0"
    );

    let final_heading: f64 = lua2.globals().get("heading")?;
    assert!(
        (final_heading - 0.1).abs() < tolerance,
        "final heading should be 0.1"
    );

    let final_cp: i64 = lua2.globals().get("_checkpoint")?;
    assert_eq!(final_cp, 2, "final checkpoint should be 2");

    Ok(())
}

// ===================================================================
// Edge case: transfer at checkpoint 0 (nothing has run yet)
// ===================================================================

#[test]
fn vehicle_transfer_before_any_execution() -> Result<(), VmError> {
    let tolerance = 0.000001;

    let bytecode = compile_source(VEHICLE_SCRIPT)?;
    let lua1 = mlua::Lua::new();

    // Initialize globals to defaults WITHOUT running the script
    lua1.globals().set("_checkpoint", 0)?;
    lua1.globals().set("pos_x", 128.0)?;
    lua1.globals().set("pos_y", 128.0)?;
    lua1.globals().set("vel_x", 5.0)?;
    lua1.globals().set("vel_y", 0.0)?;
    lua1.globals().set("heading", 0.0)?;

    // Serialize immediately
    let saved = serialize_script_state(&lua1, bytecode.clone(), VEHICLE_GLOBALS)?;

    // Restore and run from scratch -- should execute both phases
    let lua2 = restore_script_state(&saved)?;

    let cp: i64 = lua2.globals().get("_checkpoint")?;
    assert_eq!(cp, 0, "checkpoint should still be 0 before execution");

    // Run as coroutine to get phase 1 yield
    let func = lua2.load(&saved.bytecode).into_function()?;
    let thread = lua2.create_thread(func)?;

    let yielded: f64 = thread.resume(())?;
    assert!(
        (yielded - 134.0).abs() < tolerance,
        "phase 1 yield should be 134.0, got {yielded}"
    );

    // Resume for phase 2
    let final_val: f64 = thread.resume(())?;
    assert!(
        (final_val - 140.0).abs() < tolerance,
        "phase 2 return should be 140.0, got {final_val}"
    );
    Ok(())
}

// ===================================================================
// Multiple ticks: run vehicle for several cycles before transfer
// ===================================================================

/// A multi-tick vehicle script that advances one step per call.
const TICK_VEHICLE: &str = r#"
if pos_x == nil then pos_x = 128.0 end
if vel_x == nil then vel_x = 5.0 end
if tick_count == nil then tick_count = 0 end

-- Each execution is one physics tick
pos_x = pos_x + vel_x
tick_count = tick_count + 1
return pos_x
"#;

#[test]
fn vehicle_multiple_ticks_then_transfer() -> Result<(), VmError> {
    let tolerance = 0.000001;
    let globals = &["pos_x", "vel_x", "tick_count"];

    let bytecode = compile_source(TICK_VEHICLE)?;
    let lua1 = mlua::Lua::new();

    // Run 3 ticks on source region
    for _ in 0..3 {
        lua1.load(&bytecode).exec()?;
    }

    // pos_x should be 128 + 5*3 = 143
    let px: f64 = lua1.globals().get("pos_x")?;
    assert!((px - 143.0).abs() < tolerance, "after 3 ticks: expected 143.0, got {px}");

    let tc: i64 = lua1.globals().get("tick_count")?;
    assert_eq!(tc, 3, "tick_count should be 3");

    // Serialize and transfer
    let saved = serialize_script_state(&lua1, bytecode.clone(), globals)?;
    let lua2 = restore_script_state(&saved)?;

    // Verify restored state
    let rpx: f64 = lua2.globals().get("pos_x")?;
    assert!((rpx - 143.0).abs() < tolerance, "restored pos_x mismatch");

    // Run 2 more ticks on destination region
    for _ in 0..2 {
        lua2.load(&saved.bytecode).exec()?;
    }

    // pos_x should be 143 + 5*2 = 153
    let final_px: f64 = lua2.globals().get("pos_x")?;
    assert!(
        (final_px - 153.0).abs() < tolerance,
        "after transfer + 2 ticks: expected 153.0, got {final_px}"
    );

    let final_tc: i64 = lua2.globals().get("tick_count")?;
    assert_eq!(final_tc, 5, "total tick_count should be 5 (3 + 2)");

    Ok(())
}

// ===================================================================
// Full transfer bundle round-trip using RegionTransferBundle
// ===================================================================

#[test]
fn vehicle_transfer_via_bundle() -> Result<(), VmError> {
    use welkin_script_vm::{ScriptInstance, ScriptScheduler, RegionTransferBundle, ScriptEvent};
    use std::time::Duration;
    use uuid::Uuid;

    // -- Source region --
    let source_script = r#"
        if pos_x == nil then pos_x = 128.0 end
        if vel_x == nil then vel_x = 5.0 end
        _counter = 0

        function timer()
            pos_x = pos_x + vel_x
            _counter = _counter + 1
        end
    "#;

    let region_a = Uuid::new_v4();
    let sid = Uuid::new_v4();
    let pid = Uuid::new_v4();

    let mut sched_a = ScriptScheduler::new(10_000);
    let mut inst = ScriptInstance::new(sid, pid, source_script, 0)?;
    inst.track_global("pos_x");
    inst.track_global("vel_x");
    inst.track_global("_counter");

    // Fire a timer event
    inst.post_event(ScriptEvent::Timer);
    sched_a.add_script(inst);
    sched_a.set_timer(sid, Duration::from_secs(1));

    // Process the event
    let errors = sched_a.tick();
    assert!(errors.is_empty(), "tick errors: {errors:?}");

    // -- Extract transfer bundle --
    let bundle = RegionTransferBundle::from_scheduler(&mut sched_a, &[sid], region_a)?;
    assert_eq!(bundle.script_states.len(), 1);
    assert_eq!(sched_a.script_count(), 0, "source should have no scripts after transfer");

    // -- Serialize / deserialize the bundle (simulating network transfer) --
    let bytes = bundle.serialize_to_bytes()?;
    let bundle2 = RegionTransferBundle::deserialize_from_bytes(&bytes)?;

    // -- Destination region --
    let mut sched_b = ScriptScheduler::new(10_000);
    bundle2.apply_to_scheduler(&mut sched_b)?;

    assert_eq!(sched_b.script_count(), 1);
    assert!(
        sched_b.timer_interval(&sid).is_some(),
        "timer should be restored"
    );

    // Fire another timer and tick
    sched_b.post_event(sid, ScriptEvent::Timer);
    let errors = sched_b.tick();
    assert!(errors.is_empty(), "destination tick errors: {errors:?}");

    Ok(())
}

// ===================================================================
// Verify state is preserved for different value types
// ===================================================================

#[test]
fn vehicle_preserves_mixed_types() -> Result<(), VmError> {
    let source = r#"
        if name == nil then name = "MyVehicle" end
        if active == nil then active = true end
        if speed == nil then speed = 42.5 end
        if passengers == nil then passengers = {"Alice", "Bob"} end
        return name
    "#;

    let globals = &["name", "active", "speed", "passengers"];
    let bytecode = compile_source(source)?;
    let lua1 = mlua::Lua::new();
    lua1.load(&bytecode).exec()?;

    let saved = serialize_script_state(&lua1, bytecode.clone(), globals)?;
    let lua2 = restore_script_state(&saved)?;

    let name: String = lua2.globals().get("name")?;
    assert_eq!(name, "MyVehicle");

    let active: bool = lua2.globals().get("active")?;
    assert!(active);

    let speed: f64 = lua2.globals().get("speed")?;
    assert!((speed - 42.5).abs() < 0.000001);

    // Verify nested table (passengers list) survived
    let count: i64 = lua2
        .load(r#"
            local c = 0
            for _ in ipairs(passengers) do c = c + 1 end
            return c
        "#)
        .eval()?;
    assert_eq!(count, 2, "passengers list should have 2 entries");

    let p1: String = lua2.load("return passengers[1]").eval()?;
    assert_eq!(p1, "Alice");

    let p2: String = lua2.load("return passengers[2]").eval()?;
    assert_eq!(p2, "Bob");

    Ok(())
}
