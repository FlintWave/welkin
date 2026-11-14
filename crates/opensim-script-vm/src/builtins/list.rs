// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL list function bindings for the Luau VM.

use mlua::prelude::*;

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
        LuaValue::Boolean(b) => if *b { "1".to_string() } else { "0".to_string() },
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
    let mut vals = Vec::with_capacity(len as usize);
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
    let f = lua.create_function(|_, list: LuaTable| {
        Ok(list.raw_len() as i64)
    })?;
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
                    LuaValue::String(s) => {
                        Ok(s.to_string_lossy().parse::<i64>().unwrap_or(0))
                    }
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
                    LuaValue::String(s) => {
                        Ok(s.to_string_lossy().parse::<f64>().unwrap_or(0.0))
                    }
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
                    (LuaValue::Integer(x), LuaValue::Number(y)) => {
                        (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (LuaValue::Number(x), LuaValue::Integer(y)) => {
                        x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (LuaValue::String(x), LuaValue::String(y)) => {
                        Ord::cmp(x.as_bytes().as_ref(), y.as_bytes().as_ref())
                    }
                    // Numbers before strings
                    (LuaValue::Integer(_) | LuaValue::Number(_), _) => {
                        std::cmp::Ordering::Less
                    }
                    (_, LuaValue::Integer(_) | LuaValue::Number(_)) => {
                        std::cmp::Ordering::Greater
                    }
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
                    (LuaValue::Integer(x), LuaValue::Number(y)) => {
                        (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (LuaValue::Number(x), LuaValue::Integer(y)) => {
                        x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (LuaValue::String(x), LuaValue::String(y)) => {
                        Ord::cmp(x.as_bytes().as_ref(), y.as_bytes().as_ref())
                    }
                    (LuaValue::Integer(_) | LuaValue::Number(_), _) => {
                        std::cmp::Ordering::Less
                    }
                    (_, LuaValue::Integer(_) | LuaValue::Number(_)) => {
                        std::cmp::Ordering::Greater
                    }
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
        let s = if start < 0 { (len + start).max(0) } else { start.min(len - 1) } as usize;
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
                        let has_w: bool = t.get::<LuaValue>("w")
                            .map(|v| !matches!(v, LuaValue::Nil))
                            .unwrap_or(false);
                        let has_xyz = t.get::<LuaValue>("x")
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
        let mut parts = Vec::with_capacity(len as usize);
        for i in 1..=len {
            let v: LuaValue = list.raw_get(i as i64)?;
            parts.push(value_to_string(&v));
        }
        Ok(parts.join(", "))
    })?;
    globals.set("llList2CSV", f)?;

    // llListInsertList — insert src into dest at pos (0-based)
    let f = lua.create_function(
        |lua, (dest, src, pos): (LuaTable, LuaTable, i64)| {
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
        },
    )?;
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
                if sorted.len() % 2 == 0 {
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
        let result: i64 = lua.load("return llGetListLength({10, 20, 30})").eval().unwrap();
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
        let result: i64 = lua.load("return llList2Integer({10, 20, 30}, 1)").eval().unwrap();
        assert_eq!(result, 20);
    }

    #[test]
    fn test_list2_integer_negative_index() {
        let lua = setup();
        let result: i64 = lua.load("return llList2Integer({10, 20, 30}, -1)").eval().unwrap();
        assert_eq!(result, 30);
    }

    #[test]
    fn test_list2_integer_out_of_bounds() {
        let lua = setup();
        let result: i64 = lua.load("return llList2Integer({10, 20}, 5)").eval().unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_list2_float() {
        let lua = setup();
        let result: f64 = lua.load("return llList2Float({1.5, 2.5, 3.5}, 0)").eval().unwrap();
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
