// SPDX-License-Identifier: LGPL-3.0-or-later
//! MOD API — module invocation functions for the Luau script VM.
//!
//! These functions let scripts call into registered region modules via a
//! callback registry (`modInvoke*`) or send commands to the module subsystem
//! (`modSendCommand`).
//!
//! Reference: `OpenSim/Region/ScriptEngine/Shared/Api/Implementation/MOD_Api.cs`

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use crate::builtins::{ModCommand, ScriptContext};

const NULL_KEY: &str = "00000000-0000-0000-0000-000000000000";

/// Convert Lua variadic arguments into a `Vec<serde_json::Value>`.
fn lua_args_to_json(args: mlua::MultiValue) -> Vec<serde_json::Value> {
    args.into_iter()
        .map(|v| match v {
            mlua::Value::Integer(n) => serde_json::Value::Number(n.into()),
            mlua::Value::Number(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            mlua::Value::String(s) => {
                serde_json::Value::String(s.to_string_lossy().to_string())
            }
            mlua::Value::Boolean(b) => serde_json::Value::Bool(b),
            _ => serde_json::Value::Null,
        })
        .collect()
}

/// Register all `modInvoke*` and `modSendCommand` functions as Lua globals.
pub fn register_mod_api(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    // modInvokeN(fname, ...) — call callback, return nothing
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (fname, args): (String, mlua::MultiValue)| {
            let json_args = lua_args_to_json(args);
            let cb = {
                let guard = ctx.read();
                guard.mod_invoke_registry.get(&fname).cloned()
            };
            if let Some(cb) = cb {
                cb(&json_args);
            } else {
                tracing::warn!(fname = %fname, "modInvokeN: function not found in registry");
            }
            Ok(())
        })?;
        lua.globals().set("modInvokeN", f)?;
    }

    // modInvokeS(fname, ...) — call callback, return string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (fname, args): (String, mlua::MultiValue)| {
            let json_args = lua_args_to_json(args);
            let cb = {
                let guard = ctx.read();
                guard.mod_invoke_registry.get(&fname).cloned()
            };
            if let Some(cb) = cb {
                let result = cb(&json_args);
                match result {
                    serde_json::Value::String(s) => Ok(s),
                    other => Ok(other.to_string()),
                }
            } else {
                tracing::warn!(fname = %fname, "modInvokeS: function not found in registry");
                Ok(String::new())
            }
        })?;
        lua.globals().set("modInvokeS", f)?;
    }

    // modInvokeI(fname, ...) — call callback, return integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (fname, args): (String, mlua::MultiValue)| {
            let json_args = lua_args_to_json(args);
            let cb = {
                let guard = ctx.read();
                guard.mod_invoke_registry.get(&fname).cloned()
            };
            if let Some(cb) = cb {
                let result = cb(&json_args);
                Ok(result.as_i64().unwrap_or(0))
            } else {
                tracing::warn!(fname = %fname, "modInvokeI: function not found in registry");
                Ok(0i64)
            }
        })?;
        lua.globals().set("modInvokeI", f)?;
    }

    // modInvokeF(fname, ...) — call callback, return float
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (fname, args): (String, mlua::MultiValue)| {
            let json_args = lua_args_to_json(args);
            let cb = {
                let guard = ctx.read();
                guard.mod_invoke_registry.get(&fname).cloned()
            };
            if let Some(cb) = cb {
                let result = cb(&json_args);
                Ok(result.as_f64().unwrap_or(0.0))
            } else {
                tracing::warn!(fname = %fname, "modInvokeF: function not found in registry");
                Ok(0.0f64)
            }
        })?;
        lua.globals().set("modInvokeF", f)?;
    }

    // modInvokeK(fname, ...) — call callback, return key (UUID string)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (fname, args): (String, mlua::MultiValue)| {
            let json_args = lua_args_to_json(args);
            let cb = {
                let guard = ctx.read();
                guard.mod_invoke_registry.get(&fname).cloned()
            };
            if let Some(cb) = cb {
                let result = cb(&json_args);
                match result {
                    serde_json::Value::String(s) => Ok(s),
                    _ => Ok(NULL_KEY.to_string()),
                }
            } else {
                tracing::warn!(fname = %fname, "modInvokeK: function not found in registry");
                Ok(NULL_KEY.to_string())
            }
        })?;
        lua.globals().set("modInvokeK", f)?;
    }

    // modInvokeV(fname, ...) — call callback, return vector {x, y, z}
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, (fname, args): (String, mlua::MultiValue)| {
            let json_args = lua_args_to_json(args);
            let cb = {
                let guard = ctx.read();
                guard.mod_invoke_registry.get(&fname).cloned()
            };
            let (x, y, z) = if let Some(cb) = cb {
                let result = cb(&json_args);
                if let serde_json::Value::Array(ref arr) = result {
                    if arr.len() >= 3 {
                        (
                            arr[0].as_f64().unwrap_or(0.0),
                            arr[1].as_f64().unwrap_or(0.0),
                            arr[2].as_f64().unwrap_or(0.0),
                        )
                    } else {
                        (0.0, 0.0, 0.0)
                    }
                } else {
                    (0.0, 0.0, 0.0)
                }
            } else {
                tracing::warn!(fname = %fname, "modInvokeV: function not found in registry");
                (0.0, 0.0, 0.0)
            };
            let tbl = lua.create_table()?;
            tbl.set("x", x)?;
            tbl.set("y", y)?;
            tbl.set("z", z)?;
            Ok(tbl)
        })?;
        lua.globals().set("modInvokeV", f)?;
    }

    // modInvokeR(fname, ...) — call callback, return rotation {x, y, z, w}
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, (fname, args): (String, mlua::MultiValue)| {
            let json_args = lua_args_to_json(args);
            let cb = {
                let guard = ctx.read();
                guard.mod_invoke_registry.get(&fname).cloned()
            };
            let (x, y, z, w) = if let Some(cb) = cb {
                let result = cb(&json_args);
                if let serde_json::Value::Array(ref arr) = result {
                    if arr.len() >= 4 {
                        (
                            arr[0].as_f64().unwrap_or(0.0),
                            arr[1].as_f64().unwrap_or(0.0),
                            arr[2].as_f64().unwrap_or(0.0),
                            arr[3].as_f64().unwrap_or(1.0),
                        )
                    } else {
                        (0.0, 0.0, 0.0, 1.0)
                    }
                } else {
                    (0.0, 0.0, 0.0, 1.0)
                }
            } else {
                tracing::warn!(fname = %fname, "modInvokeR: function not found in registry");
                (0.0, 0.0, 0.0, 1.0)
            };
            let tbl = lua.create_table()?;
            tbl.set("x", x)?;
            tbl.set("y", y)?;
            tbl.set("z", z)?;
            tbl.set("w", w)?;
            Ok(tbl)
        })?;
        lua.globals().set("modInvokeR", f)?;
    }

    // modInvokeL(fname, ...) — call callback, return list table
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |lua, (fname, args): (String, mlua::MultiValue)| {
            let json_args = lua_args_to_json(args);
            let cb = {
                let guard = ctx.read();
                guard.mod_invoke_registry.get(&fname).cloned()
            };
            let tbl = lua.create_table()?;
            if let Some(cb) = cb {
                let result = cb(&json_args);
                if let serde_json::Value::Array(arr) = result {
                    for (i, val) in arr.into_iter().enumerate() {
                        match val {
                            serde_json::Value::Number(n) => {
                                if let Some(iv) = n.as_i64() {
                                    tbl.set(i + 1, iv)?;
                                } else if let Some(fv) = n.as_f64() {
                                    tbl.set(i + 1, fv)?;
                                }
                            }
                            serde_json::Value::String(s) => {
                                tbl.set(i + 1, s)?;
                            }
                            serde_json::Value::Bool(b) => {
                                tbl.set(i + 1, b)?;
                            }
                            _ => {
                                tbl.set(i + 1, mlua::Value::Nil)?;
                            }
                        }
                    }
                }
            } else {
                tracing::warn!(fname = %fname, "modInvokeL: function not found in registry");
            }
            Ok(tbl)
        })?;
        lua.globals().set("modInvokeL", f)?;
    }

    // modSendCommand(module, command, k) — send ModCommand, return key
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (module, command, k): (String, String, String)| {
                let guard = ctx.read();
                if let Some(ref tx) = guard.mod_command_sender {
                    let cmd = ModCommand {
                        module,
                        command,
                        key: k.clone(),
                    };
                    let _ = tx.send(cmd);
                } else {
                    tracing::debug!("modSendCommand: no command sender configured");
                }
                Ok(k)
            },
        )?;
        lua.globals().set("modSendCommand", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn make_test_vm() -> (Lua, Arc<RwLock<ScriptContext>>) {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(ScriptContext::default()));
        register_mod_api(&lua, &ctx).unwrap();
        (lua, ctx)
    }

    #[test]
    fn test_mod_invoke_s_with_callback() {
        let (lua, ctx) = make_test_vm();
        ctx.write().mod_invoke_registry.insert(
            "testFunc".to_string(),
            Arc::new(|_args: &[serde_json::Value]| {
                serde_json::Value::String("hello".into())
            }),
        );

        let result: String = lua
            .load(r#"return modInvokeS("testFunc", "arg1", 42)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_mod_invoke_i_with_callback() {
        let (lua, ctx) = make_test_vm();
        ctx.write().mod_invoke_registry.insert(
            "getCount".to_string(),
            Arc::new(|_args: &[serde_json::Value]| {
                serde_json::json!(42)
            }),
        );

        let result: i64 = lua
            .load(r#"return modInvokeI("getCount")"#)
            .eval()
            .unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_mod_invoke_n_with_callback() {
        let (lua, ctx) = make_test_vm();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        ctx.write().mod_invoke_registry.insert(
            "doThing".to_string(),
            Arc::new(move |_args: &[serde_json::Value]| {
                called_clone.store(true, Ordering::SeqCst);
                serde_json::Value::Null
            }),
        );

        lua.load(r#"modInvokeN("doThing", "x")"#).exec().unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_mod_send_command() {
        let (lua, ctx) = make_test_vm();
        let (tx, rx) = std::sync::mpsc::channel();
        ctx.write().mod_command_sender = Some(tx);

        let key: String = lua
            .load(r#"return modSendCommand("myModule", "doAction", "abc-123")"#)
            .eval()
            .unwrap();
        assert_eq!(key, "abc-123");

        let cmd = rx.recv().unwrap();
        assert_eq!(cmd.module, "myModule");
        assert_eq!(cmd.command, "doAction");
        assert_eq!(cmd.key, "abc-123");
    }

    #[test]
    fn test_mod_invoke_defaults_when_not_found() {
        let (lua, _ctx) = make_test_vm();

        let s: String = lua
            .load(r#"return modInvokeS("nonexistent")"#)
            .eval()
            .unwrap();
        assert_eq!(s, "");

        let i: i64 = lua
            .load(r#"return modInvokeI("nonexistent")"#)
            .eval()
            .unwrap();
        assert_eq!(i, 0);

        let f: f64 = lua
            .load(r#"return modInvokeF("nonexistent")"#)
            .eval()
            .unwrap();
        assert!((f - 0.0).abs() < f64::EPSILON);

        let k: String = lua
            .load(r#"return modInvokeK("nonexistent")"#)
            .eval()
            .unwrap();
        assert_eq!(k, "00000000-0000-0000-0000-000000000000");
    }
}
