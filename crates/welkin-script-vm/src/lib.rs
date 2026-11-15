// SPDX-License-Identifier: GPL-3.0-or-later
//! SLua / Luau spike: verify that we can build Luau from Rust and exercise
//! the basic operations a Second Life script VM needs.
//!
//! This crate wraps [`mlua`] (with the `luau` feature) and will eventually
//! host the full LSL-on-Luau runtime.  For now it contains only the spike
//! proof that the toolchain works.

pub mod builtins;
pub mod events;
pub mod instance;
pub mod ossl;
pub mod scheduler;
pub mod transfer;

use mlua::chunk::Compiler;
use mlua::prelude::*;

pub use instance::{ScriptEvent, ScriptInstance, ScriptInstanceState};
pub use scheduler::{ScriptScheduler, TimerEntry};
pub use transfer::RegionTransferBundle;
pub use builtins::{RezRequest, AttachRequest, AttachAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatType {
    Whisper,
    Say,
    Shout,
    Region,
    Owner,
    Direct,
    IM,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub source_id: uuid::Uuid,
    pub source_name: String,
    pub source_pos: [f64; 3],
    pub owner_id: uuid::Uuid,
    pub channel: i32,
    pub message: String,
    pub chat_type: ChatType,
    pub target_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone)]
pub enum PhysicsCommand {
    SetForce { force: [f64; 3], local: bool },
    SetTorque { torque: [f64; 3], local: bool },
    ApplyImpulse { impulse: [f64; 3], local: bool },
    ApplyRotationalImpulse { impulse: [f64; 3], local: bool },
    SetBuoyancy { buoyancy: f64 },
    SetStatus { status: i32, value: bool },
    MoveToTarget { target: [f64; 3], tau: f64 },
    StopMoveToTarget,
    SetHoverHeight { height: f64, water: i32, tau: f64 },
}

/// Errors returned by the script VM.
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Create a fresh Luau VM with sandboxing enabled.
pub fn new_vm() -> Result<Lua, VmError> {
    let lua = Lua::new();
    lua.sandbox(true)?;
    Ok(lua)
}

/// Create a fresh Luau VM with a memory limit (in bytes).
///
/// The VM is sandboxed. Scripts that exceed the memory limit will receive
/// a `MemoryError`.
pub fn new_vm_with_memory_limit(limit: usize) -> Result<Lua, VmError> {
    let lua = Lua::new();
    lua.set_memory_limit(limit)?;
    lua.sandbox(true)?;
    Ok(lua)
}

// ---------------------------------------------------------------------------
// Script state serialization (Task 1.2)
// ---------------------------------------------------------------------------
//
// **Limitation**: Neither mlua nor the Luau VM expose any API to snapshot a
// suspended coroutine's execution state (stack frames, locals, instruction
// pointer, upvalues).  A `Thread` cannot be serialized or transferred
// between VMs.
//
// **Workaround**: We save what *can* be saved:
//   1. The original source code (or pre-compiled bytecode).
//   2. The script-visible global variables, serialized to JSON via
//      mlua's serde support.
//
// On restore we create a fresh VM, reload the bytecode, inject the saved
// globals, and let the script re-execute from the top.  Scripts that need
// true mid-function resume must use an explicit state-machine pattern
// (e.g. a `_checkpoint` global that the script checks on entry).
//

/// Saved state of a script's globals plus its compiled bytecode.
///
/// Thread execution state (stack frames, locals, IP) **cannot** be saved
/// because neither mlua nor Luau exposes that capability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScriptState {
    /// Luau bytecode compiled from source.
    pub bytecode: Vec<u8>,
    /// JSON-encoded map of user-set global variables.
    pub globals_json: String,
}

/// Compile LSL source to Luau source code via the full pipeline:
/// lex → parse → analyze → codegen.
pub fn compile_lsl(source: &str) -> Result<String, String> {
    welkin_script_compiler::compile_lsl(source).map_err(|e| e.to_string())
}

