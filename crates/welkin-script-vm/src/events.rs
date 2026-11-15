// SPDX-License-Identifier: LGPL-3.0-or-later
//! Detected parameters and event dispatch for LSL script events.
//!
//! LSL scripts receive events (touch_start, collision, listen, etc.) with
//! "detected" parameters accessible via `llDetectedName`, `llDetectedKey`,
//! `llDetectedPos`, and so on.  This module provides the [`DetectedInfo`]
//! struct, an [`EventContext`] to hold the current event's detected list,
//! and registration / dispatch helpers that wire these into a Luau VM.
//!
//! Reference: OpenSim `LSL_Api.cs` lines 1322-1438.

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Detection type constants (matches LSL)
// ---------------------------------------------------------------------------

/// Detected entity is an agent (avatar).
pub const DETECTION_TYPE_AGENT: i32 = 1;
/// Detected entity is an active (physical / moving) object.
pub const DETECTION_TYPE_ACTIVE: i32 = 2;
/// Detected entity is a passive (non-physical, non-scripted) object.
pub const DETECTION_TYPE_PASSIVE: i32 = 4;
/// Detected entity is a scripted object.
pub const DETECTION_TYPE_SCRIPTED: i32 = 8;

// ---------------------------------------------------------------------------
// DetectedInfo
// ---------------------------------------------------------------------------

/// Information about a single detected entity during an event.
///
/// Mirrors the C# `DetectParams` class in OpenSimulator.
#[derive(Debug, Clone)]
pub struct DetectedInfo {
    /// UUID of the detected agent or object.
    pub key: Uuid,
    /// Display name of the detected entity.
    pub name: String,
    /// UUID of the entity's owner.
    pub owner: Uuid,
    /// World position as `[x, y, z]`.
    pub position: [f64; 3],
    /// Rotation as `[x, y, z, s]` (quaternion).
    pub rotation: [f64; 4],
    /// Velocity as `[x, y, z]`.
    pub velocity: [f64; 3],
    /// Group UUID of the detected entity.
    pub group: Uuid,
    /// Link number within the linkset.
    pub link_num: i32,
    /// Bitmask of detection type flags (`AGENT`, `ACTIVE`, `PASSIVE`, `SCRIPTED`).
    pub detection_type: i32,
    /// Face index touched (-1 if not a touch event).
    pub touch_face: i32,
    /// World position of the touch point.
    pub touch_pos: [f64; 3],
    /// Surface normal at the touch point.
    pub touch_normal: [f64; 3],
    /// Surface binormal at the touch point.
    pub touch_binormal: [f64; 3],
    /// Texture ST coordinates of the touch point (`[s, t]`).
    pub touch_st: [f64; 2],
    /// Texture UV coordinates of the touch point (`[u, v]`).
    pub touch_uv: [f64; 2],
}

impl Default for DetectedInfo {
    fn default() -> Self {
        Self {
            key: Uuid::nil(),
            name: String::new(),
            owner: Uuid::nil(),
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
            velocity: [0.0; 3],
            group: Uuid::nil(),
            link_num: 0,
            detection_type: 0,
            touch_face: -1,
            touch_pos: [0.0; 3],
            touch_normal: [0.0; 3],
            touch_binormal: [0.0; 3],
            touch_st: [-1.0, -1.0],
            touch_uv: [-1.0, -1.0],
        }
    }
}

// ---------------------------------------------------------------------------
// EventContext
// ---------------------------------------------------------------------------

/// Holds the detected-parameter list for the currently executing event.
///
/// Shared with Lua closures via `Arc<RwLock<EventContext>>`.
#[derive(Debug, Clone, Default)]
pub struct EventContext {
    /// The list of detected entities for the current event.
    pub detected: Vec<DetectedInfo>,
}

// ---------------------------------------------------------------------------
// Lua helper: push a vector table {x, y, z}
// ---------------------------------------------------------------------------

fn push_vector3(lua: &Lua, v: [f64; 3]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    t.set("z", v[2])?;
    Ok(t)
}

fn push_vector2(lua: &Lua, v: [f64; 2]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", v[0])?;
    t.set("y", v[1])?;
    Ok(t)
}

