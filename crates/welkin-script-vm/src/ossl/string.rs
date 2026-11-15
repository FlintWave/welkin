// SPDX-License-Identifier: GPL-3.0-or-later
//! OSSL string and regex functions (VeryLow threat level).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use regex::Regex;

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;

pub(crate) fn string_functions() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osMatchString",
            level: OsslThreatLevel::VeryLow,
            register: register_os_match_string,
        },
        OsslFunc {
            name: "osReplaceString",
            level: OsslThreatLevel::VeryLow,
            register: register_os_replace_string,
        },
        OsslFunc {
            name: "osStringSubString",
            level: OsslThreatLevel::VeryLow,
            register: register_os_string_sub_string,
        },
        OsslFunc {
            name: "osStringStartsWith",
            level: OsslThreatLevel::VeryLow,
            register: register_os_string_starts_with,
        },
        OsslFunc {
            name: "osStringEndsWith",
            level: OsslThreatLevel::VeryLow,
            register: register_os_string_ends_with,
        },
        OsslFunc {
            name: "osStringIndexOf",
            level: OsslThreatLevel::VeryLow,
            register: register_os_string_index_of,
        },
        OsslFunc {
            name: "osStringLastIndexOf",
            level: OsslThreatLevel::VeryLow,
            register: register_os_string_last_index_of,
        },
        OsslFunc {
            name: "osStringRemove",
            level: OsslThreatLevel::VeryLow,
            register: register_os_string_remove,
        },
        OsslFunc {
            name: "osStringReplace",
            level: OsslThreatLevel::VeryLow,
            register: register_os_string_replace,
        },
        OsslFunc {
            name: "osRegexIsMatch",
            level: OsslThreatLevel::VeryLow,
            register: register_os_regex_is_match,
        },
    ]
}

// ---------------------------------------------------------------------------
// osMatchString — returns list of [index, match] pairs using regex
// ---------------------------------------------------------------------------

fn register_os_match_string(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, pattern, start): (String, String, i32)| {
        let re = Regex::new(&pattern)
            .map_err(|e| mlua::Error::RuntimeError(format!("osMatchString: bad regex: {e}")))?;

        let start_byte = byte_offset_from_char(&src, start.max(0) as usize);
        let haystack = &src[start_byte..];

        let mut results: Vec<String> = Vec::new();
        for m in re.find_iter(haystack) {
            // Report char index relative to original string
            let char_idx = src[..start_byte + m.start()].chars().count();
            results.push(char_idx.to_string());
            results.push(m.as_str().to_string());
        }
        Ok(results)
    })?;
    lua.globals().set("osMatchString", f)
}

// ---------------------------------------------------------------------------
// osReplaceString — regex replace
// ---------------------------------------------------------------------------

fn register_os_replace_string(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (src, pattern, replace, count, start): (String, String, String, i32, i32)| {
            let re = Regex::new(&pattern).map_err(|e| {
                mlua::Error::RuntimeError(format!("osReplaceString: bad regex: {e}"))
            })?;

            let start_byte = byte_offset_from_char(&src, start.max(0) as usize);
            let (prefix, haystack) = src.split_at(start_byte);

            let replaced = if count <= 0 {
                re.replace_all(haystack, replace.as_str()).into_owned()
            } else {
                re.replacen(haystack, count as usize, replace.as_str())
                    .into_owned()
            };

            Ok(format!("{prefix}{replaced}"))
        },
    )?;
    lua.globals().set("osReplaceString", f)
}

// ---------------------------------------------------------------------------
// osStringSubString — substring with graceful bounds
// ---------------------------------------------------------------------------

fn register_os_string_sub_string(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, start, length): (String, i32, i32)| {
        let chars: Vec<char> = src.chars().collect();
        let len = chars.len() as i32;
        let start = start.clamp(0, len) as usize;
        let end = (start as i32 + length).clamp(start as i32, len) as usize;
        Ok(chars[start..end].iter().collect::<String>())
    })?;
    lua.globals().set("osStringSubString", f)
}