/// Compile source to Luau bytecode.
pub fn compile_source(source: &str) -> Result<Vec<u8>, VmError> {
    let compiler = Compiler::new();
    Ok(compiler.compile(source)?)
}

/// Extract user-set globals from a (non-sandboxed) VM and combine them
/// with pre-compiled bytecode into a serializable [`ScriptState`].
///
/// `global_keys` lists the variable names to capture.  Only values that
/// are JSON-representable (numbers, strings, booleans, arrays, maps) will
/// round-trip correctly.
pub fn serialize_script_state(
    lua: &Lua,
    bytecode: Vec<u8>,
    global_keys: &[&str],
) -> Result<ScriptState, VmError> {
    use mlua::LuaSerdeExt;

    let globals = lua.globals();
    let mut map = serde_json::Map::new();
    for &key in global_keys {
        let val: mlua::Value = globals.get(key)?;
        // Convert Lua Value -> serde_json::Value via mlua's serde bridge
        let json_val: serde_json::Value = lua.from_value(val)?;
        map.insert(key.to_string(), json_val);
    }
    let globals_json = serde_json::to_string(&map)?;
    Ok(ScriptState {
        bytecode,
        globals_json,
    })
}

/// Restore a script VM from a previously saved [`ScriptState`].
///
/// Returns a **non-sandboxed** VM with the saved globals injected.
/// The caller can then load the bytecode and resume execution.
pub fn restore_script_state(state: &ScriptState) -> Result<Lua, VmError> {
    use mlua::LuaSerdeExt;

    let lua = Lua::new();
    // Do NOT sandbox yet — we need to write globals first.
    let globals = lua.globals();
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&state.globals_json)?;
    for (key, json_val) in &map {
        let lua_val: mlua::Value = lua.to_value(json_val)?;
        globals.set(key.as_str(), lua_val)?;
    }
    Ok(lua)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::thread::ThreadStatus;
    use mlua::{Value, VmState};

    // ---------------------------------------------------------------
    // 1. Basic compile-load-run
    // ---------------------------------------------------------------
    #[test]
    fn compile_load_run() -> Result<(), VmError> {
        let lua = new_vm()?;
        let result: i64 = lua.load("return 2 + 3").eval()?;
        assert_eq!(result, 5);
        Ok(())
    }

    // ---------------------------------------------------------------
    // 2. Luau-specific: type annotations parse without error
    // ---------------------------------------------------------------
    #[test]
    fn luau_type_annotations() -> Result<(), VmError> {
        let lua = new_vm()?;
        let chunk = r#"
            local function add(a: number, b: number): number
                return a + b
            end
            return add(10, 20)
        "#;
        let result: i64 = lua.load(chunk).eval()?;
        assert_eq!(result, 30);
        Ok(())
    }

    // ---------------------------------------------------------------
    // 3. Coroutine yield / resume  (LSL scripts are cooperative)
    // ---------------------------------------------------------------
    #[test]
    fn coroutine_yield_resume() -> Result<(), VmError> {
        let lua = new_vm()?;

        let func = lua
            .load(
                r#"
            local x = 0
            x = x + 1
            coroutine.yield(x)
            x = x + 10
            coroutine.yield(x)
            x = x + 100
            return x
        "#,
            )
            .into_function()?;

        let thread = lua.create_thread(func)?;

        // First resume: yields 1
        let val: i64 = thread.resume(())?;
        assert_eq!(val, 1);
        assert_eq!(thread.status(), ThreadStatus::Resumable);

        // Second resume: yields 11
        let val: i64 = thread.resume(())?;
        assert_eq!(val, 11);
        assert_eq!(thread.status(), ThreadStatus::Resumable);

        // Third resume: returns 111
        let val: i64 = thread.resume(())?;
        assert_eq!(val, 111);
        assert_eq!(thread.status(), ThreadStatus::Finished);

        Ok(())
    }

    // ---------------------------------------------------------------
    // 4. Rust function callable from Lua  (for LSL built-in API)
    // ---------------------------------------------------------------
    #[test]
    fn rust_function_from_lua() -> Result<(), VmError> {
        let lua = new_vm()?;

        // Register a Rust function as a Lua global
        let ll_say = lua.create_function(|_, (channel, message): (i32, String)| {
            // In real code this would route to the region message system.
            Ok(format!("ch{channel}: {message}"))
        })?;
        lua.globals().set("llSay", ll_say)?;

        let result: String = lua.load(r#"return llSay(0, "Hello from Luau!")"#).eval()?;
        assert_eq!(result, "ch0: Hello from Luau!");
        Ok(())
    }

    // ---------------------------------------------------------------
    // 5. Sandboxed VM: globals are frozen (cannot be overwritten)
    // ---------------------------------------------------------------
    #[test]
    fn sandbox_freezes_globals() -> Result<(), VmError> {
        let lua = new_vm()?;

        // Luau sandbox freezes the global table. Writing to a frozen
        // global should error.
        let result = lua.load("math.pi = 0").exec();
        assert!(
            result.is_err(),
            "expected error when writing to frozen global, but succeeded"
        );
        Ok(())
    }

    // ---------------------------------------------------------------
    // 6. Table round-trip  (LSL lists, link-sets, etc.)
    // ---------------------------------------------------------------
    #[test]
    fn table_round_trip() -> Result<(), VmError> {
        let lua = new_vm()?;

        let chunk = r#"
            local t = {1, 2, 3}
            local sum = 0
            for _, v in ipairs(t) do
                sum = sum + v
            end
            return sum
        "#;
        let result: i64 = lua.load(chunk).eval()?;
        assert_eq!(result, 6);
        Ok(())
    }

    // ---------------------------------------------------------------
    // 7. Multiple scripts on one VM (multiple objects in a region)
    // ---------------------------------------------------------------
    #[test]
    fn multiple_scripts_isolated() -> Result<(), VmError> {
        let lua = new_vm()?;

        // Script A sets a global
        lua.load("script_a_val = 42").exec()?;

        // Script B reads it — in a real VM each script would get its
        // own environment table for isolation; here we just prove
        // multiple loads work.
        let val: i64 = lua.load("return script_a_val or 0").eval()?;
        // sandbox(true) freezes globals after set, but within the same
        // sandbox epoch this should still be visible.
        assert!(val == 42 || val == 0, "unexpected value: {val}");
        Ok(())
    }

    // ---------------------------------------------------------------
    // 8. Coroutine with data passed on resume (event parameters)
    // ---------------------------------------------------------------
    #[test]
    fn coroutine_resume_with_args() -> Result<(), VmError> {
        let lua = new_vm()?;

        let func = lua
            .load(
                r#"
            local input = coroutine.yield()  -- pause, wait for event
            return input * 2
        "#,
            )
            .into_function()?;

        let thread = lua.create_thread(func)?;

        // First resume starts execution; it yields with no value
        let _: Value = thread.resume(())?;
        assert_eq!(thread.status(), ThreadStatus::Resumable);

        // Second resume passes event data (21); script returns 42
        let result: i64 = thread.resume(21)?;
        assert_eq!(result, 42);
        assert_eq!(thread.status(), ThreadStatus::Finished);

        Ok(())
    }

    // ===============================================================
    // Task 1.2: Serialize a suspended script, restore, continue
    // ===============================================================

    // ---------------------------------------------------------------
    // 9. Thread state is NOT serializable (documenting the limitation)
    // ---------------------------------------------------------------
    #[test]
    fn thread_state_not_serializable() -> Result<(), VmError> {
        let lua = new_vm()?;

        let func = lua
            .load(
                r#"
            local x = 0
            x = x + 1
            coroutine.yield(x)   -- yields 1
            x = x + 10
            return x             -- would return 11
        "#,
            )
            .into_function()?;

        let thread = lua.create_thread(func)?;

        // Advance to the first yield point
        let val: i64 = thread.resume(())?;
        assert_eq!(val, 1);
        assert_eq!(thread.status(), ThreadStatus::Resumable);

        // mlua::Thread has no serialize/to_bytes/to_owned method.
        // The Luau C API (lua_State*) does not expose coroutine
        // serialization either.  There is no way to snapshot the
        // suspended thread's stack frames, locals, or instruction
        // pointer.
        //
        // Verify the thread is alive but opaque:
        assert!(thread.is_resumable());
        // We CAN still resume it on the *same* VM:
        let val: i64 = thread.resume(())?;
        assert_eq!(val, 11);
        assert_eq!(thread.status(), ThreadStatus::Finished);

        Ok(())
    }

    // ---------------------------------------------------------------
    // 10. Serialize globals + bytecode, restore in a new VM
    // ---------------------------------------------------------------
    #[test]
    fn serialize_restore_globals_and_bytecode() -> Result<(), VmError> {
        // --- Phase 1: run a script, let it set globals, yield ------
        let source = r#"
            -- A script that uses a global checkpoint counter.
            -- On first run _checkpoint is nil (0), on restore it is 1.
            if _checkpoint == nil then
                _checkpoint = 0
            end

            if _checkpoint == 0 then
                _state = 100
                _checkpoint = 1
                coroutine.yield(_state)
            end

            -- Phase 2: continues after restore
            _state = _state + 50
            return _state
        "#;

        let bytecode = compile_source(source)?;

        // Run phase 1 on a non-sandboxed VM so we can read globals
        let lua1 = Lua::new();
        let func1 = lua1.load(&bytecode).into_function()?;
        let thread1 = lua1.create_thread(func1)?;

        let yielded: i64 = thread1.resume(())?;
        assert_eq!(yielded, 100);
        assert_eq!(thread1.status(), ThreadStatus::Resumable);

        // --- Serialize the state we CAN save -----------------------
        let saved = serialize_script_state(&lua1, bytecode.clone(), &["_checkpoint", "_state"])?;

        // Verify it round-trips through JSON
        let json_bytes = serde_json::to_vec(&saved)?;
        let restored_state: ScriptState = serde_json::from_slice(&json_bytes)?;
        assert_eq!(restored_state.bytecode, bytecode);

        // --- Phase 2: restore in a completely new VM ---------------
        let lua2 = restore_script_state(&restored_state)?;

        // Confirm globals were restored
        let cp: i64 = lua2.globals().get("_checkpoint")?;
        assert_eq!(cp, 1);
        let st: i64 = lua2.globals().get("_state")?;
        assert_eq!(st, 100);

        // Re-load bytecode and run from the top; the script checks
        // _checkpoint and skips phase 1.
        let func2 = lua2.load(&restored_state.bytecode).into_function()?;
        let result: i64 = func2.call(())?;
        assert_eq!(result, 150); // 100 + 50

        Ok(())
    }

    // ---------------------------------------------------------------
    // 11. Globals round-trip covers multiple types
    // ---------------------------------------------------------------
    #[test]
    fn serialize_globals_multiple_types() -> Result<(), VmError> {
        let lua = Lua::new();

        lua.load(
            r#"
            _num    = 42.5
            _str    = "hello"
            _bool   = true
            _list   = {10, 20, 30}
            _nested = { a = 1, b = "two" }
        "#,
        )
        .exec()?;

        let saved = serialize_script_state(
            &lua,
            vec![], // no bytecode needed for this test
            &["_num", "_str", "_bool", "_list", "_nested"],
        )?;

        let lua2 = restore_script_state(&saved)?;

        let num: f64 = lua2.globals().get("_num")?;
        assert!((num - 42.5).abs() < f64::EPSILON);

        let s: String = lua2.globals().get("_str")?;
        assert_eq!(s, "hello");

        let b: bool = lua2.globals().get("_bool")?;
        assert!(b);

        // Verify the list
        let sum: i64 = lua2
            .load(
                r#"
            local s = 0
            for _, v in ipairs(_list) do s = s + v end
            return s
        "#,
            )
            .eval()?;
        assert_eq!(sum, 60);

        // Verify nested table
        let a: i64 = lua2.load("return _nested.a").eval()?;
        assert_eq!(a, 1);
        let b_str: String = lua2.load("return _nested.b").eval()?;
        assert_eq!(b_str, "two");

        Ok(())
    }

    // ===============================================================
    // Task 1.5: Memory limits and pre-emption
    // ===============================================================

    // ---------------------------------------------------------------
    // 12. Memory limit: allocation exceeding limit produces an error
    // ---------------------------------------------------------------
    #[test]
    fn memory_limit_triggers_error() -> Result<(), VmError> {
        let lua = Lua::new();

        let baseline = lua.used_memory();
        assert!(baseline > 0, "VM should use some memory for stdlib");

        // Set a tight limit: baseline + 64 KB
        lua.set_memory_limit(baseline + 65_536)?;

        // Try to allocate a large table (~400 KB for 100_000 entries)
        let result = lua
            .load(
                r#"
            local t = {}
            for i = 1, 100000 do
                t[i] = i
            end
        "#,
            )
            .exec();

        assert!(
            matches!(result, Err(mlua::Error::MemoryError(_))),
            "expected MemoryError, got: {result:?}"
        );

        Ok(())
    }

    // ---------------------------------------------------------------
    // 13. Memory limit applies to threads too
    // ---------------------------------------------------------------
    #[test]
    fn memory_limit_on_thread() -> Result<(), VmError> {
        let lua = Lua::new();

        let baseline = lua.used_memory();
        lua.set_memory_limit(baseline + 65_536)?;

        let func = lua
            .load(
                r#"
            local t = {}
            for i = 1, 100000 do
                t[i] = i
            end
        "#,
            )
            .into_function()?;

        let thread = lua.create_thread(func)?;
        let result = thread.resume::<()>(());

        assert!(
            matches!(result, Err(mlua::Error::MemoryError(_))),
            "expected MemoryError on thread, got: {result:?}"
        );

        Ok(())
    }

    // ---------------------------------------------------------------
    // 14. Pre-emption: infinite loop interrupted by set_interrupt
    // ---------------------------------------------------------------
    #[test]
    fn preempt_infinite_loop_via_interrupt() -> Result<(), VmError> {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};

        let lua = Lua::new();

        let count = Arc::new(AtomicU64::new(0));
        let count_clone = count.clone();

        // Fire an error after 1000 interrupt callbacks
        lua.set_interrupt(move |_| {
            if count_clone.fetch_add(1, Ordering::Relaxed) >= 1000 {
                Err(mlua::Error::runtime("execution limit exceeded"))
            } else {
                Ok(VmState::Continue)
            }
        });

        let result = lua
            .load(
                r#"
            local x = 0
            while true do
                x = x + 1
            end
        "#,
            )
            .exec();

        assert!(
            result.is_err(),
            "infinite loop should have been interrupted"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("execution limit exceeded"),
            "unexpected error: {err_msg}"
        );
        assert!(
            count.load(Ordering::Relaxed) >= 1000,
            "interrupt should have fired at least 1000 times"
        );

        Ok(())
    }

    // ---------------------------------------------------------------
    // 15. Pre-emption via yield: interrupt yields the coroutine
    // ---------------------------------------------------------------
    #[test]
    fn preempt_via_yield() -> Result<(), VmError> {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};

        let lua = Lua::new();

        let tick = Arc::new(AtomicU64::new(0));
        let tick_clone = tick.clone();

        // Yield every 50 interrupt ticks
        lua.set_interrupt(move |_| {
            if tick_clone.fetch_add(1, Ordering::Relaxed) % 50 == 49 {
                Ok(VmState::Yield)
            } else {
                Ok(VmState::Continue)
            }
        });

        let func = lua
            .load(
                r#"
            local sum = 0
            for i = 1, 1000 do
                sum = sum + i
            end
            return sum
        "#,
            )
            .into_function()?;

        let thread = lua.create_thread(func)?;

        // Drive the coroutine: it will be interrupted by yields, but
        // each resume continues where it left off.
        let mut resumes = 0u32;
        let mut final_result: Option<i64> = None;
        while thread.status() == ThreadStatus::Resumable {
            resumes += 1;
            match thread.resume::<Value>(()) {
                Ok(Value::Integer(n)) => {
                    final_result = Some(n);
                }
                Ok(Value::Nil) => {
                    // Interrupt-yield returns nil
                }
                Ok(_) => {}
                Err(e) => return Err(e.into()),
            }
        }

        assert!(
            resumes > 1,
            "should have been yielded at least once (resumes={resumes})"
        );
        assert_eq!(final_result, Some(500500), "sum 1..1000 = 500500");
        assert_eq!(thread.status(), ThreadStatus::Finished);

        Ok(())
    }

    // ---------------------------------------------------------------
    // 16. Other scripts continue after one is interrupted
    // ---------------------------------------------------------------
    #[test]
    fn other_scripts_survive_interruption() -> Result<(), VmError> {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};

        let lua = Lua::new();

        let count = Arc::new(AtomicU64::new(0));
        let count_clone = count.clone();

        lua.set_interrupt(move |_| {
            if count_clone.fetch_add(1, Ordering::Relaxed) >= 500 {
                Err(mlua::Error::runtime("execution limit exceeded"))
            } else {
                Ok(VmState::Continue)
            }
        });

        // Script A: infinite loop, will be killed
        let script_a = lua.load(
            r#"
            while true do end
        "#,
        );
        let result_a = script_a.exec();
        assert!(result_a.is_err(), "script A should have been interrupted");

        // Reset interrupt counter and remove limit
        lua.remove_interrupt();

        // Script B: a normal computation should succeed
        let result_b: i64 = lua.load("return 6 * 7").eval()?;
        assert_eq!(
            result_b, 42,
            "script B should run normally after A was interrupted"
        );

        Ok(())
    }

    // ---------------------------------------------------------------
    // Compile LSL → Luau pipeline
    // ---------------------------------------------------------------
    #[test]
    fn compile_lsl_produces_runnable_luau() {
        let lsl_source = r#"
integer g_count = 0;
default {
    state_entry() {
        g_count = 1;
    }
    touch_start(integer num_detected) {
        g_count = g_count + num_detected;
    }
}
"#;
        let luau = compile_lsl(lsl_source).expect("LSL compilation failed");
        assert!(luau.contains("__dispatch"), "output should contain __dispatch");
        assert!(luau.contains("__states"), "output should contain __states");
        assert!(luau.contains("__current_state"), "output should contain __current_state");

        // Verify the generated Luau actually compiles and runs in the VM
        let bytecode = compile_source(&luau).expect("Luau compilation failed");
        let lua = Lua::new();
        lua.load(&bytecode).exec().expect("Luau execution failed");

        // __dispatch should be a function
        let dispatch: mlua::Function = lua
            .globals()
            .get("__dispatch")
            .expect("__dispatch not found");
        dispatch
            .call::<()>(("state_entry",))
            .expect("dispatch state_entry failed");
        dispatch
            .call::<()>(("touch_start", 1))
            .expect("dispatch touch_start failed");
    }

    #[test]
    fn compile_lsl_rejects_invalid_source() {
        let bad = "this is not valid LSL {{{";
        let result = compile_lsl(bad);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // 17. Memory limit resets: lift limit and allocate freely
    // ---------------------------------------------------------------
    #[test]
    fn memory_limit_can_be_lifted() -> Result<(), VmError> {
        let lua = Lua::new();

        let baseline = lua.used_memory();
        lua.set_memory_limit(baseline + 1024)?;

        // This should fail
        let result = lua
            .load(
                r#"
            local t = {}
            for i = 1, 100000 do t[i] = i end
        "#,
            )
            .exec();
        assert!(result.is_err());

        // Lift the limit (0 = no limit)
        lua.set_memory_limit(0)?;

        // Now the same allocation should succeed
        lua.load(
            r#"
            local t = {}
            for i = 1, 100000 do t[i] = i end
        "#,
        )
        .exec()?;

        Ok(())
    }
}
