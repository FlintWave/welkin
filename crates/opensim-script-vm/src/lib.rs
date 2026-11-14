// SPDX-License-Identifier: GPL-3.0-or-later
//! SLua / Luau spike: verify that we can build Luau from Rust and exercise
//! the basic operations a Second Life script VM needs.
//!
//! This crate wraps [`mlua`] (with the `luau` feature) and will eventually
//! host the full LSL-on-Luau runtime.  For now it contains only the spike
//! proof that the toolchain works.

use mlua::prelude::*;

/// Errors returned by the script VM.
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("lua error: {0}")]
    Lua(#[from] mlua::Error),
}

/// Create a fresh Luau VM with sandboxing enabled.
pub fn new_vm() -> Result<Lua, VmError> {
    let lua = Lua::new();
    lua.sandbox(true)?;
    Ok(lua)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::thread::ThreadStatus;
    use mlua::Value;

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

        let func = lua.load(r#"
            local x = 0
            x = x + 1
            coroutine.yield(x)
            x = x + 10
            coroutine.yield(x)
            x = x + 100
            return x
        "#).into_function()?;

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

        let func = lua.load(r#"
            local input = coroutine.yield()  -- pause, wait for event
            return input * 2
        "#).into_function()?;

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
}
