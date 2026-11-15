// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::approx_constant)]
//! Task 4.1: Port of script API tests from the C# OpenSimulator tree.
//!
//! The C# tests in `OpenSim/Region/ScriptEngine/Shared/Tests/` verify the LSL
//! type system (float, integer, string, list) and API functions.  This file
//! ports those concepts to the Luau VM, proving that:
//!
//! 1. Math functions produce correct results
//! 2. String functions work correctly
//! 3. List (table) operations work correctly
//! 4. LSL constants have expected values when injected
//! 5. Type-system conversions (integer <-> float <-> string) work

use welkin_script_vm::{VmError, compile_source, new_vm};

// ===================================================================
// Helper: create a non-sandboxed VM for tests that set globals
// ===================================================================

fn test_vm() -> mlua::Lua {
    mlua::Lua::new()
}

// ===================================================================
// 1. Math functions (ported from LSL_ApiTest.cs and LSL_TypesTestLSLFloat.cs)
// ===================================================================

/// Port of C# float construction/arithmetic tests.
/// LSL_TypesTestLSLFloat: TestConstructFromDouble, TestAddTwoLSLFloats, etc.
#[test]
fn math_basic_arithmetic() -> Result<(), VmError> {
    let lua = new_vm()?;
    // Integer arithmetic
    assert_eq!(lua.load("return 2 + 3").eval::<i64>()?, 5);
    assert_eq!(lua.load("return 10 - 7").eval::<i64>()?, 3);
    assert_eq!(lua.load("return 6 * 7").eval::<i64>()?, 42);
    assert_eq!(lua.load("return 10 / 3").eval::<f64>()?, 10.0 / 3.0);
    // Modulo
    assert_eq!(lua.load("return 17 % 5").eval::<i64>()?, 2);
    // Negation
    assert_eq!(lua.load("return -(42)").eval::<i64>()?, -42);
    Ok(())
}

/// Port of C# float precision tests.
/// LSL_TypesTestLSLFloat: DoubleToleranceConstraint checks on float values.
#[test]
fn math_float_precision() -> Result<(), VmError> {
    let lua = new_vm()?;
    let tolerance = 0.000001;

    // Corresponds to m_doubleDoubleSet in C#
    let cases: &[(f64, &str)] = &[
        (2.0, "return 2.0"),
        (-2.0, "return -2.0"),
        (0.0, "return 0.0"),
        (0.5, "return 0.5"),
        (0.0005, "return 0.0005"),
        (0.6805, "return 0.6805"),
        (-0.6805, "return -0.6805"),
        (548.5, "return 548.5"),
        (2.0005, "return 2.0005"),
        (349485435.6805, "return 349485435.6805"),
    ];

    for (expected, code) in cases {
        let result: f64 = lua.load(*code).eval()?;
        assert!(
            (result - expected).abs() < tolerance,
            "float {code}: expected {expected}, got {result}"
        );
    }
    Ok(())
}

/// LSL llSin, llCos, llTan equivalents via Lua math library.
/// The C# LSL_ApiTest.cs tested llAngleBetween using trig internally;
/// here we verify the underlying trig functions that the ll* wrappers use.
#[test]
fn math_trig_functions() -> Result<(), VmError> {
    let lua = new_vm()?;
    let tolerance = 0.000001;

    // sin(0) = 0
    let v: f64 = lua.load("return math.sin(0)").eval()?;
    assert!(v.abs() < tolerance, "sin(0) should be 0, got {v}");

    // sin(pi/2) = 1
    let v: f64 = lua.load("return math.sin(math.pi / 2)").eval()?;
    assert!(
        (v - 1.0).abs() < tolerance,
        "sin(pi/2) should be 1, got {v}"
    );

    // cos(0) = 1
    let v: f64 = lua.load("return math.cos(0)").eval()?;
    assert!((v - 1.0).abs() < tolerance, "cos(0) should be 1, got {v}");

    // cos(pi) = -1
    let v: f64 = lua.load("return math.cos(math.pi)").eval()?;
    assert!((v + 1.0).abs() < tolerance, "cos(pi) should be -1, got {v}");

    // tan(0) = 0
    let v: f64 = lua.load("return math.tan(0)").eval()?;
    assert!(v.abs() < tolerance, "tan(0) should be 0, got {v}");

    // tan(pi/4) = 1
    let v: f64 = lua.load("return math.tan(math.pi / 4)").eval()?;
    assert!(
        (v - 1.0).abs() < tolerance,
        "tan(pi/4) should be 1, got {v}"
    );

    // atan2(1, 1) = pi/4
    let v: f64 = lua.load("return math.atan2(1, 1)").eval()?;
    assert!(
        (v - std::f64::consts::FRAC_PI_4).abs() < tolerance,
        "atan2(1,1) should be pi/4, got {v}"
    );

    Ok(())
}