// ---------------------------------------------------------------------------
// osStringStartsWith / osStringEndsWith
// ---------------------------------------------------------------------------

fn register_os_string_starts_with(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, prefix): (String, String)| {
        Ok(if src.starts_with(&prefix) { 1i32 } else { 0 })
    })?;
    lua.globals().set("osStringStartsWith", f)
}

fn register_os_string_ends_with(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, suffix): (String, String)| {
        Ok(if src.ends_with(&suffix) { 1i32 } else { 0 })
    })?;
    lua.globals().set("osStringEndsWith", f)
}

// ---------------------------------------------------------------------------
// osStringIndexOf / osStringLastIndexOf
// ---------------------------------------------------------------------------

fn register_os_string_index_of(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, value, ignore_case): (String, String, i32)| {
        let (haystack, needle);
        if ignore_case != 0 {
            haystack = src.to_lowercase();
            needle = value.to_lowercase();
        } else {
            haystack = src.clone();
            needle = value;
        }
        match haystack.find(&needle) {
            Some(byte_pos) => {
                let char_idx = haystack[..byte_pos].chars().count();
                Ok(char_idx as i32)
            }
            None => Ok(-1i32),
        }
    })?;
    lua.globals().set("osStringIndexOf", f)
}

fn register_os_string_last_index_of(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, value, ignore_case): (String, String, i32)| {
        let (haystack, needle);
        if ignore_case != 0 {
            haystack = src.to_lowercase();
            needle = value.to_lowercase();
        } else {
            haystack = src.clone();
            needle = value;
        }
        match haystack.rfind(&needle) {
            Some(byte_pos) => {
                let char_idx = haystack[..byte_pos].chars().count();
                Ok(char_idx as i32)
            }
            None => Ok(-1i32),
        }
    })?;
    lua.globals().set("osStringLastIndexOf", f)
}

// ---------------------------------------------------------------------------
// osStringRemove — remove `count` chars starting at `start`
// ---------------------------------------------------------------------------

fn register_os_string_remove(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, start, count): (String, i32, i32)| {
        let chars: Vec<char> = src.chars().collect();
        let len = chars.len() as i32;
        let start = start.clamp(0, len) as usize;
        let count = count.clamp(0, len - start as i32) as usize;
        let mut result: Vec<char> = Vec::with_capacity(chars.len() - count);
        result.extend_from_slice(&chars[..start]);
        result.extend_from_slice(&chars[start + count..]);
        Ok(result.into_iter().collect::<String>())
    })?;
    lua.globals().set("osStringRemove", f)
}

// ---------------------------------------------------------------------------
// osStringReplace — literal (non-regex) replace
// ---------------------------------------------------------------------------

fn register_os_string_replace(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (src, old, new): (String, String, String)| {
        Ok(src.replace(&old, &new))
    })?;
    lua.globals().set("osStringReplace", f)
}

// ---------------------------------------------------------------------------
// osRegexIsMatch
// ---------------------------------------------------------------------------

fn register_os_regex_is_match(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (input, pattern): (String, String)| {
        let re = Regex::new(&pattern)
            .map_err(|e| mlua::Error::RuntimeError(format!("osRegexIsMatch: bad regex: {e}")))?;
        Ok(if re.is_match(&input) { 1i32 } else { 0 })
    })?;
    lua.globals().set("osRegexIsMatch", f)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a char index to a byte offset, clamping to string length.
