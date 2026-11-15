// SPDX-License-Identifier: LGPL-3.0-or-later
//! LSL string built-in functions registered as Lua globals.

use mlua::prelude::*;

/// Convert a Lua value to its string representation (mirrors list.rs value_to_string).
fn lua_value_to_string(v: &LuaValue) -> String {
    match v {
        LuaValue::Integer(n) => n.to_string(),
        LuaValue::Number(n) => format!("{n:.6}"),
        LuaValue::String(s) => s.to_string_lossy().to_string(),
        LuaValue::Boolean(b) => if *b { "1" } else { "0" }.to_string(),
        LuaValue::Nil => String::new(),
        _ => String::new(),
    }
}

/// Collect string values from a Lua table's sequential entries.
fn collect_string_entries(t: &LuaTable) -> Vec<String> {
    let len = t.raw_len();
    let mut out = Vec::with_capacity(len);
    for i in 1..=len {
        if let Ok(v) = t.raw_get::<LuaValue>(i) {
            let s = lua_value_to_string(&v);
            if !s.is_empty() {
                out.push(s);
            }
        }
    }
    out
}

/// Internal implementation of llParseString2List / llParseStringKeepNulls.
fn parse_string_to_list(
    src: &str,
    separators: &[String],
    spacers: &[String],
    keep_nulls: bool,
) -> Vec<String> {
    let src_len = src.len();
    let sep_count = separators.len();

    // Combine separators and spacers into one delimiter array.
    // Indices < sep_count are separators; >= sep_count are spacers.
    let mut delimiters: Vec<Option<&str>> = Vec::with_capacity(sep_count + spacers.len());
    for s in separators {
        delimiters.push(Some(s.as_str()));
    }
    for s in spacers {
        delimiters.push(Some(s.as_str()));
    }

    let mut output: Vec<String> = Vec::new();
    let mut i = 0;

    loop {
        // Find the earliest delimiter at or after position i.
        let mut earliest_del: Option<usize> = None;
        let mut earliest_src = src_len;
        let mut earliest_str: &str = "";

        for (j, delim_opt) in delimiters.iter_mut().enumerate() {
            if let Some(d) = delim_opt {
                if let Some(pos) = src[i..].find(*d) {
                    let abs_pos = i + pos;
                    if abs_pos < earliest_src {
                        earliest_src = abs_pos;
                        earliest_del = Some(j);
                        earliest_str = d;
                        if abs_pos == i {
                            break; // can't do better
                        }
                    }
                } else {
                    // This delimiter doesn't appear any more; stop checking it.
                    *delim_opt = None;
                }
            }
        }

        // Output the text from i to the start of the earliest delimiter.
        if keep_nulls || earliest_src > i {
            output.push(src[i..earliest_src].to_string());
        }

        // If no delimiter found, we're done.
        let del_idx = match earliest_del {
            Some(idx) => idx,
            None => break,
        };

        // If the delimiter is a spacer (index >= sep_count), output it.
        if del_idx >= sep_count {
            output.push(earliest_str.to_string());
        }

        i = earliest_src + earliest_str.len();
    }

    output
}

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

    // llSHA256String(s, nonce) — SHA-256 of "{s}:{nonce}" as lowercase hex
    let f = lua.create_function(|_, (s, nonce): (String, i64)| {
        use sha2::{Digest as _, Sha256};
        let mut h = Sha256::new();
        h.update(format!("{}:{}", s, nonce));
        Ok(h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>())
    })?;
    g.set("llSHA256String", f)?;

    // llXorBase64(s1, s2) — XOR two base64 strings, cycling shorter
    let f = lua.create_function(|_, (s1, s2): (String, String)| {
        use base64::Engine;
        let b1 = base64::engine::general_purpose::STANDARD.decode(&s1).unwrap_or_default();
        let b2 = base64::engine::general_purpose::STANDARD.decode(&s2).unwrap_or_default();
        let (long, short) = if b1.len() >= b2.len() { (b1, b2) } else { (b2, b1) };
        let result: Vec<u8> = if short.is_empty() {
            long
        } else {
            long.iter().enumerate().map(|(i, b)| b ^ short[i % short.len()]).collect()
        };
        Ok(base64::engine::general_purpose::STANDARD.encode(&result))
    })?;
    g.set("llXorBase64", f)?;

    // llIntegerToBase64(val) — 4 big-endian bytes, base64-encoded
    let f = lua.create_function(|_, val: i64| {
        use base64::Engine;
        let bytes = (val as i32).to_be_bytes();
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    })?;
    g.set("llIntegerToBase64", f)?;

    // llBase64ToInteger(s) — base64-decode, first 4 bytes as big-endian i32
    let f = lua.create_function(|_, s: String| {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(&s).unwrap_or_default();
        let mut buf = [0u8; 4];
        for (i, &b) in bytes.iter().take(4).enumerate() {
            buf[i] = b;
        }
        Ok(i32::from_be_bytes(buf) as i64)
    })?;
    g.set("llBase64ToInteger", f)?;

    // llXorBase64StringsCorrect — XOR two base64 strings, cycling shorter,
    // with padding normalization matching OpenSim behaviour.
    let f = lua.create_function(|_, (str1, str2): (String, String)| {
        use base64::Engine;

        if str1.is_empty() {
            return Ok(String::new());
        }
        if str2.is_empty() {
            return Ok(str1);
        }

        // Normalize str2 padding.
        let mut s2 = str2.trim_end_matches('=').to_string();
        if s2.is_empty() {
            return Ok(str1);
        }
        let modulo = s2.len() % 4;
        match modulo {
            1 => { s2.pop(); }
            2 => s2.push_str("=="),
            3 => s2.push('='),
            _ => {}
        }
        if s2.is_empty() {
            return Ok(str1);
        }

        let data1 = match base64::engine::general_purpose::STANDARD.decode(&str1) {
            Ok(d) => d,
            Err(_) => return Ok(String::new()),
        };
        let data2 = match base64::engine::general_purpose::STANDARD.decode(&s2) {
            Ok(d) => d,
            Err(_) => return Ok(String::new()),
        };

        if data1.is_empty() {
            return Ok(String::new());
        }
        if data2.is_empty() {
            return Ok(str1);
        }

        let len2 = data2.len();
        let result: Vec<u8> = data1
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ data2[i % len2])
            .collect();

        Ok(base64::engine::general_purpose::STANDARD.encode(&result))
    })?;
    g.set("llXorBase64StringsCorrect", f)?;

    // llEscapeURL — percent-encode all bytes except unreserved (RFC 3986)
    let f = lua.create_function(|_, url: String| {
        let mut out = String::with_capacity(url.len() * 3);
        for &b in url.as_bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char);
                }
                _ => {
                    let hex = b"0123456789ABCDEF";
                    out.push('%');
                    out.push(hex[(b >> 4) as usize] as char);
                    out.push(hex[(b & 0x0F) as usize] as char);
                }
            }
        }
        Ok(out)
    })?;
    g.set("llEscapeURL", f)?;

    // llUnescapeURL — decode percent-encoded sequences
    let f = lua.create_function(|_, url: String| {
        let bytes = url.as_bytes();
        let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                let hi = bytes[i + 1];
                let lo = bytes[i + 2];
                if let (Some(h), Some(l)) = (hex_val(hi), hex_val(lo)) {
                    out.push((h << 4) | l);
                    i += 3;
                    continue;
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
        Ok(String::from_utf8(out).unwrap_or_default())
    })?;
    g.set("llUnescapeURL", f)?;

    // llDumpList2String — join list elements with a separator
    let f = lua.create_function(|_, (list, sep): (LuaTable, String)| {
        let len = list.raw_len();
        if len == 0 {
            return Ok(String::new());
        }
        let mut parts = Vec::with_capacity(len);
        for i in 1..=len {
            let v: LuaValue = list.raw_get(i as i64)?;
            parts.push(lua_value_to_string(&v));
        }
        Ok(parts.join(&sep))
    })?;
    g.set("llDumpList2String", f)?;

    // llParseString2List — split by separators/spacers, drop empty tokens
    let f = lua.create_function(|lua, (src, separators, spacers): (String, LuaTable, LuaTable)| {
        let seps = collect_string_entries(&separators);
        let spcs = collect_string_entries(&spacers);
        let tokens = parse_string_to_list(&src, &seps, &spcs, false);
        let result = lua.create_table()?;
        for (i, tok) in tokens.into_iter().enumerate() {
            result.raw_set(i as i64 + 1, tok)?;
        }
        Ok(result)
    })?;
    g.set("llParseString2List", f)?;

    // llParseStringKeepNulls — same as above but keeps empty tokens
    let f = lua.create_function(|lua, (src, separators, spacers): (String, LuaTable, LuaTable)| {
        let seps = collect_string_entries(&separators);
        let spcs = collect_string_entries(&spacers);
        let tokens = parse_string_to_list(&src, &seps, &spcs, true);
        let result = lua.create_table()?;
        for (i, tok) in tokens.into_iter().enumerate() {
            result.raw_set(i as i64 + 1, tok)?;
        }
        Ok(result)
    })?;
    g.set("llParseStringKeepNulls", f)?;

    // llHash — SDBM-variant hash over Unicode codepoints (matches OpenSim)
    let f = lua.create_function(|_, s: String| {
        if s.is_empty() {
            return Ok(0i64);
        }
        let mut hash: i32 = 0;
        for cp in s.chars().map(|c| c as u32) {
            hash = hash.wrapping_mul(65599).wrapping_add(cp as i32);
        }
        Ok(hash as i64)
    })?;
    g.set("llHash", f)?;

    // llOrd — Unicode codepoint at character index
    let f = lua.create_function(|_, (s, index): (String, i64)| {
        if s.is_empty() {
            return Ok(0i64);
        }
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len() as i64;
        let idx = if index < 0 { len + index } else { index };
        if idx < 0 || idx >= len {
            return Ok(0i64);
        }
        Ok(chars[idx as usize] as u32 as i64)
    })?;
    g.set("llOrd", f)?;

    // llChar — Unicode codepoint to single-character string
    let f = lua.create_function(|_, unicode: i64| {
        if unicode == 0 {
            return Ok(String::new());
        }
        match char::from_u32(unicode as u32) {
            Some(c) => Ok(c.to_string()),
            None => Ok("\u{FFFD}".to_string()),
        }
    })?;
    g.set("llChar", f)?;

    // llReplaceSubString — regex-based replace (count controls max replacements)
    let f = lua.create_function(|_, (src, pattern, replacement, count): (String, String, String, i64)| {
        if src.is_empty() || pattern.is_empty() {
            return Ok(src);
        }
        let rx = match regex::Regex::new(&pattern) {
            Ok(r) => r,
            Err(_) => return Ok(src),
        };
        let limit = if count < 0 {
            count.unsigned_abs() as usize
        } else if count == 0 {
            0 // 0 means replace all
        } else {
            count as usize
        };
        if limit == 0 {
            Ok(rx.replace_all(&src, replacement.as_str()).into_owned())
        } else {
            Ok(rx.replacen(&src, limit, replacement.as_str()).into_owned())
        }
    })?;
    g.set("llReplaceSubString", f)?;

    Ok(())
}