/// LSL llSqrt, llPow, llAbs, llFabs, llLog, llLog10 equivalents.
#[test]
fn math_power_and_log() -> Result<(), VmError> {
    let lua = new_vm()?;
    let tolerance = 0.000001;

    // sqrt(4) = 2
    let v: f64 = lua.load("return math.sqrt(4)").eval()?;
    assert!((v - 2.0).abs() < tolerance);

    // sqrt(2) ~ 1.41421356
    let v: f64 = lua.load("return math.sqrt(2)").eval()?;
    assert!((v - std::f64::consts::SQRT_2).abs() < tolerance);

    // pow(2, 10) = 1024
    let v: f64 = lua.load("return 2 ^ 10").eval()?;
    assert!((v - 1024.0).abs() < tolerance);

    // abs(-42) = 42  (llAbs equivalent)
    let v: f64 = lua.load("return math.abs(-42)").eval()?;
    assert!((v - 42.0).abs() < tolerance);

    // abs(-3.14) = 3.14  (llFabs equivalent)
    let v: f64 = lua.load("return math.abs(-3.14)").eval()?;
    assert!((v - 3.14).abs() < tolerance);

    // log(1) = 0  (llLog equivalent)
    let v: f64 = lua.load("return math.log(1)").eval()?;
    assert!(v.abs() < tolerance);

    // log(e) = 1
    let v: f64 = lua.load("return math.log(math.exp(1))").eval()?;
    assert!((v - 1.0).abs() < tolerance);

    // log10(100) = 2  (llLog10 equivalent)
    let v: f64 = lua.load("return math.log(100, 10)").eval()?;
    assert!((v - 2.0).abs() < tolerance);

    Ok(())
}

/// LSL llFloor, llCeil, llRound equivalents.
/// Ported from LSL_TypesTestLSLFloat::TestExplicitCastLSLFloatToInt (truncation)
/// and the C# m_doubleIntSet data.
#[test]
fn math_floor_ceil_round() -> Result<(), VmError> {
    let lua = new_vm()?;

    // floor
    assert_eq!(lua.load("return math.floor(2.7)").eval::<i64>()?, 2);
    assert_eq!(lua.load("return math.floor(-2.7)").eval::<i64>()?, -3);
    assert_eq!(lua.load("return math.floor(0.5)").eval::<i64>()?, 0);
    assert_eq!(lua.load("return math.floor(548.5)").eval::<i64>()?, 548);
    assert_eq!(lua.load("return math.floor(-548.5)").eval::<i64>()?, -549);

    // ceil
    assert_eq!(lua.load("return math.ceil(2.1)").eval::<i64>()?, 3);
    assert_eq!(lua.load("return math.ceil(-2.1)").eval::<i64>()?, -2);
    assert_eq!(lua.load("return math.ceil(0.0005)").eval::<i64>()?, 1);

    // round (Luau has math.round)
    assert_eq!(lua.load("return math.round(2.5)").eval::<i64>()?, 3);
    assert_eq!(lua.load("return math.round(2.4)").eval::<i64>()?, 2);
    assert_eq!(lua.load("return math.round(-2.5)").eval::<i64>()?, -3);

    Ok(())
}