fn push_rotation(lua: &Lua, r: [f64; 4]) -> LuaResult<LuaTable> {
    let t = lua.create_table()?;
    t.set("x", r[0])?;
    t.set("y", r[1])?;
    t.set("z", r[2])?;
    t.set("s", r[3])?;
    Ok(t)
}

// ---------------------------------------------------------------------------
// Default values for out-of-range indices (matches C# LSL_Api behaviour)
// ---------------------------------------------------------------------------

/// Zero vector default for out-of-range detected position/velocity/touch.
const ZERO_VEC3: [f64; 3] = [0.0, 0.0, 0.0];
/// Identity rotation default for out-of-range detected rotation.
const ZERO_ROT: [f64; 4] = [0.0, 0.0, 0.0, 1.0];
/// Default touch ST/UV when index is out of range (C# returns {-1, -1, 0}).
const DEFAULT_TOUCH_ST_UV: [f64; 2] = [-1.0, -1.0];

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all `llDetected*` functions as Lua globals.
///
/// Each function captures a shared reference to the [`EventContext`] and
/// reads from the detected list by index.
pub fn register_detected_functions(lua: &Lua, ctx: Arc<RwLock<EventContext>>) -> LuaResult<()> {
    let globals = lua.globals();

    // llDetectedKey(index) -> String
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |_, index: i32| {
            let guard = ctx.read();
            match guard.detected.get(index as usize) {
                Some(d) => Ok(d.key.to_string()),
                None => Ok(Uuid::nil().to_string()),
            }
        })?;
        globals.set("llDetectedKey", f)?;
    }

    // llDetectedName(index) -> String
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |_, index: i32| {
            let guard = ctx.read();
            match guard.detected.get(index as usize) {
                Some(d) => Ok(d.name.clone()),
                None => Ok(String::new()),
            }
        })?;
        globals.set("llDetectedName", f)?;
    }

    // llDetectedPos(index) -> table {x, y, z}
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |lua, index: i32| {
            let guard = ctx.read();
            let v = guard
                .detected
                .get(index as usize)
                .map_or(ZERO_VEC3, |d| d.position);
            push_vector3(lua, v)
        })?;
        globals.set("llDetectedPos", f)?;
    }

    // llDetectedRot(index) -> table {x, y, z, s}
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |lua, index: i32| {
            let guard = ctx.read();
            let r = guard
                .detected
                .get(index as usize)
                .map_or(ZERO_ROT, |d| d.rotation);
            push_rotation(lua, r)
        })?;
        globals.set("llDetectedRot", f)?;
    }

    // llDetectedVel(index) -> table {x, y, z}
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |lua, index: i32| {
            let guard = ctx.read();
            let v = guard
                .detected
                .get(index as usize)
                .map_or(ZERO_VEC3, |d| d.velocity);
            push_vector3(lua, v)
        })?;
        globals.set("llDetectedVel", f)?;
    }

    // llDetectedOwner(index) -> String
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |_, index: i32| {
            let guard = ctx.read();
            match guard.detected.get(index as usize) {
                Some(d) => Ok(d.owner.to_string()),
                None => Ok(Uuid::nil().to_string()),
            }
        })?;
        globals.set("llDetectedOwner", f)?;
    }

    // llDetectedGroup(index) -> String
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |_, index: i32| {
            let guard = ctx.read();
            match guard.detected.get(index as usize) {
                Some(d) => Ok(d.group.to_string()),
                None => Ok(Uuid::nil().to_string()),
            }
        })?;
        globals.set("llDetectedGroup", f)?;
    }

    // llDetectedType(index) -> integer
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |_, index: i32| {
            let guard = ctx.read();
            Ok(guard
                .detected
                .get(index as usize)
                .map_or(0, |d| d.detection_type))
        })?;
        globals.set("llDetectedType", f)?;
    }

    // llDetectedLinkNumber(index) -> integer
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |_, index: i32| {
            let guard = ctx.read();
            Ok(guard.detected.get(index as usize).map_or(0, |d| d.link_num))
        })?;
        globals.set("llDetectedLinkNumber", f)?;
    }

    // llDetectedTouchFace(index) -> integer
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |_, index: i32| {
            let guard = ctx.read();
            Ok(guard
                .detected
                .get(index as usize)
                .map_or(-1, |d| d.touch_face))
        })?;
        globals.set("llDetectedTouchFace", f)?;
    }

    // llDetectedTouchPos(index) -> table {x, y, z}
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |lua, index: i32| {
            let guard = ctx.read();
            let v = guard
                .detected
                .get(index as usize)
                .map_or(ZERO_VEC3, |d| d.touch_pos);
            push_vector3(lua, v)
        })?;
        globals.set("llDetectedTouchPos", f)?;
    }

    // llDetectedTouchNormal(index) -> table {x, y, z}
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |lua, index: i32| {
            let guard = ctx.read();
            let v = guard
                .detected
                .get(index as usize)
                .map_or(ZERO_VEC3, |d| d.touch_normal);
            push_vector3(lua, v)
        })?;
        globals.set("llDetectedTouchNormal", f)?;
    }

    // llDetectedTouchBinormal(index) -> table {x, y, z}
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |lua, index: i32| {
            let guard = ctx.read();
            let v = guard
                .detected
                .get(index as usize)
                .map_or(ZERO_VEC3, |d| d.touch_binormal);
            push_vector3(lua, v)
        })?;
        globals.set("llDetectedTouchBinormal", f)?;
    }

    // llDetectedTouchST(index) -> table {x, y}
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |lua, index: i32| {
            let guard = ctx.read();
            let v = guard
                .detected
                .get(index as usize)
                .map_or(DEFAULT_TOUCH_ST_UV, |d| d.touch_st);
            push_vector2(lua, v)
        })?;
        globals.set("llDetectedTouchST", f)?;
    }

    // llDetectedTouchUV(index) -> table {x, y}
    {
        let ctx = Arc::clone(&ctx);
        let f = lua.create_function(move |lua, index: i32| {
            let guard = ctx.read();
            let v = guard
                .detected
                .get(index as usize)
                .map_or(DEFAULT_TOUCH_ST_UV, |d| d.touch_uv);
            push_vector2(lua, v)
        })?;
        globals.set("llDetectedTouchUV", f)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Event dispatch
// ---------------------------------------------------------------------------

/// Dispatch an event to the Luau VM.
///
/// 1. Sets the event context with the provided detected list.
/// 2. Calls the Luau function named `event_name` (e.g. `touch_start`),
///    passing `#detected` as the `num_detected` argument.
/// 3. Clears the event context after the handler returns (or errors).
///
/// If the named function does not exist in the VM, this is a no-op
/// (matching LSL semantics).
pub fn dispatch_event(
    lua: &Lua,
    ctx: &Arc<RwLock<EventContext>>,
    event_name: &str,
    detected: Vec<DetectedInfo>,
) -> LuaResult<()> {
    let num_detected = detected.len() as i32;

    // Set the detected list.
    {
        let mut guard = ctx.write();
        guard.detected = detected;
    }

    // Look up the handler function.
    let result = (|| -> LuaResult<()> {
        let globals = lua.globals();
        let func: LuaFunction = match globals.get(event_name) {
            Ok(f) => f,
            Err(LuaError::FromLuaConversionError { .. }) => return Ok(()),
            Err(e) => return Err(e),
        };
        func.call::<()>(num_detected)?;
        Ok(())
    })();

    // Always clear the context, even on error.
    {
        let mut guard = ctx.write();
        guard.detected.clear();
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a non-sandboxed Lua VM with detected functions registered.
    fn setup_vm(detected: Vec<DetectedInfo>) -> (Lua, Arc<RwLock<EventContext>>) {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(EventContext { detected }));
        register_detected_functions(&lua, Arc::clone(&ctx))
            .expect("failed to register detected functions");
        (lua, ctx)
    }

    fn make_test_info() -> DetectedInfo {
        DetectedInfo {
            key: Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap(),
            name: "Test Agent".to_string(),
            owner: Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789").unwrap(),
            position: [128.0, 64.0, 32.0],
            rotation: [0.0, 0.0, 0.707, 0.707],
            velocity: [1.0, 2.0, 3.0],
            group: Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap(),
            link_num: 2,
            detection_type: DETECTION_TYPE_AGENT,
            touch_face: 3,
            touch_pos: [10.0, 20.0, 30.0],
            touch_normal: [0.0, 0.0, 1.0],
            touch_binormal: [1.0, 0.0, 0.0],
            touch_st: [0.5, 0.75],
            touch_uv: [0.25, 0.125],
        }
    }

    // ---------------------------------------------------------------
    // 1. DetectedInfo creation with defaults
    // ---------------------------------------------------------------
    #[test]
    fn detected_info_defaults() {
        let d = DetectedInfo::default();
        assert_eq!(d.key, Uuid::nil());
        assert_eq!(d.name, "");
        assert_eq!(d.owner, Uuid::nil());
        assert_eq!(d.position, [0.0, 0.0, 0.0]);
        assert_eq!(d.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(d.velocity, [0.0, 0.0, 0.0]);
        assert_eq!(d.group, Uuid::nil());
        assert_eq!(d.link_num, 0);
        assert_eq!(d.detection_type, 0);
        assert_eq!(d.touch_face, -1);
        assert_eq!(d.touch_pos, [0.0, 0.0, 0.0]);
        assert_eq!(d.touch_normal, [0.0, 0.0, 0.0]);
        assert_eq!(d.touch_binormal, [0.0, 0.0, 0.0]);
        assert_eq!(d.touch_st, [-1.0, -1.0]);
        assert_eq!(d.touch_uv, [-1.0, -1.0]);
    }

    // ---------------------------------------------------------------
    // 2. llDetectedKey returns correct UUID string
    // ---------------------------------------------------------------
    #[test]
    fn detected_key_returns_uuid() {
        let info = make_test_info();
        let expected = info.key.to_string();
        let (lua, _ctx) = setup_vm(vec![info]);

        let result: String = lua
            .load("return llDetectedKey(0)")
            .eval()
            .expect("llDetectedKey failed");
        assert_eq!(result, expected);
    }

    // ---------------------------------------------------------------
    // 3. llDetectedName returns correct name
    // ---------------------------------------------------------------
    #[test]
    fn detected_name_returns_name() {
        let info = make_test_info();
        let (lua, _ctx) = setup_vm(vec![info]);

        let result: String = lua
            .load("return llDetectedName(0)")
            .eval()
            .expect("llDetectedName failed");
        assert_eq!(result, "Test Agent");
    }

    // ---------------------------------------------------------------
    // 4. llDetectedPos returns vector table
    // ---------------------------------------------------------------
    #[test]
    fn detected_pos_returns_vector() {
        let info = make_test_info();
        let (lua, _ctx) = setup_vm(vec![info]);

        let result: LuaTable = lua
            .load("return llDetectedPos(0)")
            .eval()
            .expect("llDetectedPos failed");
        let x: f64 = result.get("x").unwrap();
        let y: f64 = result.get("y").unwrap();
        let z: f64 = result.get("z").unwrap();
        assert!((x - 128.0).abs() < f64::EPSILON);
        assert!((y - 64.0).abs() < f64::EPSILON);
        assert!((z - 32.0).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // 5. Out-of-range index returns sensible defaults
    // ---------------------------------------------------------------
    #[test]
    fn out_of_range_returns_defaults() {
        let (lua, _ctx) = setup_vm(vec![]); // no detected items

        // String functions return empty / nil UUID.
        let key: String = lua
            .load("return llDetectedKey(0)")
            .eval()
            .expect("llDetectedKey OOB failed");
        assert_eq!(key, Uuid::nil().to_string());

        let name: String = lua
            .load("return llDetectedName(0)")
            .eval()
            .expect("llDetectedName OOB failed");
        assert_eq!(name, "");

        // Integer functions return 0 (or -1 for touch face).
        let dtype: i32 = lua
            .load("return llDetectedType(0)")
            .eval()
            .expect("llDetectedType OOB failed");
        assert_eq!(dtype, 0);

        let face: i32 = lua
            .load("return llDetectedTouchFace(0)")
            .eval()
            .expect("llDetectedTouchFace OOB failed");
        assert_eq!(face, -1);

        // Vector function returns zero vector.
        let pos: LuaTable = lua
            .load("return llDetectedPos(0)")
            .eval()
            .expect("llDetectedPos OOB failed");
        let x: f64 = pos.get("x").unwrap();
        let y: f64 = pos.get("y").unwrap();
        let z: f64 = pos.get("z").unwrap();
        assert!((x).abs() < f64::EPSILON);
        assert!((y).abs() < f64::EPSILON);
        assert!((z).abs() < f64::EPSILON);

        // Touch ST returns {-1, -1}.
        let st: LuaTable = lua
            .load("return llDetectedTouchST(0)")
            .eval()
            .expect("llDetectedTouchST OOB failed");
        let sx: f64 = st.get("x").unwrap();
        let sy: f64 = st.get("y").unwrap();
        assert!((sx - (-1.0)).abs() < f64::EPSILON);
        assert!((sy - (-1.0)).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // 6. Touch detection fields work
    // ---------------------------------------------------------------
    #[test]
    fn touch_detection_fields() {
        let info = make_test_info();
        let (lua, _ctx) = setup_vm(vec![info]);

        let face: i32 = lua
            .load("return llDetectedTouchFace(0)")
            .eval()
            .expect("llDetectedTouchFace failed");
        assert_eq!(face, 3);

        let tp: LuaTable = lua
            .load("return llDetectedTouchPos(0)")
            .eval()
            .expect("llDetectedTouchPos failed");
        let x: f64 = tp.get("x").unwrap();
        let y: f64 = tp.get("y").unwrap();
        let z: f64 = tp.get("z").unwrap();
        assert!((x - 10.0).abs() < f64::EPSILON);
        assert!((y - 20.0).abs() < f64::EPSILON);
        assert!((z - 30.0).abs() < f64::EPSILON);

        let tn: LuaTable = lua
            .load("return llDetectedTouchNormal(0)")
            .eval()
            .expect("llDetectedTouchNormal failed");
        let nz: f64 = tn.get("z").unwrap();
        assert!((nz - 1.0).abs() < f64::EPSILON);

        let tb: LuaTable = lua
            .load("return llDetectedTouchBinormal(0)")
            .eval()
            .expect("llDetectedTouchBinormal failed");
        let bx: f64 = tb.get("x").unwrap();
        assert!((bx - 1.0).abs() < f64::EPSILON);

        let st: LuaTable = lua
            .load("return llDetectedTouchST(0)")
            .eval()
            .expect("llDetectedTouchST failed");
        let sx: f64 = st.get("x").unwrap();
        let sy: f64 = st.get("y").unwrap();
        assert!((sx - 0.5).abs() < f64::EPSILON);
        assert!((sy - 0.75).abs() < f64::EPSILON);

        let uv: LuaTable = lua
            .load("return llDetectedTouchUV(0)")
            .eval()
            .expect("llDetectedTouchUV failed");
        let ux: f64 = uv.get("x").unwrap();
        let uy: f64 = uv.get("y").unwrap();
        assert!((ux - 0.25).abs() < f64::EPSILON);
        assert!((uy - 0.125).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // 7. dispatch_event calls the right handler
    // ---------------------------------------------------------------
    #[test]
    fn dispatch_event_calls_handler() {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(EventContext::default()));
        register_detected_functions(&lua, Arc::clone(&ctx)).expect("register failed");

        // Define a handler that records its argument.
        lua.load(
            r#"
            _called = false
            _num = 0
            function touch_start(num_detected)
                _called = true
                _num = num_detected
            end
        "#,
        )
        .exec()
        .expect("handler definition failed");

        let info = make_test_info();
        dispatch_event(&lua, &ctx, "touch_start", vec![info]).expect("dispatch_event failed");

        let called: bool = lua.globals().get("_called").unwrap();
        assert!(called, "touch_start handler was not called");

        let num: i32 = lua.globals().get("_num").unwrap();
        assert_eq!(num, 1);

        // Context should be cleared after dispatch.
        let guard = ctx.read();
        assert!(
            guard.detected.is_empty(),
            "context not cleared after dispatch"
        );
    }

    // ---------------------------------------------------------------
    // 8. Multiple detected items (touch_start with 2 agents)
    // ---------------------------------------------------------------
    #[test]
    fn multiple_detected_items() {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(EventContext::default()));
        register_detected_functions(&lua, Arc::clone(&ctx)).expect("register failed");

        // Handler that reads both detected items.
        lua.load(
            r#"
            _name0 = ""
            _name1 = ""
            _count = 0
            function touch_start(num_detected)
                _count = num_detected
                _name0 = llDetectedName(0)
                _name1 = llDetectedName(1)
            end
        "#,
        )
        .exec()
        .expect("handler definition failed");

        let mut info0 = make_test_info();
        info0.name = "Agent Alpha".to_string();
        info0.key = Uuid::new_v4();

        let mut info1 = make_test_info();
        info1.name = "Agent Beta".to_string();
        info1.key = Uuid::new_v4();

        dispatch_event(&lua, &ctx, "touch_start", vec![info0, info1])
            .expect("dispatch_event failed");

        let count: i32 = lua.globals().get("_count").unwrap();
        assert_eq!(count, 2);

        let name0: String = lua.globals().get("_name0").unwrap();
        assert_eq!(name0, "Agent Alpha");

        let name1: String = lua.globals().get("_name1").unwrap();
        assert_eq!(name1, "Agent Beta");
    }

    // ---------------------------------------------------------------
    // 9. dispatch_event with missing handler is a no-op
    // ---------------------------------------------------------------
    #[test]
    fn dispatch_missing_handler_is_noop() {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(EventContext::default()));
        register_detected_functions(&lua, Arc::clone(&ctx)).expect("register failed");

        // No handler defined -- should succeed silently.
        let result = dispatch_event(&lua, &ctx, "nonexistent_event", vec![]);
        assert!(result.is_ok(), "missing handler should not error");
    }

    // ---------------------------------------------------------------
    // 10. Remaining detected functions (owner, group, type, link, rot, vel)
    // ---------------------------------------------------------------
    #[test]
    fn remaining_detected_functions() {
        let info = make_test_info();
        let expected_owner = info.owner.to_string();
        let expected_group = info.group.to_string();
        let (lua, _ctx) = setup_vm(vec![info]);

        let owner: String = lua
            .load("return llDetectedOwner(0)")
            .eval()
            .expect("llDetectedOwner failed");
        assert_eq!(owner, expected_owner);

        let group: String = lua
            .load("return llDetectedGroup(0)")
            .eval()
            .expect("llDetectedGroup failed");
        assert_eq!(group, expected_group);

        let dtype: i32 = lua
            .load("return llDetectedType(0)")
            .eval()
            .expect("llDetectedType failed");
        assert_eq!(dtype, DETECTION_TYPE_AGENT);

        let link: i32 = lua
            .load("return llDetectedLinkNumber(0)")
            .eval()
            .expect("llDetectedLinkNumber failed");
        assert_eq!(link, 2);

        let rot: LuaTable = lua
            .load("return llDetectedRot(0)")
            .eval()
            .expect("llDetectedRot failed");
        let rz: f64 = rot.get("z").unwrap();
        let rs: f64 = rot.get("s").unwrap();
        assert!((rz - 0.707).abs() < 0.001);
        assert!((rs - 0.707).abs() < 0.001);

        let vel: LuaTable = lua
            .load("return llDetectedVel(0)")
            .eval()
            .expect("llDetectedVel failed");
        let vx: f64 = vel.get("x").unwrap();
        let vy: f64 = vel.get("y").unwrap();
        let vz: f64 = vel.get("z").unwrap();
        assert!((vx - 1.0).abs() < f64::EPSILON);
        assert!((vy - 2.0).abs() < f64::EPSILON);
        assert!((vz - 3.0).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // 11. Context is cleared even if handler errors
    // ---------------------------------------------------------------
    #[test]
    fn context_cleared_on_error() {
        let lua = Lua::new();
        let ctx = Arc::new(RwLock::new(EventContext::default()));
        register_detected_functions(&lua, Arc::clone(&ctx)).expect("register failed");

        lua.load(
            r#"
            function bad_handler(n)
                error("intentional error")
            end
        "#,
        )
        .exec()
        .expect("handler definition failed");

        let info = make_test_info();
        let result = dispatch_event(&lua, &ctx, "bad_handler", vec![info]);
        assert!(result.is_err(), "should propagate handler error");

        let guard = ctx.read();
        assert!(
            guard.detected.is_empty(),
            "context must be cleared even after error"
        );
    }
}