fn byte_offset_from_char(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(byte, _)| byte)
        .unwrap_or(s.len())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::ScriptContext;
    use crate::ossl::{OsslConfig, OsslThreatLevel, register_ossl};
    use uuid::Uuid;

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            ..ScriptContext::default()
        }))
    }

    fn make_string_vm() -> Lua {
        let lua = Lua::new();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::VeryLow,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();
        lua
    }

    #[test]
    fn test_match_string_basic() {
        let lua = make_string_vm();
        let r: Vec<String> = lua
            .load(r#"return osMatchString("hello world", "\\w+", 0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, vec!["0", "hello", "6", "world"]);
    }

    #[test]
    fn test_match_string_with_offset() {
        let lua = make_string_vm();
        let r: Vec<String> = lua
            .load(r#"return osMatchString("hello world", "\\w+", 6)"#)
            .eval()
            .unwrap();
        assert_eq!(r, vec!["6", "world"]);
    }

    #[test]
    fn test_replace_string() {
        let lua = make_string_vm();
        let r: String = lua
            .load(r#"return osReplaceString("aaa bbb ccc", "[a-z]+", "X", 2, 0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "X X ccc");
    }

    #[test]
    fn test_replace_string_all() {
        let lua = make_string_vm();
        let r: String = lua
            .load(r#"return osReplaceString("aaa bbb ccc", "[a-z]+", "X", 0, 0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "X X X");
    }

    #[test]
    fn test_substring() {
        let lua = make_string_vm();
        let r: String = lua
            .load(r#"return osStringSubString("Hello World", 6, 5)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "World");
    }

    #[test]
    fn test_substring_out_of_bounds() {
        let lua = make_string_vm();
        let r: String = lua
            .load(r#"return osStringSubString("Hi", 0, 100)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "Hi");
    }

    #[test]
    fn test_starts_ends_with() {
        let lua = make_string_vm();
        let r: i32 = lua
            .load(r#"return osStringStartsWith("Hello", "He")"#)
            .eval()
            .unwrap();
        assert_eq!(r, 1);

        let r: i32 = lua
            .load(r#"return osStringStartsWith("Hello", "lo")"#)
            .eval()
            .unwrap();
        assert_eq!(r, 0);

        let r: i32 = lua
            .load(r#"return osStringEndsWith("Hello", "lo")"#)
            .eval()
            .unwrap();
        assert_eq!(r, 1);
    }

    #[test]
    fn test_index_of() {
        let lua = make_string_vm();
        let r: i32 = lua
            .load(r#"return osStringIndexOf("Hello World", "World", 0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, 6);

        let r: i32 = lua
            .load(r#"return osStringIndexOf("Hello World", "xyz", 0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, -1);
    }

    #[test]
    fn test_index_of_ignore_case() {
        let lua = make_string_vm();
        let r: i32 = lua
            .load(r#"return osStringIndexOf("Hello World", "hello", 1)"#)
            .eval()
            .unwrap();
        assert_eq!(r, 0);
    }

    #[test]
    fn test_last_index_of() {
        let lua = make_string_vm();
        let r: i32 = lua
            .load(r#"return osStringLastIndexOf("abcabc", "abc", 0)"#)
            .eval()
            .unwrap();
        assert_eq!(r, 3);
    }

    #[test]
    fn test_string_remove() {
        let lua = make_string_vm();
        let r: String = lua
            .load(r#"return osStringRemove("Hello World", 5, 6)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "Hello");
    }

    #[test]
    fn test_string_replace() {
        let lua = make_string_vm();
        let r: String = lua
            .load(r#"return osStringReplace("Hello World", "World", "Rust")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "Hello Rust");
    }

    #[test]
    fn test_regex_is_match() {
        let lua = make_string_vm();
        let r: i32 = lua
            .load(r#"return osRegexIsMatch("hello123", "\\d+")"#)
            .eval()
            .unwrap();
        assert_eq!(r, 1);

        let r: i32 = lua
            .load(r#"return osRegexIsMatch("hello", "\\d+")"#)
            .eval()
            .unwrap();
        assert_eq!(r, 0);
    }

    #[test]
    fn test_unicode_substring() {
        let lua = make_string_vm();
        // Unicode string: each char counts as 1
        let r: String = lua
            .load(r#"return osStringSubString("héllo", 1, 4)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "éllo");
    }
}