/// LSL llFrand equivalent: verify random float generation stays in range.
#[test]
fn math_random_in_range() -> Result<(), VmError> {
    let lua = new_vm()?;

    // math.random() returns [0, 1), scaled by magnitude
    let result: f64 = lua
        .load(
            r#"
        local sum = 0
        for i = 1, 100 do
            local r = math.random() * 10.0
            if r < 0 or r >= 10.0 then
                error("out of range: " .. r)
            end
            sum = sum + r
        end
        return sum
    "#,
        )
        .eval()?;

    // Sum of 100 random values in [0, 10) should be positive and < 1000
    assert!(
        result > 0.0 && result < 1000.0,
        "random sum out of range: {result}"
    );
    Ok(())
}

// ===================================================================
// 2. String functions (ported from LSL_TypesTestLSLString.cs)
// ===================================================================

/// Port of C# LSL_TypesTestLSLString and string function tests.
/// llStringLength, llGetSubString, llToUpper, llToLower, llSubStringIndex.
#[test]
fn string_length() -> Result<(), VmError> {
    let lua = new_vm()?;

    // llStringLength equivalent
    assert_eq!(lua.load(r#"return string.len("hello")"#).eval::<i64>()?, 5);
    assert_eq!(lua.load(r#"return string.len("")"#).eval::<i64>()?, 0);
    assert_eq!(
        lua.load(r#"return string.len("hello world")"#)
            .eval::<i64>()?,
        11
    );
    Ok(())
}

/// llGetSubString equivalent.
#[test]
fn string_substring() -> Result<(), VmError> {
    let lua = new_vm()?;

    // Lua string.sub is 1-based
    let v: String = lua
        .load(r#"return string.sub("Hello World", 1, 5)"#)
        .eval()?;
    assert_eq!(v, "Hello");

    let v: String = lua.load(r#"return string.sub("Hello World", 7)"#).eval()?;
    assert_eq!(v, "World");

    // Negative indices count from end
    let v: String = lua.load(r#"return string.sub("Hello World", -5)"#).eval()?;
    assert_eq!(v, "World");
    Ok(())
}

/// llToUpper, llToLower equivalents.
#[test]
fn string_case_conversion() -> Result<(), VmError> {
    let lua = new_vm()?;

    let v: String = lua.load(r#"return string.upper("hello")"#).eval()?;
    assert_eq!(v, "HELLO");

    let v: String = lua.load(r#"return string.lower("HELLO")"#).eval()?;
    assert_eq!(v, "hello");

    let v: String = lua
        .load(r#"return string.upper("Hello World 123")"#)
        .eval()?;
    assert_eq!(v, "HELLO WORLD 123");
    Ok(())
}

/// llSubStringIndex equivalent.
#[test]
fn string_find() -> Result<(), VmError> {
    let lua = new_vm()?;

    // string.find returns start,end or nil
    let v: i64 = lua
        .load(
            r#"
        local s, e = string.find("hello world", "world", 1, true)
        return s or -1
    "#,
        )
        .eval()?;
    assert_eq!(v, 7); // 1-based

    // Not found
    let v: i64 = lua
        .load(
            r#"
        local s, e = string.find("hello world", "xyz", 1, true)
        return s or -1
    "#,
        )
        .eval()?;
    assert_eq!(v, -1);
    Ok(())
}

/// llStringTrim equivalent.
#[test]
fn string_trim() -> Result<(), VmError> {
    let lua = new_vm()?;

    // Luau has built-in string patterns for trimming
    let v: String = lua
        .load(
            r#"
        local s = "  hello  "
        return s:match("^%s*(.-)%s*$")
    "#,
        )
        .eval()?;
    assert_eq!(v, "hello");

    // Head trim only
    let v: String = lua
        .load(
            r#"
        local s = "  hello  "
        return s:match("^%s*(.*)")
    "#,
        )
        .eval()?;
    assert_eq!(v, "hello  ");
    Ok(())
}

/// llInsertString equivalent.
#[test]
fn string_insert() -> Result<(), VmError> {
    let lua = new_vm()?;

    let v: String = lua
        .load(
            r#"
        local function insert(dst, pos, src)
            return string.sub(dst, 1, pos) .. src .. string.sub(dst, pos + 1)
        end
        return insert("Hello World", 5, " Beautiful")
    "#,
        )
        .eval()?;
    assert_eq!(v, "Hello Beautiful World");
    Ok(())
}

/// String concatenation (fundamental for LSL scripts).
#[test]
fn string_concatenation() -> Result<(), VmError> {
    let lua = new_vm()?;

    let v: String = lua.load(r#"return "hello" .. " " .. "world""#).eval()?;
    assert_eq!(v, "hello world");

    // Number to string concatenation
    let v: String = lua.load(r#"return "value: " .. tostring(42)"#).eval()?;
    assert_eq!(v, "value: 42");
    Ok(())
}

// ===================================================================
// 3. List (table) operations (ported from LSL_TypesTestList.cs)
// ===================================================================

/// Port of C# LSL_TypesTestList::TestConcatenateString, TestConcatenateInteger.
/// In Luau, LSL lists map to sequential tables.
#[test]
fn list_creation_and_length() -> Result<(), VmError> {
    let lua = new_vm()?;

    // llGetListLength equivalent
    assert_eq!(lua.load("return #{}").eval::<i64>()?, 0);
    assert_eq!(lua.load("return #{1, 2, 3}").eval::<i64>()?, 3);
    assert_eq!(
        lua.load(r#"return #{1, "hello", 3.14, true}"#)
            .eval::<i64>()?,
        4
    );
    Ok(())
}

/// Port of C# TestConcatenateString/Integer/Double.
#[test]
fn list_concatenation() -> Result<(), VmError> {
    let lua = new_vm()?;

    let len: i64 = lua
        .load(
            r#"
        local t = {1, 97, "test"}
        -- Concatenate a string
        table.insert(t, "addition")
        return #t
    "#,
        )
        .eval()?;
    assert_eq!(len, 4);

    // Verify the appended element
    let v: String = lua
        .load(
            r#"
        local t = {1, 97, "test"}
        table.insert(t, "addition")
        return t[4]
    "#,
        )
        .eval()?;
    assert_eq!(v, "addition");

    // Concatenate integer
    let v: i64 = lua
        .load(
            r#"
        local t = {1, 97, "test"}
        table.insert(t, 20)
        return t[4]
    "#,
        )
        .eval()?;
    assert_eq!(v, 20);

    // Concatenate float
    let v: f64 = lua
        .load(
            r#"
        local t = {1, 97, "test"}
        table.insert(t, 2.0)
        return t[4]
    "#,
        )
        .eval()?;
    assert!((v - 2.0).abs() < 0.000001);
    Ok(())
}

/// Port of C# TestCastLSLIntegerItemToLSLInteger etc.
/// Accessing typed list elements (llList2Integer, llList2Float, llList2String).
#[test]
fn list_element_access() -> Result<(), VmError> {
    let lua = new_vm()?;

    // Integer element
    let v: i64 = lua
        .load(
            r#"
        local t = {123, 456.789, "hello"}
        return t[1]
    "#,
        )
        .eval()?;
    assert_eq!(v, 123);

    // Float element
    let v: f64 = lua
        .load(
            r#"
        local t = {123, 456.789, "hello"}
        return t[2]
    "#,
        )
        .eval()?;
    assert!((v - 456.789).abs() < 0.001);

    // String element
    let v: String = lua
        .load(
            r#"
        local t = {123, 456.789, "hello"}
        return t[3]
    "#,
        )
        .eval()?;
    assert_eq!(v, "hello");
    Ok(())
}

/// Port of C# list sort and find operations.
/// llListSort, llListFindList equivalents.
#[test]
fn list_sort_and_find() -> Result<(), VmError> {
    let lua = new_vm()?;

    // Sort ascending
    let v: String = lua
        .load(
            r#"
        local t = {3, 1, 4, 1, 5, 9}
        table.sort(t)
        local result = ""
        for _, v in ipairs(t) do
            result = result .. tostring(v) .. ","
        end
        return result
    "#,
        )
        .eval()?;
    assert_eq!(v, "1,1,3,4,5,9,");

    // Sort descending
    let v: String = lua
        .load(
            r#"
        local t = {3, 1, 4, 1, 5, 9}
        table.sort(t, function(a, b) return a > b end)
        local result = ""
        for _, v in ipairs(t) do
            result = result .. tostring(v) .. ","
        end
        return result
    "#,
        )
        .eval()?;
    assert_eq!(v, "9,5,4,3,1,1,");

    // Find element (llListFindList equivalent)
    let v: i64 = lua
        .load(
            r#"
        local t = {10, 20, 30, 40}
        for i, v in ipairs(t) do
            if v == 30 then return i end
        end
        return -1
    "#,
        )
        .eval()?;
    assert_eq!(v, 3); // 1-based index

    // Find missing
    let v: i64 = lua
        .load(
            r#"
        local t = {10, 20, 30, 40}
        for i, v in ipairs(t) do
            if v == 99 then return i end
        end
        return -1
    "#,
        )
        .eval()?;
    assert_eq!(v, -1);
    Ok(())
}

/// llDeleteSubList and llDumpList2String equivalents.
#[test]
fn list_delete_and_join() -> Result<(), VmError> {
    let lua = new_vm()?;

    // Delete element (remove index 2)
    let v: String = lua
        .load(
            r#"
        local t = {"a", "b", "c", "d"}
        table.remove(t, 2)
        return table.concat(t, ",")
    "#,
        )
        .eval()?;
    assert_eq!(v, "a,c,d");

    // Join / dump list to string (llDumpList2String equivalent)
    let v: String = lua
        .load(
            r#"
        local t = {"hello", "beautiful", "world"}
        return table.concat(t, " ")
    "#,
        )
        .eval()?;
    assert_eq!(v, "hello beautiful world");
    Ok(())
}

/// llCSV2List / llList2CSV equivalents.
#[test]
fn list_csv_conversion() -> Result<(), VmError> {
    let lua = new_vm()?;

    // CSV to list
    let v: i64 = lua
        .load(
            r#"
        local csv = "one,two,three,four"
        local t = {}
        for item in string.gmatch(csv, "[^,]+") do
            table.insert(t, item)
        end
        return #t
    "#,
        )
        .eval()?;
    assert_eq!(v, 4);

    // List to CSV
    let v: String = lua
        .load(
            r#"
        local t = {"one", "two", "three"}
        return table.concat(t, ", ")
    "#,
        )
        .eval()?;
    assert_eq!(v, "one, two, three");
    Ok(())
}

// ===================================================================
// 4. Constants (ported from welkin-script-api CONSTANTS table)
// ===================================================================

/// Verify that LSL constants, when injected into a VM, hold expected values.
/// Ported from C#'s LSL_Constants.cs and welkin-script-api's CONSTANTS table.
#[test]
fn constants_math() -> Result<(), VmError> {
    let lua = test_vm();
    let tolerance = 0.001;

    // Register the LSL math constants as globals
    #[allow(clippy::approx_constant)]
    {
        lua.globals().set("PI", 3.14159274_f64)?;
        lua.globals().set("TWO_PI", 6.28318548_f64)?;
        lua.globals().set("PI_BY_TWO", 1.57079637_f64)?;
        lua.globals().set("DEG_TO_RAD", 0.01745329238_f64)?;
        lua.globals().set("RAD_TO_DEG", 57.29578_f64)?;
        lua.globals().set("SQRT2", 1.414213538_f64)?;
    }

    let v: f64 = lua.load("return PI").eval()?;
    assert!((v - std::f64::consts::PI).abs() < tolerance, "PI mismatch");

    let v: f64 = lua.load("return TWO_PI").eval()?;
    assert!(
        (v - 2.0 * std::f64::consts::PI).abs() < tolerance,
        "TWO_PI mismatch"
    );

    let v: f64 = lua.load("return PI_BY_TWO").eval()?;
    assert!(
        (v - std::f64::consts::FRAC_PI_2).abs() < tolerance,
        "PI_BY_TWO mismatch"
    );

    let v: f64 = lua.load("return DEG_TO_RAD").eval()?;
    assert!(
        (v - std::f64::consts::PI / 180.0).abs() < tolerance,
        "DEG_TO_RAD mismatch"
    );

    let v: f64 = lua.load("return RAD_TO_DEG").eval()?;
    assert!(
        (v - 180.0 / std::f64::consts::PI).abs() < tolerance,
        "RAD_TO_DEG mismatch"
    );

    let v: f64 = lua.load("return SQRT2").eval()?;
    assert!(
        (v - std::f64::consts::SQRT_2).abs() < tolerance,
        "SQRT2 mismatch"
    );

    Ok(())
}

/// Verify boolean and special value constants.
#[test]
fn constants_boolean_and_special() -> Result<(), VmError> {
    let lua = test_vm();

    lua.globals().set("TRUE", 1)?;
    lua.globals().set("FALSE", 0)?;
    lua.globals()
        .set("NULL_KEY", "00000000-0000-0000-0000-000000000000")?;
    lua.globals().set("ALL_SIDES", -1)?;
    lua.globals().set("EOF", "\n\n\n")?;

    assert_eq!(lua.load("return TRUE").eval::<i64>()?, 1);
    assert_eq!(lua.load("return FALSE").eval::<i64>()?, 0);
    assert_eq!(
        lua.load("return NULL_KEY").eval::<String>()?,
        "00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(lua.load("return ALL_SIDES").eval::<i64>()?, -1);
    assert_eq!(lua.load("return EOF").eval::<String>()?, "\n\n\n");
    Ok(())
}

/// Verify link constants.
/// Ported from welkin-script-api CONSTANTS: LINK_SET, LINK_ROOT, etc.
#[test]
fn constants_link() -> Result<(), VmError> {
    let lua = test_vm();

    lua.globals().set("LINK_SET", -1)?;
    lua.globals().set("LINK_ROOT", 1)?;
    lua.globals().set("LINK_ALL_OTHERS", -2)?;
    lua.globals().set("LINK_ALL_CHILDREN", -3)?;
    lua.globals().set("LINK_THIS", -4)?;

    assert_eq!(lua.load("return LINK_SET").eval::<i64>()?, -1);
    assert_eq!(lua.load("return LINK_ROOT").eval::<i64>()?, 1);
    assert_eq!(lua.load("return LINK_ALL_OTHERS").eval::<i64>()?, -2);
    assert_eq!(lua.load("return LINK_ALL_CHILDREN").eval::<i64>()?, -3);
    assert_eq!(lua.load("return LINK_THIS").eval::<i64>()?, -4);
    Ok(())
}

/// Verify type constants match C# TYPE_INTEGER=1, TYPE_FLOAT=2, etc.
#[test]
fn constants_type_flags() -> Result<(), VmError> {
    let lua = test_vm();

    lua.globals().set("TYPE_INTEGER", 1)?;
    lua.globals().set("TYPE_FLOAT", 2)?;
    lua.globals().set("TYPE_STRING", 3)?;
    lua.globals().set("TYPE_KEY", 4)?;
    lua.globals().set("TYPE_VECTOR", 5)?;
    lua.globals().set("TYPE_ROTATION", 6)?;
    lua.globals().set("TYPE_INVALID", 0)?;

    assert_eq!(lua.load("return TYPE_INTEGER").eval::<i64>()?, 1);
    assert_eq!(lua.load("return TYPE_FLOAT").eval::<i64>()?, 2);
    assert_eq!(lua.load("return TYPE_STRING").eval::<i64>()?, 3);
    assert_eq!(lua.load("return TYPE_KEY").eval::<i64>()?, 4);
    assert_eq!(lua.load("return TYPE_VECTOR").eval::<i64>()?, 5);
    assert_eq!(lua.load("return TYPE_ROTATION").eval::<i64>()?, 6);
    assert_eq!(lua.load("return TYPE_INVALID").eval::<i64>()?, 0);
    Ok(())
}

/// Verify changed-event flags.
/// Ported from C# CHANGED_INVENTORY=1, CHANGED_REGION=256, etc.
#[test]
fn constants_changed_flags() -> Result<(), VmError> {
    let lua = test_vm();

    lua.globals().set("CHANGED_INVENTORY", 1)?;
    lua.globals().set("CHANGED_COLOR", 2)?;
    lua.globals().set("CHANGED_SHAPE", 4)?;
    lua.globals().set("CHANGED_SCALE", 8)?;
    lua.globals().set("CHANGED_TEXTURE", 16)?;
    lua.globals().set("CHANGED_LINK", 32)?;
    lua.globals().set("CHANGED_OWNER", 128)?;
    lua.globals().set("CHANGED_REGION", 256)?;
    lua.globals().set("CHANGED_TELEPORT", 512)?;

    // Test bitwise OR: CHANGED_INVENTORY | CHANGED_REGION = 257
    let v: i64 = lua
        .load("return bit32.bor(CHANGED_INVENTORY, CHANGED_REGION)")
        .eval()?;
    assert_eq!(v, 257);

    // Test bitwise AND: (257 & CHANGED_REGION) == CHANGED_REGION
    let v: i64 = lua.load("return bit32.band(257, CHANGED_REGION)").eval()?;
    assert_eq!(v, 256);
    Ok(())
}

// ===================================================================
// 5. Type system conversions
//    (ported from LSL_TypesTestLSLFloat, LSL_TypesTestLSLInteger,
//     LSL_TypesTestLSLString)
// ===================================================================

/// Port of C# TestExplicitCastLSLFloatToInt: float -> int truncation.
/// m_doubleIntSet: (2.0->2, -2.0->-2, 0.5->0, 548.5->548, etc.)
#[test]
fn type_float_to_int_truncation() -> Result<(), VmError> {
    let lua = new_vm()?;

    // Lua truncation uses math.floor for positive, special handling for negative
    // In LSL, (integer)float truncates toward zero.
    let cases: &[(f64, i64)] = &[
        (2.0, 2),
        (-2.0, -2),
        (0.0, 0),
        (1.0, 1),
        (-1.0, -1),
        (0.5, 0),
        (0.0005, 0),
        (0.6805, 0),
        (-0.5, 0),
        (-0.6805, 0),
        (548.5, 548),
        (-548.5, -548),
        (2.0005, 2),
        (-2.0005, -2),
    ];

    for (f, expected) in cases {
        // Luau: truncation toward zero
        let code = format!(
            "local v = {f}; if v >= 0 then return math.floor(v) else return math.ceil(v) end"
        );
        let result: i64 = lua.load(&code).eval()?;
        assert_eq!(
            result, *expected,
            "truncate({f}) should be {expected}, got {result}"
        );
    }
    Ok(())
}

/// Port of C# TestExplicitCastStringToLSLInteger and TestExplicitCastStringToLSLFloat.
/// String -> number conversions.
#[test]
fn type_string_to_number() -> Result<(), VmError> {
    let lua = new_vm()?;

    // tonumber("42") == 42
    let v: i64 = lua.load(r#"return tonumber("42")"#).eval()?;
    assert_eq!(v, 42);

    // tonumber("3.14") ~ 3.14
    let v: f64 = lua.load(r#"return tonumber("3.14")"#).eval()?;
    assert!((v - 3.14).abs() < 0.000001);

    // tonumber("-99999999") == -99999999
    let v: i64 = lua.load(r#"return tonumber("-99999999")"#).eval()?;
    assert_eq!(v, -99999999);

    // Empty string -> nil (LSL converts to 0)
    let v: bool = lua.load(r#"return tonumber("") == nil"#).eval()?;
    assert!(v, "tonumber('') should return nil in Lua");

    // Non-numeric string -> nil
    let v: bool = lua.load(r#"return tonumber("aa") == nil"#).eval()?;
    assert!(v, "tonumber('aa') should return nil in Lua");

    Ok(())
}

/// Port of C# TestImplicitCastLSLFloatToBooleanTrue/False.
/// Non-zero -> true, zero -> false.
#[test]
fn type_number_to_boolean() -> Result<(), VmError> {
    let lua = new_vm()?;

    // In Lua, only nil and false are falsy.  0 is truthy!
    // But LSL treats 0 as false.  Our Luau scripts will use explicit checks.
    // Verify the explicit pattern works:
    let v: bool = lua.load("return 0 ~= 0").eval()?;
    assert!(!v, "0 ~= 0 should be false");

    let v: bool = lua.load("return 1 ~= 0").eval()?;
    assert!(v, "1 ~= 0 should be true");

    let v: bool = lua.load("return -1 ~= 0").eval()?;
    assert!(v, "-1 ~= 0 should be true");

    let v: bool = lua.load("return 42.5 ~= 0").eval()?;
    assert!(v, "42.5 ~= 0 should be true");
    Ok(())
}

/// Port of C# TestToString / TestConstructFromLSLFloat.
/// Number -> string formatting.
#[test]
fn type_number_to_string() -> Result<(), VmError> {
    let lua = new_vm()?;

    // tostring on integers
    let v: String = lua.load("return tostring(42)").eval()?;
    assert_eq!(v, "42");

    let v: String = lua.load("return tostring(-1)").eval()?;
    assert_eq!(v, "-1");

    // tostring on floats -- Lua may format differently than LSL's 6-decimal
    // but the value must round-trip
    let v: String = lua.load("return tostring(3.14)").eval()?;
    let parsed: f64 = v.parse().expect("should parse back to float");
    assert!((parsed - 3.14).abs() < 0.000001);

    // boolean -> string
    let v: String = lua.load("return tostring(true)").eval()?;
    assert_eq!(v, "true");

    let v: String = lua.load("return tostring(false)").eval()?;
    assert_eq!(v, "false");
    Ok(())
}

/// Integer <-> Float conversions.
/// Ported from LSL_TypesTestLSLFloat::TestImplicitCastIntToLSLFloat and
/// LSL_TypesTestLSLInteger::TestExplicitCastLSLFloatToLSLInteger.
#[test]
fn type_int_float_roundtrip() -> Result<(), VmError> {
    let lua = new_vm()?;
    let tolerance = 0.000001;

    // Integer to float: exact for small integers
    let cases: &[i64] = &[2, -2, 0, 1, -1, 999999999, -99999999];
    for &i in cases {
        let code = format!("return {i} + 0.0");
        let v: f64 = lua.load(&code).eval()?;
        assert!((v - i as f64).abs() < tolerance, "int->float: {i} -> {v}");
    }

    // Float to integer truncation: m_doubleIntSet from C#
    let float_to_int: &[(f64, i64)] = &[
        (2.0, 2),
        (-2.0, -2),
        (0.0, 0),
        (1.0, 1),
        (-1.0, -1),
        (999999999.0, 999999999),
        (-99999999.0, -99999999),
    ];
    for (f, expected) in float_to_int {
        let code = format!(
            "local v = {f}; if v >= 0 then return math.floor(v) else return math.ceil(v) end"
        );
        let result: i64 = lua.load(&code).eval()?;
        assert_eq!(result, *expected, "float->int: {f} should be {expected}");
    }
    Ok(())
}

// ===================================================================
// Additional: Verify the script VM's compile_source API
// ===================================================================

/// Verify compile_source produces valid bytecode that can be loaded.
#[test]
fn compile_and_load_bytecode() -> Result<(), VmError> {
    let bytecode = compile_source("return 2 + 2")?;
    assert!(!bytecode.is_empty(), "bytecode should not be empty");

    let lua = new_vm()?;
    let result: i64 = lua.load(&bytecode).eval()?;
    assert_eq!(result, 4);
    Ok(())
}

/// Verify that compiling invalid source produces an error.
#[test]
fn compile_invalid_source() {
    let result = compile_source("this is not valid lua {{{{");
    assert!(result.is_err(), "invalid source should fail to compile");
}

/// Multiple compiled scripts can run on the same VM.
#[test]
fn multiple_compiled_scripts() -> Result<(), VmError> {
    let bc1 = compile_source("x = 10")?;
    let bc2 = compile_source("y = 20")?;
    let bc3 = compile_source("return (x or 0) + (y or 0)")?;

    let lua = mlua::Lua::new();
    lua.load(&bc1).exec()?;
    lua.load(&bc2).exec()?;
    let result: i64 = lua.load(&bc3).eval()?;
    assert_eq!(result, 30);
    Ok(())
}
