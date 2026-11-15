// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL list function bindings for the Luau VM.

use mlua::prelude::*;
use serde_json::Value as JsonValue;

/// Convert an LSL 0-based index (possibly negative) to a Lua 1-based index.
/// Returns `None` if out of bounds.
fn lsl_to_lua_index(index: i64, len: i64) -> Option<i64> {
    let idx = if index < 0 { len + index } else { index };
    if idx >= 0 && idx < len {
        Some(idx + 1)
    } else {
        None
    }
}

/// Convert a Lua value to its string representation.
fn value_to_string(v: &LuaValue) -> String {
    match v {
        LuaValue::Integer(n) => n.to_string(),
        LuaValue::Number(n) => {
            // Format like LSL: 6 decimal places for floats
            format!("{n:.6}")
        }
        LuaValue::String(s) => s.to_string_lossy().to_string(),
        LuaValue::Boolean(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        LuaValue::Nil => String::new(),
        _ => String::new(),
    }
}

/// Extract all numeric values from a Lua table (sequential integer keys).
fn collect_numbers(list: &LuaTable) -> Vec<f64> {
    let len = list.raw_len();
    let mut nums = Vec::new();
    for i in 1..=len {
        if let Ok(v) = list.raw_get::<LuaValue>(i) {
            match v {
                LuaValue::Integer(n) => nums.push(n as f64),
                LuaValue::Number(n) => nums.push(n),
                _ => {}
            }
        }
    }
    nums
}

/// Collect all values from a Lua table into a Vec.
fn collect_values(list: &LuaTable) -> Vec<LuaValue> {
    let len = list.raw_len();
    let mut vals = Vec::with_capacity(len);
    for i in 1..=len {
        if let Ok(v) = list.raw_get::<LuaValue>(i) {
            vals.push(v);
        }
    }
    vals
}

/// Register all LSL list functions as Lua globals.
pub fn register_list(lua: &Lua) -> Result<(), crate::VmError> {
    let globals = lua.globals();

    // llGetListLength
    let f = lua.create_function(|_, list: LuaTable| Ok(list.raw_len() as i64))?;
    globals.set("llGetListLength", f)?;

    // llList2Integer
    let f = lua.create_function(|_, (list, index): (LuaTable, i64)| {
        let len = list.raw_len() as i64;
        match lsl_to_lua_index(index, len) {
            Some(lua_idx) => {
                let v: LuaValue = list.raw_get(lua_idx)?;
                match v {
                    LuaValue::Integer(n) => Ok(n),
                    LuaValue::Number(n) => Ok(n as i64),
                    LuaValue::String(s) => Ok(s.to_string_lossy().parse::<i64>().unwrap_or(0)),
                    _ => Ok(0),
                }
            }
            None => Ok(0),
        }
    })?;
    globals.set("llList2Integer", f)?;

    // llList2Float
    let f = lua.create_function(|_, (list, index): (LuaTable, i64)| {
        let len = list.raw_len() as i64;
        match lsl_to_lua_index(index, len) {
            Some(lua_idx) => {
                let v: LuaValue = list.raw_get(lua_idx)?;
                match v {
                    LuaValue::Integer(n) => Ok(n as f64),
                    LuaValue::Number(n) => Ok(n),
                    LuaValue::String(s) => Ok(s.to_string_lossy().parse::<f64>().unwrap_or(0.0)),
                    _ => Ok(0.0),
                }
            }
            None => Ok(0.0),
        }
    })?;
    globals.set("llList2Float", f)?;

    // llList2String
    let f = lua.create_function(|_, (list, index): (LuaTable, i64)| {
        let len = list.raw_len() as i64;
        match lsl_to_lua_index(index, len) {
            Some(lua_idx) => {
                let v: LuaValue = list.raw_get(lua_idx)?;
                Ok(value_to_string(&v))
            }
            None => Ok(String::new()),
        }
    })?;
    globals.set("llList2String", f)?;

    // llList2Key (same as llList2String — keys are strings in LSL)
    let f = lua.create_function(|_, (list, index): (LuaTable, i64)| {
        let len = list.raw_len() as i64;
        match lsl_to_lua_index(index, len) {
            Some(lua_idx) => {
                let v: LuaValue = list.raw_get(lua_idx)?;
                Ok(value_to_string(&v))
            }
            None => Ok(String::new()),
        }
    })?;
    globals.set("llList2Key", f)?;

    // llList2Vector — returns a table {x, y, z} or zero vector
    let f = lua.create_function(|lua, (list, index): (LuaTable, i64)| {
        let len = list.raw_len() as i64;
        let make_zero = || -> LuaResult<LuaTable> {
            let t = lua.create_table()?;
            t.set("x", 0.0)?;
            t.set("y", 0.0)?;
            t.set("z", 0.0)?;
            Ok(t)
        };
        match lsl_to_lua_index(index, len) {
            Some(lua_idx) => {
                let v: LuaValue = list.raw_get(lua_idx)?;
                match v {
                    LuaValue::Table(t) => {
                        // Check if it has x, y, z
                        let x: Result<f64, _> = t.get("x");
                        let y: Result<f64, _> = t.get("y");
                        let z: Result<f64, _> = t.get("z");
                        if x.is_ok() && y.is_ok() && z.is_ok() {
                            Ok(t)
                        } else {
                            make_zero()
                        }
                    }
                    _ => make_zero(),
                }
            }
            None => make_zero(),
        }
    })?;
    globals.set("llList2Vector", f)?;

    // llList2Rot — returns a table {x, y, z, w} or zero rotation
    let f = lua.create_function(|lua, (list, index): (LuaTable, i64)| {
        let len = list.raw_len() as i64;
        let make_zero = || -> LuaResult<LuaTable> {
            let t = lua.create_table()?;
            t.set("x", 0.0)?;
            t.set("y", 0.0)?;
            t.set("z", 0.0)?;
            t.set("w", 1.0)?;
            Ok(t)
        };
        match lsl_to_lua_index(index, len) {
            Some(lua_idx) => {
                let v: LuaValue = list.raw_get(lua_idx)?;
                match v {
                    LuaValue::Table(t) => {
                        let x: Result<f64, _> = t.get("x");
                        let y: Result<f64, _> = t.get("y");
                        let z: Result<f64, _> = t.get("z");
                        let w: Result<f64, _> = t.get("w");
                        if x.is_ok() && y.is_ok() && z.is_ok() && w.is_ok() {
                            Ok(t)
                        } else {
                            make_zero()
                        }
                    }
                    _ => make_zero(),
                }
            }
            None => make_zero(),
        }
    })?;
    globals.set("llList2Rot", f)?;

    // llListSort — sort list, stride=1 supported, ascending flag
    let f = lua.create_function(|lua, (list, stride, ascending): (LuaTable, i64, i64)| {
        let vals = collect_values(&list);
        if vals.is_empty() || stride < 1 {
            let result = lua.create_table()?;
            for (i, v) in vals.into_iter().enumerate() {
                result.raw_set(i as i64 + 1, v)?;
            }
            return Ok(result);
        }

        let stride = stride as usize;
        let asc = ascending == 1;

        if stride == 1 {
            let mut sortable: Vec<LuaValue> = vals;
            sortable.sort_by(|a, b| {
                // Numbers sort before strings; within same type, natural order
                let ord = match (a, b) {
                    (LuaValue::Integer(x), LuaValue::Integer(y)) => x.cmp(y),
                    (LuaValue::Number(x), LuaValue::Number(y)) => {
                        x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (LuaValue::Integer(x), LuaValue::Number(y)) => (*x as f64)
                        .partial_cmp(y)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    (LuaValue::Number(x), LuaValue::Integer(y)) => x
                        .partial_cmp(&(*y as f64))
                        .unwrap_or(std::cmp::Ordering::Equal),
                    (LuaValue::String(x), LuaValue::String(y)) => {
                        Ord::cmp(x.as_bytes().as_ref(), y.as_bytes().as_ref())
                    }
                    // Numbers before strings
                    (LuaValue::Integer(_) | LuaValue::Number(_), _) => std::cmp::Ordering::Less,
                    (_, LuaValue::Integer(_) | LuaValue::Number(_)) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                };
                if asc { ord } else { ord.reverse() }
            });
            let result = lua.create_table()?;
            for (i, v) in sortable.into_iter().enumerate() {
                result.raw_set(i as i64 + 1, v)?;
            }
            Ok(result)
        } else {
            // Multi-stride sort: sort by first element of each stride group
            let len = vals.len();
            let num_groups = len / stride;
            let remainder = len % stride;

            let mut groups: Vec<Vec<LuaValue>> = Vec::with_capacity(num_groups);
            for g in 0..num_groups {
                let start = g * stride;
                let end = start + stride;
                groups.push(vals[start..end].to_vec());
            }
            let leftover: Vec<LuaValue> = if remainder > 0 {
                vals[num_groups * stride..].to_vec()
            } else {
                Vec::new()
            };

            groups.sort_by(|a, b| {
                let ord = match (&a[0], &b[0]) {
                    (LuaValue::Integer(x), LuaValue::Integer(y)) => x.cmp(y),
                    (LuaValue::Number(x), LuaValue::Number(y)) => {
                        x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (LuaValue::Integer(x), LuaValue::Number(y)) => (*x as f64)
                        .partial_cmp(y)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    (LuaValue::Number(x), LuaValue::Integer(y)) => x
                        .partial_cmp(&(*y as f64))
                        .unwrap_or(std::cmp::Ordering::Equal),
                    (LuaValue::String(x), LuaValue::String(y)) => {
                        Ord::cmp(x.as_bytes().as_ref(), y.as_bytes().as_ref())
                    }
                    (LuaValue::Integer(_) | LuaValue::Number(_), _) => std::cmp::Ordering::Less,
                    (_, LuaValue::Integer(_) | LuaValue::Number(_)) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                };
                if asc { ord } else { ord.reverse() }
            });

            let result = lua.create_table()?;
            let mut idx = 1i64;
            for group in groups {
                for v in group {
                    result.raw_set(idx, v)?;
                    idx += 1;
                }
            }
            for v in leftover {
                result.raw_set(idx, v)?;
                idx += 1;
            }
            Ok(result)
        }
    })?;
    globals.set("llListSort", f)?;

    // llListFindList — find first occurrence of needle as contiguous subsequence
    let f = lua.create_function(|_, (haystack, needle): (LuaTable, LuaTable)| {
        let h = collect_values(&haystack);
        let n = collect_values(&needle);
        if n.is_empty() {
            return Ok(0i64);
        }
        if n.len() > h.len() {
            return Ok(-1i64);
        }
        'outer: for start in 0..=(h.len() - n.len()) {
            for (j, nv) in n.iter().enumerate() {
                let hv = &h[start + j];
                if !lua_values_equal(hv, nv) {
                    continue 'outer;
                }
            }
            return Ok(start as i64);
        }
        Ok(-1i64)
    })?;
    globals.set("llListFindList", f)?;

    // llDeleteSubList — remove elements [start..=end] (0-based LSL)
    let f = lua.create_function(|lua, (list, start, end_idx): (LuaTable, i64, i64)| {
        let vals = collect_values(&list);
        let len = vals.len() as i64;
        if len == 0 {
            return lua.create_table();
        }
        let s = if start < 0 {
            (len + start).max(0)
        } else {
            start.min(len - 1)
        } as usize;
        let e = if end_idx < 0 {
            (len + end_idx).max(0)
        } else {
            end_idx.min(len - 1)
        } as usize;

        let result = lua.create_table()?;
        let mut idx = 1i64;
        if s <= e {
            // Normal range: delete [s..=e]
            for (i, v) in vals.into_iter().enumerate() {
                if i < s || i > e {
                    result.raw_set(idx, v)?;
                    idx += 1;
                }
            }
        } else {
            // Inverted range: keep [e+1..s-1], delete [0..=e] and [s..end]
            for (i, v) in vals.into_iter().enumerate() {
                if i > e && i < s {
                    result.raw_set(idx, v)?;
                    idx += 1;
                }
            }
        }
        Ok(result)
    })?;
    globals.set("llDeleteSubList", f)?;

    // llGetListEntryType
    // 0=invalid, 1=integer, 2=float, 3=string, 5=vector(table with x,y,z),
    // 6=rotation(table with x,y,z,w)
    let f = lua.create_function(|_, (list, index): (LuaTable, i64)| {
        let len = list.raw_len() as i64;
        match lsl_to_lua_index(index, len) {
            Some(lua_idx) => {
                let v: LuaValue = list.raw_get(lua_idx)?;
                let type_code = match &v {
                    LuaValue::Integer(_) => 1,
                    LuaValue::Number(_) => 2,
                    LuaValue::String(_) => 3,
                    LuaValue::Table(t) => {
                        let has_w: bool = t
                            .get::<LuaValue>("w")
                            .map(|v| !matches!(v, LuaValue::Nil))
                            .unwrap_or(false);
                        let has_xyz = t
                            .get::<LuaValue>("x")
                            .map(|v| !matches!(v, LuaValue::Nil))
                            .unwrap_or(false)
                            && t.get::<LuaValue>("y")
                                .map(|v| !matches!(v, LuaValue::Nil))
                                .unwrap_or(false)
                            && t.get::<LuaValue>("z")
                                .map(|v| !matches!(v, LuaValue::Nil))
                                .unwrap_or(false);
                        if has_xyz && has_w {
                            6 // rotation
                        } else if has_xyz {
                            5 // vector
                        } else {
                            0
                        }
                    }
                    _ => 0,
                };
                Ok(type_code)
            }
            None => Ok(0),
        }
    })?;
    globals.set("llGetListEntryType", f)?;

    // llCSV2List — split by comma, trim whitespace
    let f = lua.create_function(|lua, csv: String| {
        let result = lua.create_table()?;
        for (i, part) in csv.split(',').enumerate() {
            result.raw_set(i as i64 + 1, part.trim())?;
        }
        Ok(result)
    })?;
    globals.set("llCSV2List", f)?;

    // llList2CSV — join elements with ", "
    let f = lua.create_function(|_, list: LuaTable| {
        let len = list.raw_len();
        let mut parts = Vec::with_capacity(len);
        for i in 1..=len {
            let v: LuaValue = list.raw_get(i as i64)?;
            parts.push(value_to_string(&v));
        }
        Ok(parts.join(", "))
    })?;
    globals.set("llList2CSV", f)?;

    // llListInsertList — insert src into dest at pos (0-based)
    let f = lua.create_function(|lua, (dest, src, pos): (LuaTable, LuaTable, i64)| {
        let d = collect_values(&dest);
        let s = collect_values(&src);
        let len = d.len() as i64;
        // Clamp pos to [0, len]
        let insert_at = if pos < 0 {
            (len + pos).max(0) as usize
        } else {
            (pos as usize).min(d.len())
        };

        let result = lua.create_table()?;
        let mut idx = 1i64;
        for (i, v) in d.into_iter().enumerate() {
            if i == insert_at {
                for sv in &s {
                    result.raw_set(idx, sv.clone())?;
                    idx += 1;
                }
            }
            result.raw_set(idx, v)?;
            idx += 1;
        }
        // If insert_at == original len, append at end
        if insert_at >= (len as usize) {
            for sv in s {
                result.raw_set(idx, sv)?;
                idx += 1;
            }
        }
        Ok(result)
    })?;
    globals.set("llListInsertList", f)?;

    // llListStatistics — compute statistics on numeric elements
    let f = lua.create_function(|_, (operation, list): (i64, LuaTable)| {
        let nums = collect_numbers(&list);
        if nums.is_empty() {
            return Ok(0.0f64);
        }
        let result = match operation {
            0 => {
                // LIST_STAT_RANGE
                let min = nums.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                max - min
            }
            1 => {
                // LIST_STAT_MIN
                nums.iter().cloned().fold(f64::INFINITY, f64::min)
            }
            2 => {
                // LIST_STAT_MAX
                nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            }
            3 => {
                // LIST_STAT_MEAN
                let sum: f64 = nums.iter().sum();
                sum / nums.len() as f64
            }
            4 => {
                // LIST_STAT_MEDIAN
                let mut sorted = nums.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mid = sorted.len() / 2;
                if sorted.len().is_multiple_of(2) {
                    (sorted[mid - 1] + sorted[mid]) / 2.0
                } else {
                    sorted[mid]
                }
            }
            5 => {
                // LIST_STAT_STD_DEV
                let n = nums.len() as f64;
                let mean = nums.iter().sum::<f64>() / n;
                let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
                variance.sqrt()
            }
            6 => {
                // LIST_STAT_SUM
                nums.iter().sum()
            }
            7 => {
                // LIST_STAT_SUM_SQUARES
                nums.iter().map(|x| x * x).sum()
            }
            8 => {
                // LIST_STAT_NUM_COUNT
                nums.len() as f64
            }
            9 => {
                // LIST_STAT_GEOMETRIC_MEAN
                let product: f64 = nums.iter().product();
                product.powf(1.0 / nums.len() as f64)
            }
            _ => 0.0,
        };
        Ok(result)
    })?;
    globals.set("llListStatistics", f)?;

    // -- Additional list functions -------------------------------------------

    // llList2List — extract sub-list [start..=end], 0-based, inclusive
    let f = lua.create_function(|lua, (list, start, end): (LuaTable, i64, i64)| {
        let vals = collect_values(&list);
        let len = vals.len() as i64;
        if len == 0 {
            return lua.create_table();
        }

        let mut s = if start < 0 { (len + start).max(0) } else { start };
        let mut e = if end < 0 { (len + end).max(0) } else { end.min(len - 1) };

        let result = lua.create_table()?;
        let mut idx = 1i64;

        if s > e {
            // Inverted range: return [0..=e] + [s..len-1]
            e = e.min(len - 1);
            s = s.max(0);
            for i in 0..=e {
                result.raw_set(idx, vals[i as usize].clone())?;
                idx += 1;
            }
            for i in s..len {
                result.raw_set(idx, vals[i as usize].clone())?;
                idx += 1;
            }
        } else {
            // Normal range
            if s >= len {
                return Ok(result);
            }
            e = e.min(len - 1);
            for i in s..=e {
                result.raw_set(idx, vals[i as usize].clone())?;
                idx += 1;
            }
        }
        Ok(result)
    })?;
    globals.set("llList2List", f)?;

    // llList2ListStrided — extract every stride-th element
    let f = lua.create_function(|lua, (list, start, end, stride): (LuaTable, i64, i64, i64)| {
        let vals = collect_values(&list);
        let len = vals.len() as i64;
        if len == 0 {
            return lua.create_table();
        }

        let mut s = if start < 0 { (len + start).max(0) } else { start };
        let mut e = if end < 0 { (len + end).max(0) } else { end };

        if s > e {
            s = 0;
            e = len - 1;
        } else {
            if s >= len {
                return lua.create_table();
            }
            if e >= len {
                e = len - 1;
            }
        }

        let stride = if stride < 1 { 1i64 } else { stride };

        // Align start to stride boundary
        if stride > 1 && s > 0 {
            let sst = (s / stride) * stride;
            if sst != s {
                s = sst + stride;
            }
            if s > e {
                return lua.create_table();
            }
        }

        let result = lua.create_table()?;
        let mut idx = 1i64;
        let mut i = s;
        while i <= e {
            result.raw_set(idx, vals[i as usize].clone())?;
            idx += 1;
            i += stride;
        }
        Ok(result)
    })?;
    globals.set("llList2ListStrided", f)?;

    // llList2ListSlice — like strided but picks offset within each stride group
    let f = lua.create_function(
        |lua, (list, start, end, stride, stride_index): (LuaTable, i64, i64, i64, i64)| {
            let vals = collect_values(&list);
            let len = vals.len() as i64;
            if len == 0 {
                return lua.create_table();
            }

            let mut s = if start < 0 { (len + start).max(0) } else { start };
            let mut e = if end < 0 { (len + end).max(0) } else { end };

            if s > e {
                s = 0;
                e = len - 1;
            } else {
                if s >= len {
                    return lua.create_table();
                }
                if e >= len {
                    e = len - 1;
                }
            }

            let stride = if stride < 1 { 1i64 } else { stride };

            let mut si = stride_index;
            if si < 0 {
                si += stride;
                if si < 0 {
                    return lua.create_table();
                }
            } else if si >= stride {
                return lua.create_table();
            }

            // Align start to stride boundary
            if stride > 1 && s > 0 {
                let sst = (s / stride) * stride;
                if sst != s {
                    s = sst + stride;
                }
                if s > e {
                    return lua.create_table();
                }
            }

            s += si;

            let result = lua.create_table()?;
            let mut idx = 1i64;
            let mut i = s;
            while i <= e {
                result.raw_set(idx, vals[i as usize].clone())?;
                idx += 1;
                i += stride;
            }
            Ok(result)
        },
    )?;
    globals.set("llList2ListSlice", f)?;

    // llListFindStrided — find test as subsequence with stride stepping
    let f = lua.create_function(
        |_, (src, test, lstart, lend, lstride): (LuaTable, LuaTable, i64, i64, i64)| {
            let h = collect_values(&src);
            let n = collect_values(&test);
            let len = h.len() as i64;

            if len == 0 {
                return Ok(-1i64);
            }
            if n.is_empty() {
                return Ok(0i64);
            }
            if n.len() as i64 > len {
                return Ok(-1i64);
            }

            let start = if lstart < 0 {
                let v = len + lstart;
                if v < 0 {
                    return Ok(-1i64);
                }
                v
            } else {
                if lstart >= len {
                    return Ok(-1i64);
                }
                lstart
            };

            let end = if lend < 0 {
                let v = len + lend;
                if v < 0 {
                    return Ok(-1i64);
                }
                v - n.len() as i64 + 1
            } else {
                if lend >= len {
                    len - n.len() as i64
                } else {
                    lend
                }
            };

            let stride = if lstride < 1 { 1i64 } else { lstride };

            let _ = end; // suppress warning
            let mut i = start;
            while i <= end {
                let mut matched = true;
                for (j, nv) in n.iter().enumerate() {
                    let hi = i as usize + j;
                    if hi >= h.len() || !lua_values_equal(&h[hi], nv) {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    return Ok(i);
                }
                i += stride;
            }
            Ok(-1i64)
        },
    )?;
    globals.set("llListFindStrided", f)?;

    // llListReplaceList — replace range [start..=end] with src
    let f = lua.create_function(
        |lua, (dest, src, start, end): (LuaTable, LuaTable, i64, i64)| {
            let d = collect_values(&dest);
            let s = collect_values(&src);
            let len = d.len() as i64;

            let st = if start < 0 { len + start } else { start };
            let en = if end < 0 { len + end } else { end };

            let result = lua.create_table()?;
            let mut idx = 1i64;

            if st <= en {
                // Normal case: dest[0..st] + src + dest[en+1..]
                if st > 0 {
                    for i in 0..st.min(len) {
                        result.raw_set(idx, d[i as usize].clone())?;
                        idx += 1;
                    }
                    for v in &s {
                        result.raw_set(idx, v.clone())?;
                        idx += 1;
                    }
                    if en + 1 < len {
                        for i in (en + 1)..len {
                            result.raw_set(idx, d[i as usize].clone())?;
                            idx += 1;
                        }
                    }
                } else if st == 0 {
                    for v in &s {
                        result.raw_set(idx, v.clone())?;
                        idx += 1;
                    }
                    if en + 1 < len {
                        for i in (en + 1)..len {
                            result.raw_set(idx, d[i as usize].clone())?;
                            idx += 1;
                        }
                    }
                } else {
                    // start < 0 after resolution
                    if en + 1 < len {
                        for i in (en + 1)..len {
                            result.raw_set(idx, d[i as usize].clone())?;
                            idx += 1;
                        }
                    }
                }
            } else {
                // Inverted: dest[en+1..st-1] + src
                let from = (en + 1).max(0);
                let to = st.min(len);
                if from < to {
                    for i in from..to {
                        result.raw_set(idx, d[i as usize].clone())?;
                        idx += 1;
                    }
                }
                for v in &s {
                    result.raw_set(idx, v.clone())?;
                    idx += 1;
                }
            }
            Ok(result)
        },
    )?;
    globals.set("llListReplaceList", f)?;

    // llListSortStrided — sort by specific element within each stride group
    let f = lua.create_function(
        |lua, (list, stride, stride_index, ascending): (LuaTable, i64, i64, i64)| {
            let vals = collect_values(&list);
            if vals.is_empty() {
                return lua.create_table();
            }

            let stride = if stride < 1 { 1usize } else { stride as usize };
            let si = if stride_index < 0 { 0usize } else { (stride_index as usize).min(stride - 1) };
            let asc = ascending == 1;

            let len = vals.len();
            let num_groups = len / stride;
            let remainder = len % stride;

            let mut groups: Vec<Vec<LuaValue>> = Vec::with_capacity(num_groups);
            for g in 0..num_groups {
                let start = g * stride;
                let end = start + stride;
                groups.push(vals[start..end].to_vec());
            }
            let leftover: Vec<LuaValue> = if remainder > 0 {
                vals[num_groups * stride..].to_vec()
            } else {
                Vec::new()
            };

            groups.sort_by(|a, b| {
                let ak = if si < a.len() { &a[si] } else { return std::cmp::Ordering::Equal };
                let bk = if si < b.len() { &b[si] } else { return std::cmp::Ordering::Equal };
                let ord = compare_lua_values(ak, bk);
                if asc { ord } else { ord.reverse() }
            });

            let result = lua.create_table()?;
            let mut idx = 1i64;
            for group in groups {
                for v in group {
                    result.raw_set(idx, v)?;
                    idx += 1;
                }
            }
            for v in leftover {
                result.raw_set(idx, v)?;
                idx += 1;
            }
            Ok(result)
        },
    )?;
    globals.set("llListSortStrided", f)?;

    // llListRandomize — randomize list in stride-sized groups
    let f = lua.create_function(|lua, (list, stride): (LuaTable, i64)| {
        let vals = collect_values(&list);
        let len = vals.len();
        if len == 0 {
            return lua.create_table();
        }

        let stride = if stride <= 0 { 1usize } else { stride as usize };

        // Per OpenSim: stride must be a factor of length, and length != stride
        if len == stride || len % stride != 0 {
            let result = lua.create_table()?;
            for (i, v) in vals.into_iter().enumerate() {
                result.raw_set(i as i64 + 1, v)?;
            }
            return Ok(result);
        }

        let num_chunks = len / stride;
        let mut chunks: Vec<usize> = (0..num_chunks).collect();

        // Knuth shuffle using time-based seed
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut rng_state = seed;
        for i in (1..num_chunks).rev() {
            // Simple LCG
            rng_state = rng_state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = (rng_state >> 33) as usize % (i + 1);
            chunks.swap(i, j);
        }

        let result = lua.create_table()?;
        let mut idx = 1i64;
        for &chunk_idx in &chunks {
            let start = chunk_idx * stride;
            for j in 0..stride {
                result.raw_set(idx, vals[start + j].clone())?;
                idx += 1;
            }
        }
        Ok(result)
    })?;
    globals.set("llListRandomize", f)?;

    // -- JSON constants ------------------------------------------------------

    let json_invalid = "\u{FDD0}";
    let json_object = "\u{FDD1}";
    let json_array = "\u{FDD2}";
    let json_number = "\u{FDD3}";
    let json_string = "\u{FDD4}";
    let json_null = "\u{FDD5}";
    let json_true = "\u{FDD6}";
    let json_false = "\u{FDD7}";
    let json_delete = "\u{FDD8}";

    globals.set("JSON_INVALID", json_invalid)?;
    globals.set("JSON_OBJECT", json_object)?;
    globals.set("JSON_ARRAY", json_array)?;
    globals.set("JSON_NUMBER", json_number)?;
    globals.set("JSON_STRING", json_string)?;
    globals.set("JSON_NULL", json_null)?;
    globals.set("JSON_TRUE", json_true)?;
    globals.set("JSON_FALSE", json_false)?;
    globals.set("JSON_DELETE", json_delete)?;
    globals.set("JSON_APPEND", "-1")?;

    // -- JSON functions ------------------------------------------------------

    // llJson2List — parse JSON string into flat list
    let f = lua.create_function(|lua, json: String| {
        let json_invalid: &str = "\u{FDD0}";
        let json_null: &str = "\u{FDD5}";
        let json_true: &str = "\u{FDD6}";
        let json_false: &str = "\u{FDD7}";

        if json.is_empty() {
            return lua.create_table();
        }
        if json == "[]" || json == "{}" {
            return lua.create_table();
        }

        let trimmed = json.trim();
        let first = match trimmed.bytes().next() {
            Some(b) => b,
            None => return lua.create_table(),
        };

        if first != b'[' && first != b'{' {
            // Single element
            let result = lua.create_table()?;
            result.raw_set(1, json)?;
            return Ok(result);
        }

        let parsed: JsonValue = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                let result = lua.create_table()?;
                result.raw_set(1, json_invalid)?;
                return Ok(result);
            }
        };

        let result = lua.create_table()?;
        let mut idx = 1i64;

        match &parsed {
            JsonValue::Array(arr) => {
                for elem in arr {
                    let v = json_value_to_lua_top(lua, elem, json_null, json_true, json_false)?;
                    result.raw_set(idx, v)?;
                    idx += 1;
                }
            }
            JsonValue::Object(map) => {
                for (key, val) in map {
                    result.raw_set(idx, key.as_str())?;
                    idx += 1;
                    let v = json_value_to_lua_top(lua, val, json_null, json_true, json_false)?;
                    result.raw_set(idx, v)?;
                    idx += 1;
                }
            }
            _ => {
                let v = json_value_to_lua_top(lua, &parsed, json_null, json_true, json_false)?;
                result.raw_set(1, v)?;
            }
        }
        Ok(result)
    })?;
    globals.set("llJson2List", f)?;

    // llList2Json — build JSON from list
    let f = lua.create_function(|_, (type_str, values): (String, LuaTable)| {
        let json_invalid: &str = "\u{FDD0}";
        let json_array: &str = "\u{FDD2}";
        let json_object: &str = "\u{FDD1}";
        let json_null: &str = "\u{FDD5}";
        let json_true: &str = "\u{FDD6}";
        let json_false: &str = "\u{FDD7}";

        let vals = collect_values(&values);

        if type_str == json_array {
            let mut parts: Vec<String> = Vec::with_capacity(vals.len());
            for v in &vals {
                parts.push(lua_value_to_json_str(v, json_null, json_true, json_false));
            }
            Ok(format!("[{}]", parts.join(",")))
        } else if type_str == json_object {
            if vals.len() % 2 != 0 {
                return Ok(json_invalid.to_string());
            }
            let mut parts: Vec<String> = Vec::new();
            for chunk in vals.chunks(2) {
                let key_str = match &chunk[0] {
                    LuaValue::String(s) => s.to_string_lossy().to_string(),
                    LuaValue::Integer(n) => n.to_string(),
                    LuaValue::Number(n) => format!("{n}"),
                    _ => return Ok(json_invalid.to_string()),
                };
                let key_json = serde_json::to_string(&key_str)
                    .unwrap_or_else(|_| format!("\"{}\"", key_str));
                let val_json =
                    lua_value_to_json_str(&chunk[1], json_null, json_true, json_false);
                parts.push(format!("{}:{}", key_json, val_json));
            }
            Ok(format!("{{{}}}", parts.join(",")))
        } else {
            Ok(json_invalid.to_string())
        }
    })?;
    globals.set("llList2Json", f)?;

    // llJsonGetValue — navigate JSON by path and return value as string
    let f = lua.create_function(|_, (json, specifiers): (String, LuaTable)| {
        let json_invalid: &str = "\u{FDD0}";
        let json_null: &str = "\u{FDD5}";
        let json_true: &str = "\u{FDD6}";
        let json_false: &str = "\u{FDD7}";

        if json.trim().is_empty() {
            return Ok(json_invalid.to_string());
        }

        let specs = collect_values(&specifiers);

        let trimmed = json.trim();
        let first = trimmed.bytes().next().unwrap_or(0);

        let (work_json, work_specs) = if first != b'[' && first != b'{' {
            if !specs.is_empty() {
                return Ok(json_invalid.to_string());
            }
            (format!("[{}]", trimmed), vec![LuaValue::Integer(0)])
        } else {
            if specs.is_empty() && (trimmed == "{}" || trimmed == "[]") {
                return Ok(json_invalid.to_string());
            }
            (trimmed.to_string(), specs)
        };

        let parsed: JsonValue = match serde_json::from_str(&work_json) {
            Ok(v) => v,
            Err(_) => return Ok(json_invalid.to_string()),
        };

        if work_specs.is_empty() {
            return Ok(json_element_to_string(&parsed, json_null, json_true, json_false));
        }

        match json_navigate(&parsed, &work_specs) {
            Some(val) => Ok(json_element_to_string(val, json_null, json_true, json_false)),
            None => Ok(json_invalid.to_string()),
        }
    })?;
    globals.set("llJsonGetValue", f)?;

    // llJsonSetValue — set a value in JSON at path
    let f = lua.create_function(|_, (json, specifiers, value): (String, LuaTable, String)| {
        let json_invalid: &str = "\u{FDD0}";
        let json_null: &str = "\u{FDD5}";
        let json_true: &str = "\u{FDD6}";
        let json_false: &str = "\u{FDD7}";
        let json_delete: &str = "\u{FDD8}";
        let json_append: &str = "-1";

        let specs = collect_values(&specifiers);

        let trimmed = json.trim();
        let mut parsed: JsonValue = if trimmed.is_empty() {
            JsonValue::Array(vec![])
        } else {
            match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => return Ok(json_invalid.to_string()),
            }
        };

        let no_specs = specs.is_empty();
        let work_specs = if no_specs {
            vec![LuaValue::Integer(0)]
        } else {
            specs
        };

        match json_set_value(
            &mut parsed,
            &work_specs,
            0,
            &value,
            json_null,
            json_true,
            json_false,
            json_delete,
            json_append,
        ) {
            Ok(replacement) => {
                let target = replacement.unwrap_or(parsed);
                match serde_json::to_string(&target) {
                    Ok(mut s) => {
                        if no_specs {
                            // Strip leading [ and trailing ]
                            if s.starts_with('[') && s.ends_with(']') {
                                s = s[1..s.len() - 1].to_string();
                            }
                        }
                        Ok(s)
                    }
                    Err(_) => Ok(json_invalid.to_string()),
                }
            }
            Err(_) => Ok(json_invalid.to_string()),
        }
    })?;
    globals.set("llJsonSetValue", f)?;

    // llJsonValueType — return the type constant for the value at path
    let f = lua.create_function(|_, (json, specifiers): (String, LuaTable)| {
        let json_invalid: &str = "\u{FDD0}";
        let json_object: &str = "\u{FDD1}";
        let json_array: &str = "\u{FDD2}";
        let json_number: &str = "\u{FDD3}";
        let json_string: &str = "\u{FDD4}";
        let json_null: &str = "\u{FDD5}";
        let json_true: &str = "\u{FDD6}";
        let json_false: &str = "\u{FDD7}";

        if json.trim().is_empty() {
            return Ok(json_invalid.to_string());
        }

        let specs = collect_values(&specifiers);

        let trimmed = json.trim();
        let first = trimmed.bytes().next().unwrap_or(0);

        let (work_json, work_specs) = if first != b'[' && first != b'{' {
            if !specs.is_empty() {
                return Ok(json_invalid.to_string());
            }
            (format!("[{}]", trimmed), vec![LuaValue::Integer(0)])
        } else {
            if !specs.is_empty() && (trimmed == "{}" || trimmed == "[]") {
                return Ok(json_invalid.to_string());
            }
            (trimmed.to_string(), specs)
        };

        let parsed: JsonValue = match serde_json::from_str(&work_json) {
            Ok(v) => v,
            Err(_) => return Ok(json_invalid.to_string()),
        };

        let elem = if work_specs.is_empty() {
            &parsed
        } else {
            match json_navigate(&parsed, &work_specs) {
                Some(v) => v,
                None => return Ok(json_invalid.to_string()),
            }
        };

        let type_str = match elem {
            JsonValue::Array(_) => json_array,
            JsonValue::Object(_) => json_object,
            JsonValue::Number(_) => json_number,
            JsonValue::String(s) => {
                if s == json_null {
                    json_null
                } else if s == json_true {
                    json_true
                } else if s == json_false {
                    json_false
                } else {
                    json_string
                }
            }
            JsonValue::Bool(b) => {
                if *b { json_true } else { json_false }
            }
            JsonValue::Null => json_null,
        };
        Ok(type_str.to_string())
    })?;
    globals.set("llJsonValueType", f)?;

    Ok(())
}