/// Decode a single hex digit (ASCII) to its value 0-15.
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
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
        let r: String = lua.load(r#"return llSHA1String("Hello")"#).eval().unwrap();
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

    #[test]
    fn sha256_string() {
        let lua = vm();
        let r: String = lua
            .load(r#"return llSHA256String("Hello", 0)"#)
            .eval()
            .unwrap();
        let expected = {
            use sha2::{Digest as _, Sha256};
            let mut h = Sha256::new();
            h.update("Hello:0");
            h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>()
        };
        assert_eq!(r, expected);
    }

    #[test]
    fn xor_base64_round_trip() {
        let lua = vm();
        // XOR "AAAA" with itself should give all zeros (base64 of 3 zero bytes = "AAAA")
        let r: String = lua
            .load(r#"return llXorBase64("AAAA", "AAAA")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "AAAA"); // 0 XOR 0 = 0

        // XOR with non-trivial values
        let r2: String = lua
            .load(r#"return llXorBase64("AQID", "AwIB")"#)
            .eval()
            .unwrap();
        // [1,2,3] XOR [3,2,1] = [2,0,2]
        assert_eq!(r2, "AgAC");
    }

    #[test]
    fn integer_to_base64_round_trip() {
        let lua = vm();
        let encoded: String = lua
            .load("return llIntegerToBase64(1)")
            .eval()
            .unwrap();
        // 1 as big-endian i32 = [0,0,0,1], base64 = "AAAAAQ=="
        assert_eq!(encoded, "AAAAAQ==");

        let decoded: i64 = lua
            .load(r#"return llBase64ToInteger("AAAAAQ==")"#)
            .eval()
            .unwrap();
        assert_eq!(decoded, 1);
    }

    #[test]
    fn base64_to_integer_empty() {
        let lua = vm();
        let r: i64 = lua
            .load(r#"return llBase64ToInteger("")"#)
            .eval()
            .unwrap();
        assert_eq!(r, 0);
    }
}
