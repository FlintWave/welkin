// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL string built-in functions registered as Lua globals.

use mlua::prelude::*;

/// Resolve an LSL-style index (0-based, negative counts from end) into a
/// clamped `usize` suitable for indexing a `Vec<char>` of length `len`.
/// Returns `None` when the string is empty.
fn resolve_index(idx: i64, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let len_i = len as i64;
    let resolved = if idx < 0 { len_i + idx } else { idx };
    Some(resolved.clamp(0, len_i - 1) as usize)
}

/// Register all LSL string functions on `lua`.
pub fn register_string(lua: &Lua) -> Result<(), crate::VmError> {
    let g = lua.globals();

    // llToUpper
    let f = lua.create_function(|_, s: String| Ok(s.to_uppercase()))?;
    g.set("llToUpper", f)?;

    // llToLower
    let f = lua.create_function(|_, s: String| Ok(s.to_lowercase()))?;
    g.set("llToLower", f)?;

    // llStringLength — byte length, matching LSL behaviour
    let f = lua.create_function(|_, s: String| Ok(s.len() as i64))?;
    g.set("llStringLength", f)?;

    // llSubStringIndex
    let f = lua.create_function(|_, (source, pattern): (String, String)| {
        Ok(source.find(&*pattern).map(|i| i as i64).unwrap_or(-1))
    })?;
    g.set("llSubStringIndex", f)?;

    // llGetSubString — 0-based inclusive indices, negative wraps from end
    let f = lua.create_function(|_, (s, start, end): (String, i64, i64)| {
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        if len == 0 {
            return Ok(String::new());
        }
        let si = match resolve_index(start, len) {
            Some(v) => v,
            None => return Ok(String::new()),
        };
        let ei = match resolve_index(end, len) {
            Some(v) => v,
            None => return Ok(String::new()),
        };
        if si > ei {
            return Ok(String::new());
        }
        Ok(chars[si..=ei].iter().collect())
    })?;
    g.set("llGetSubString", f)?;

    // llDeleteSubString — remove chars in [start..=end], return the rest
    let f = lua.create_function(|_, (s, start, end): (String, i64, i64)| {
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        if len == 0 {
            return Ok(String::new());
        }
        let si = match resolve_index(start, len) {
            Some(v) => v,
            None => return Ok(s),
        };
        let ei = match resolve_index(end, len) {
            Some(v) => v,
            None => return Ok(s),
        };
        if si > ei {
            return Ok(s);
        }
        let mut result: Vec<char> = Vec::with_capacity(len - (ei - si + 1));
        result.extend_from_slice(&chars[..si]);
        if ei + 1 < len {
            result.extend_from_slice(&chars[ei + 1..]);
        }
        Ok(result.into_iter().collect())
    })?;
    g.set("llDeleteSubString", f)?;

    // llInsertString
    let f = lua.create_function(|_, (dest, pos, src): (String, i64, String)| {
        let chars: Vec<char> = dest.chars().collect();
        let len = chars.len();
        let insert_at = if pos < 0 {
            0usize
        } else if (pos as usize) > len {
            len
        } else {
            pos as usize
        };
        let mut result = String::with_capacity(dest.len() + src.len());
        result.extend(chars[..insert_at].iter());
        result.push_str(&src);
        result.extend(chars[insert_at..].iter());
        Ok(result)
    })?;
    g.set("llInsertString", f)?;

    // llStringTrim — 1=HEAD, 2=TAIL, 3=BOTH
    let f = lua.create_function(|_, (s, trim_type): (String, i64)| {
        Ok(match trim_type {
            1 => s.trim_start().to_string(),
            2 => s.trim_end().to_string(),
            _ => s.trim().to_string(), // 3 or any other value => both
        })
    })?;
    g.set("llStringTrim", f)?;

    // llSHA1String
    let f = lua.create_function(|_, s: String| {
        let hash = sha1_smol::Sha1::from(&s).hexdigest();
        Ok(hash)
    })?;
    g.set("llSHA1String", f)?;

    // llMD5String(s, nonce) — MD5 of "{s}:{nonce}"
    let f = lua.create_function(|_, (s, nonce): (String, i64)| {
        use md5::Digest;
        let mut hasher = md5::Md5::new();
        hasher.update(format!("{}:{}", s, nonce));
        let result = hasher.finalize();
        let hex: String = result.iter().map(|b| format!("{:02x}", b)).collect();
        Ok(hex)
    })?;
    g.set("llMD5String", f)?;

    // llBase64ToString
    let f = lua.create_function(|_, s: String| {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&s)
            .unwrap_or_default();
        Ok(String::from_utf8(bytes).unwrap_or_default())
    })?;
    g.set("llBase64ToString", f)?;

    // llStringToBase64
    let f = lua.create_function(|_, s: String| {
        use base64::Engine;
        Ok(base64::engine::general_purpose::STANDARD.encode(&s))
    })?;
    g.set("llStringToBase64", f)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a plain (non-sandboxed) Lua VM with string builtins.
    fn vm() -> Lua {
        let lua = Lua::new();
        register_string(&lua).expect("register_string");
        lua
    }

    #[test]
    fn to_upper() {
        let lua = vm();
        let r: String = lua.load(r#"return llToUpper("hello")"#).eval().unwrap();
        assert_eq!(r, "HELLO");
    }

    #[test]
    fn to_lower() {
        let lua = vm();
        let r: String = lua.load(r#"return llToLower("WORLD")"#).eval().unwrap();
        assert_eq!(r, "world");
    }

    #[test]
    fn string_length() {
        let lua = vm();
        let r: i64 = lua.load(r#"return llStringLength("test")"#).eval().unwrap();
        assert_eq!(r, 4);
    }

    #[test]
    fn sub_string_index_found() {
        let lua = vm();
        let r: i64 = lua
            .load(r#"return llSubStringIndex("hello world", "world")"#)
            .eval()
            .unwrap();
        assert_eq!(r, 6);
    }

    #[test]
    fn sub_string_index_not_found() {
        let lua = vm();
        let r: i64 = lua
            .load(r#"return llSubStringIndex("hello", "xyz")"#)
            .eval()
            .unwrap();
        assert_eq!(r, -1);
    }

    #[test]
    fn get_sub_string() {
        let lua = vm();
        let r: String = lua
            .load(r#"return llGetSubString("Hello", 0, 2)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "Hel");
    }

    #[test]
    fn get_sub_string_negative_index() {
        let lua = vm();
        let r: String = lua
            .load(r#"return llGetSubString("Hello", -3, -1)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "llo");
    }

    #[test]
    fn delete_sub_string() {
        let lua = vm();
        let r: String = lua
            .load(r#"return llDeleteSubString("Hello World", 5, 5)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "HelloWorld");
    }

    #[test]
    fn insert_string() {
        let lua = vm();
        let r: String = lua
            .load(r#"return llInsertString("hello", 5, " world")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "hello world");
    }

    #[test]
    fn string_trim() {
        let lua = vm();
        let r: String = lua
            .load(r#"return llStringTrim("  hello  ", 3)"#)
            .eval()
            .unwrap();
        assert_eq!(r, "hello");
    }

    #[test]
    fn sha1_string() {
        let lua = vm();
        let r: String = lua
            .load(r#"return llSHA1String("Hello")"#)
            .eval()
            .unwrap();
        // SHA-1 of "Hello"
        assert_eq!(r, "f7ff9e8b7bb2e09b70935a5d785e0cc5d9d0abf0");
    }

    #[test]
    fn md5_string() {
        let lua = vm();
        let r: String = lua
            .load(r#"return llMD5String("Hello", 0)"#)
            .eval()
            .unwrap();
        // MD5 of "Hello:0"
        let expected = {
            use md5::Digest;
            let mut h = md5::Md5::new();
            h.update("Hello:0");
            let res = h.finalize();
            res.iter().map(|b| format!("{:02x}", b)).collect::<String>()
        };
        assert_eq!(r, expected);
    }

    #[test]
    fn base64_round_trip() {
        let lua = vm();
        let encoded: String = lua
            .load(r#"return llStringToBase64("Hello World")"#)
            .eval()
            .unwrap();
        assert_eq!(encoded, "SGVsbG8gV29ybGQ=");

        let decoded: String = lua
            .load(r#"return llBase64ToString("SGVsbG8gV29ybGQ=")"#)
            .eval()
            .unwrap();
        assert_eq!(decoded, "Hello World");
    }
}