/// Compare two Lua values for equality (used by llListFindList).
fn lua_values_equal(a: &LuaValue, b: &LuaValue) -> bool {
    match (a, b) {
        (LuaValue::Integer(x), LuaValue::Integer(y)) => x == y,
        (LuaValue::Number(x), LuaValue::Number(y)) => (x - y).abs() < f64::EPSILON,
        (LuaValue::Integer(x), LuaValue::Number(y)) => ((*x as f64) - y).abs() < f64::EPSILON,
        (LuaValue::Number(x), LuaValue::Integer(y)) => (x - (*y as f64)).abs() < f64::EPSILON,
        (LuaValue::String(x), LuaValue::String(y)) => x.as_bytes() == y.as_bytes(),
        (LuaValue::Boolean(x), LuaValue::Boolean(y)) => x == y,
        (LuaValue::Nil, LuaValue::Nil) => true,
        _ => false,
    }
}

/// Compare two Lua values for sort ordering (used by llListSortStrided).
fn compare_lua_values(a: &LuaValue, b: &LuaValue) -> std::cmp::Ordering {
    match (a, b) {
        (LuaValue::Integer(x), LuaValue::Integer(y)) => x.cmp(y),
        (LuaValue::Number(x), LuaValue::Number(y)) => {
            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (LuaValue::Integer(x), LuaValue::Number(y)) => (*x as f64)
            .partial_cmp(y)
            .unwrap_or(std::cmp::Ordering::Equal),
        (LuaValue::Number(x), LuaValue::Integer(y)) => x
            .partial_cmp(&(*y as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        (LuaValue::String(x), LuaValue::String(y)) => {
            Ord::cmp(x.as_bytes().as_ref(), y.as_bytes().as_ref())
        }
        (LuaValue::Integer(_) | LuaValue::Number(_), _) => std::cmp::Ordering::Less,
        (_, LuaValue::Integer(_) | LuaValue::Number(_)) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    }
}

// ---------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------

/// Convert a serde_json Value to a Lua value for llJson2List top-level extraction.
fn json_value_to_lua_top<'a>(
    lua: &'a Lua,
    val: &JsonValue,
    json_null: &str,
    json_true: &str,
    json_false: &str,
) -> LuaResult<LuaValue> {
    match val {
        JsonValue::Null => Ok(LuaValue::String(lua.create_string(json_null)?)),
        JsonValue::Bool(b) => {
            let s = if *b { json_true } else { json_false };
            Ok(LuaValue::String(lua.create_string(s)?))
        }
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else {
                Ok(LuaValue::Number(n.as_f64().unwrap_or(0.0)))
            }
        }
        JsonValue::String(s) => Ok(LuaValue::String(lua.create_string(s.as_str())?)),
        JsonValue::Array(_) | JsonValue::Object(_) => {
            // Nested structures become their JSON string representation
            let s = serde_json::to_string(val).unwrap_or_default();
            Ok(LuaValue::String(lua.create_string(s.as_str())?))
        }
    }
}

/// Convert a Lua value to a JSON string fragment for llList2Json.
fn lua_value_to_json_str(v: &LuaValue, json_null: &str, json_true: &str, json_false: &str) -> String {
    match v {
        LuaValue::Integer(n) => n.to_string(),
        LuaValue::Number(n) => {
            if n.is_infinite() {
                "\"Inf\"".to_string()
            } else if n.is_nan() {
                "\"NaN\"".to_string()
            } else {
                n.to_string()
            }
        }
        LuaValue::String(s) => {
            let st = s.to_string_lossy();
            let trimmed = st.trim();
            if trimmed.is_empty() {
                return "\"\"".to_string();
            }
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                return trimmed.to_string();
            }
            if trimmed == json_true || trimmed == "true" {
                return "true".to_string();
            }
            if trimmed == json_false || trimmed == "false" {
                return "false".to_string();
            }
            if trimmed == json_null || trimmed == "null" {
                return "null".to_string();
            }
            // Try to parse as number
            if let Ok(_) = trimmed.parse::<f64>() {
                // Check if it looks like an integer (no dot)
                if !trimmed.contains('.') {
                    if let Ok(i) = trimmed.parse::<i64>() {
                        return i.to_string();
                    }
                }
                return trimmed.to_string();
            }
            serde_json::to_string(trimmed).unwrap_or_else(|_| format!("\"{}\"", trimmed))
        }
        LuaValue::Boolean(b) => {
            if *b { "true" } else { "false" }.to_string()
        }
        LuaValue::Nil => "null".to_string(),
        _ => "null".to_string(),
    }
}

/// Navigate into a JSON value following a path of specifiers.
fn json_navigate<'a>(root: &'a JsonValue, specs: &[LuaValue]) -> Option<&'a JsonValue> {
    let mut current = root;
    for spec in specs {
        match spec {
            LuaValue::Integer(idx) => {
                let arr = current.as_array()?;
                let i = *idx as usize;
                if i >= arr.len() {
                    return None;
                }
                current = &arr[i];
            }
            LuaValue::Number(n) => {
                let arr = current.as_array()?;
                let i = *n as usize;
                if i >= arr.len() {
                    return None;
                }
                current = &arr[i];
            }
            LuaValue::String(s) => {
                let key = s.to_string_lossy();
                let obj = current.as_object()?;
                current = obj.get::<str>(key.as_ref())?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Convert a JSON element to its LSL string representation.
fn json_element_to_string(
    val: &JsonValue,
    json_null: &str,
    json_true: &str,
    json_false: &str,
) -> String {
    match val {
        JsonValue::Null => json_null.to_string(),
        JsonValue::Bool(b) => {
            if *b { json_true } else { json_false }.to_string()
        }
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else {
                let d = n.as_f64().unwrap_or(0.0);
                // Format with up to 6 decimal places, at least 1
                let s = format!("{:.6}", d);
                // Trim trailing zeros but keep at least one decimal
                let s = s.trim_end_matches('0');
                let s = if s.ends_with('.') {
                    format!("{}0", s)
                } else {
                    s.to_string()
                };
                s
            }
        }
        JsonValue::String(s) => s.clone(),
        JsonValue::Array(_) | JsonValue::Object(_) => {
            serde_json::to_string(val).unwrap_or_default()
        }
    }
}

/// Recursively set a value in a JSON tree.
fn json_set_value(
    current: &mut JsonValue,
    specs: &[LuaValue],
    level: usize,
    value: &str,
    json_null: &str,
    json_true: &str,
    json_false: &str,
    json_delete: &str,
    json_append: &str,
) -> Result<Option<JsonValue>, ()> {
    if level >= specs.len() {
        return Err(());
    }

    let spec = &specs[level];
    let is_last = level == specs.len() - 1;

    // Convert the value string to a JsonValue
    let make_json_val = |val: &str| -> JsonValue {
        if val == json_null || val == "null" {
            JsonValue::Null
        } else if val == json_true || val == "true" {
            JsonValue::Bool(true)
        } else if val == json_false || val == "false" {
            JsonValue::Bool(false)
        } else if let Ok(i) = val.parse::<i64>() {
            if !val.contains('.') {
                JsonValue::Number(serde_json::Number::from(i))
            } else {
                match serde_json::Number::from_f64(val.parse::<f64>().unwrap_or(0.0)) {
                    Some(n) => JsonValue::Number(n),
                    None => JsonValue::String(val.to_string()),
                }
            }
        } else if let Ok(f) = val.parse::<f64>() {
            match serde_json::Number::from_f64(f) {
                Some(n) => JsonValue::Number(n),
                None => JsonValue::String(val.to_string()),
            }
        } else {
            JsonValue::String(val.to_string())
        }
    };

    // Build the rest of the spec chain as nested structure
    let build_rest = |specs: &[LuaValue], from: usize, val: &str| -> Result<JsonValue, ()> {
        if from >= specs.len() {
            return Ok(make_json_val(val));
        }
        let next_spec = &specs[from];
        match next_spec {
            LuaValue::Integer(_) | LuaValue::Number(_) => {
                let arr = vec![build_rest_inner(specs, from + 1, val, &make_json_val)?];
                Ok(JsonValue::Array(arr))
            }
            LuaValue::String(s) => {
                let key = s.to_string_lossy().to_string();
                if key == json_append {
                    let arr = vec![build_rest_inner(specs, from + 1, val, &make_json_val)?];
                    Ok(JsonValue::Array(arr))
                } else {
                    let mut map = serde_json::Map::new();
                    map.insert(key, build_rest_inner(specs, from + 1, val, &make_json_val)?);
                    Ok(JsonValue::Object(map))
                }
            }
            _ => Err(()),
        }
    };

    match current {
        JsonValue::Array(arr) => {
            match spec {
                LuaValue::Integer(idx) => {
                    let i = *idx as usize;
                    if is_last {
                        if value == json_delete {
                            if i < arr.len() {
                                arr.remove(i);
                                return Ok(None);
                            }
                            return Err(());
                        }
                        let new_val = make_json_val(value);
                        if i == arr.len() {
                            arr.push(new_val);
                        } else if i < arr.len() {
                            arr[i] = new_val;
                        } else {
                            return Err(());
                        }
                        Ok(None)
                    } else {
                        if i == arr.len() {
                            let new_elem = build_rest(specs, level + 1, value)?;
                            arr.push(new_elem);
                            Ok(None)
                        } else if i < arr.len() {
                            let replacement = json_set_value(
                                &mut arr[i],
                                specs,
                                level + 1,
                                value,
                                json_null,
                                json_true,
                                json_false,
                                json_delete,
                                json_append,
                            )?;
                            if let Some(r) = replacement {
                                arr[i] = r;
                            }
                            Ok(None)
                        } else {
                            Err(())
                        }
                    }
                }
                LuaValue::Number(n) => {
                    let i = *n as usize;
                    // Same as integer case
                    let spec_as_int = LuaValue::Integer(i as i64);
                    let mut new_specs = specs.to_vec();
                    new_specs[level] = spec_as_int;
                    json_set_value(
                        current,
                        &new_specs,
                        level,
                        value,
                        json_null,
                        json_true,
                        json_false,
                        json_delete,
                        json_append,
                    )
                }
                LuaValue::String(s) => {
                    let key = s.to_string_lossy().to_string();
                    if key == json_append {
                        let new_elem = build_rest(specs, level + 1, value)?;
                        arr.push(new_elem);
                        Ok(None)
                    } else if arr.len() < 2 {
                        // Reinterpret as object
                        let mut map = serde_json::Map::new();
                        let new_elem = build_rest(specs, level + 1, value)?;
                        map.insert(key, new_elem);
                        Ok(Some(JsonValue::Object(map)))
                    } else {
                        Err(())
                    }
                }
                _ => Err(()),
            }
        }
        JsonValue::Object(map) => {
            match spec {
                LuaValue::String(s) => {
                    let key = s.to_string_lossy().to_string();
                    if is_last {
                        if value == json_delete {
                            map.remove(&key);
                            return Ok(None);
                        }
                        let new_val = make_json_val(value);
                        map.insert(key, new_val);
                        Ok(None)
                    } else {
                        if let Some(existing) = map.get_mut(&key) {
                            let replacement = json_set_value(
                                existing,
                                specs,
                                level + 1,
                                value,
                                json_null,
                                json_true,
                                json_false,
                                json_delete,
                                json_append,
                            )?;
                            if let Some(r) = replacement {
                                map.insert(key, r);
                            }
                            Ok(None)
                        } else {
                            let new_elem = build_rest(specs, level + 1, value)?;
                            map.insert(key, new_elem);
                            Ok(None)
                        }
                    }
                }
                LuaValue::Integer(i) if *i == 0 => {
                    // Replace object with array
                    let mut arr = Vec::new();
                    let new_elem = build_rest(specs, level + 1, value)?;
                    arr.push(new_elem);
                    Ok(Some(JsonValue::Array(arr)))
                }
                _ => Err(()),
            }
        }
        _ => {
            // Replace scalar with new structure
            let new_data = build_rest(specs, level, value)?;
            Ok(Some(new_data))
        }
    }
}

/// Helper for building nested JSON from remaining spec chain.
fn build_rest_inner(
    specs: &[LuaValue],
    from: usize,
    val: &str,
    make_val: &dyn Fn(&str) -> JsonValue,
) -> Result<JsonValue, ()> {
    if from >= specs.len() {
        return Ok(make_val(val));
    }
    let next_spec = &specs[from];
    match next_spec {
        LuaValue::Integer(_) | LuaValue::Number(_) => {
            let inner = build_rest_inner(specs, from + 1, val, make_val)?;
            Ok(JsonValue::Array(vec![inner]))
        }
        LuaValue::String(s) => {
            let key = s.to_string_lossy().to_string();
            let inner = build_rest_inner(specs, from + 1, val, make_val)?;
            let mut map = serde_json::Map::new();
            map.insert(key, inner);
            Ok(JsonValue::Object(map))
        }
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Lua {
        let lua = Lua::new();
        register_list(&lua).expect("register_list failed");
        lua
    }

    #[test]
    fn test_get_list_length() {
        let lua = setup();
        let result: i64 = lua
            .load("return llGetListLength({10, 20, 30})")
            .eval()
            .unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn test_get_list_length_empty() {
        let lua = setup();
        let result: i64 = lua.load("return llGetListLength({})").eval().unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_list2_integer() {
        let lua = setup();
        let result: i64 = lua
            .load("return llList2Integer({10, 20, 30}, 1)")
            .eval()
            .unwrap();
        assert_eq!(result, 20);
    }

    #[test]
    fn test_list2_integer_negative_index() {
        let lua = setup();
        let result: i64 = lua
            .load("return llList2Integer({10, 20, 30}, -1)")
            .eval()
            .unwrap();
        assert_eq!(result, 30);
    }

    #[test]
    fn test_list2_integer_out_of_bounds() {
        let lua = setup();
        let result: i64 = lua
            .load("return llList2Integer({10, 20}, 5)")
            .eval()
            .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_list2_float() {
        let lua = setup();
        let result: f64 = lua
            .load("return llList2Float({1.5, 2.5, 3.5}, 0)")
            .eval()
            .unwrap();
        assert!((result - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_list2_string_converts_number() {
        let lua = setup();
        let result: String = lua.load("return llList2String({42}, 0)").eval().unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_list2_string_from_string() {
        let lua = setup();
        let result: String = lua
            .load(r#"return llList2String({"hello", "world"}, 1)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "world");
    }

    #[test]
    fn test_csv2_list() {
        let lua = setup();
        let result: i64 = lua
            .load(r#"return llGetListLength(llCSV2List("a, b, c"))"#)
            .eval()
            .unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn test_csv2_list_values() {
        let lua = setup();
        let result: String = lua
            .load(r#"return llList2String(llCSV2List("a, b, c"), 1)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "b");
    }

    #[test]
    fn test_list2_csv_roundtrip() {
        let lua = setup();
        let result: String = lua
            .load(r#"return llList2CSV({"a", "b", "c"})"#)
            .eval()
            .unwrap();
        assert_eq!(result, "a, b, c");
    }

    #[test]
    fn test_list_find_list_found() {
        let lua = setup();
        let result: i64 = lua
            .load("return llListFindList({10, 20, 30, 40}, {20, 30})")
            .eval()
            .unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_list_find_list_not_found() {
        let lua = setup();
        let result: i64 = lua
            .load("return llListFindList({10, 20, 30}, {99})")
            .eval()
            .unwrap();
        assert_eq!(result, -1);
    }

    #[test]
    fn test_delete_sub_list() {
        let lua = setup();
        let result: i64 = lua
            .load("return llGetListLength(llDeleteSubList({10, 20, 30, 40}, 1, 2))")
            .eval()
            .unwrap();
        assert_eq!(result, 2);
    }

    #[test]
    fn test_list_insert_list() {
        let lua = setup();
        let result: String = lua
            .load(r#"return llList2CSV(llListInsertList({1, 4}, {2, 3}, 1))"#)
            .eval()
            .unwrap();
        assert_eq!(result, "1, 2, 3, 4");
    }

    #[test]
    fn test_get_list_entry_type_integer() {
        let lua = setup();
        let result: i64 = lua
            .load("return llGetListEntryType({42}, 0)")
            .eval()
            .unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn test_get_list_entry_type_string() {
        let lua = setup();
        let result: i64 = lua
            .load(r#"return llGetListEntryType({"hello"}, 0)"#)
            .eval()
            .unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn test_list_statistics_sum() {
        let lua = setup();
        let result: f64 = lua
            .load("return llListStatistics(6, {1, 2, 3, 4, 5})")
            .eval()
            .unwrap();
        assert!((result - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_list_statistics_mean() {
        let lua = setup();
        let result: f64 = lua
            .load("return llListStatistics(3, {2, 4, 6})")
            .eval()
            .unwrap();
        assert!((result - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_list_sort_ascending() {
        let lua = setup();
        let result: String = lua
            .load("return llList2CSV(llListSort({3, 1, 2}, 1, 1))")
            .eval()
            .unwrap();
        assert_eq!(result, "1, 2, 3");
    }

    #[test]
    fn test_list_sort_descending() {
        let lua = setup();
        let result: String = lua
            .load("return llList2CSV(llListSort({3, 1, 2}, 1, 0))")
            .eval()
            .unwrap();
        assert_eq!(result, "3, 2, 1");
    }
}
