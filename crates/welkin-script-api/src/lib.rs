// SPDX-License-Identifier: LGPL-3.0-or-later
//! LSL built-in function, constant, and event definitions for the OpenSimulator
//! script engine.
//!
//! These tables are the authoritative registry of LSL built-in symbols. The
//! script compiler uses them for type-checking and the VM uses them to register
//! host functions.

// Re-export the compiler's type system for downstream users.
pub use welkin_script_compiler::{LslType, LslValue};

use std::fmt;

// ---------------------------------------------------------------------------
// Parameter type enum (used in function/event signatures in const context)
// ---------------------------------------------------------------------------

/// LSL type tag used in function and event parameter declarations.
///
/// This is intentionally a simple Copy enum so it can appear in static arrays.
/// For the full type system see [`LslType`] in `welkin-script-compiler`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LslParamType {
    Void,
    Integer,
    Float,
    String,
    Key,
    Vector,
    Rotation,
    List,
}

impl fmt::Display for LslParamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Void => write!(f, "void"),
            Self::Integer => write!(f, "integer"),
            Self::Float => write!(f, "float"),
            Self::String => write!(f, "string"),
            Self::Key => write!(f, "key"),
            Self::Vector => write!(f, "vector"),
            Self::Rotation => write!(f, "rotation"),
            Self::List => write!(f, "list"),
        }
    }
}

// ---------------------------------------------------------------------------
// Function definition
// ---------------------------------------------------------------------------

/// A named parameter in an LSL function or event signature.
#[derive(Debug, Clone, Copy)]
pub struct LslParam {
    pub name: &'static str,
    pub param_type: LslParamType,
}

/// Definition of a single LSL built-in function.
#[derive(Debug, Clone)]
pub struct LslFunction {
    /// Function name (e.g. "llSay").
    pub name: &'static str,
    /// Return type.
    pub return_type: LslParamType,
    /// Parameter list.
    pub params: &'static [LslParam],
    /// Short documentation string.
    pub doc: &'static str,
}

// ---------------------------------------------------------------------------
// Constant definition
// ---------------------------------------------------------------------------

/// The value of an LSL constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LslConstantValue {
    Integer(i32),
    Float(f64),
    String(&'static str),
}

/// Definition of a single LSL built-in constant.
#[derive(Debug, Clone, Copy)]
pub struct LslConstant {
    pub name: &'static str,
    pub value: LslConstantValue,
}

// ---------------------------------------------------------------------------
// Event definition
// ---------------------------------------------------------------------------

/// Definition of a single LSL event handler.
#[derive(Debug, Clone)]
pub struct LslEvent {
    /// Event name (e.g. "state_entry").
    pub name: &'static str,
    /// Parameter list.
    pub params: &'static [LslParam],
    /// Short documentation string.
    pub doc: &'static str,
}

// ---------------------------------------------------------------------------
// Convenience macros for table construction
// ---------------------------------------------------------------------------

macro_rules! params {
    () => { &[] as &[LslParam] };
    ($($name:ident : $ty:ident),+ $(,)?) => {
        &[$(LslParam { name: stringify!($name), param_type: LslParamType::$ty }),+]
    };
}

// ===================================================================
// FUNCTION TABLE -- 100 most important ll* functions by category
// ===================================================================

/// The 100 most important LSL built-in functions, organized by category.
///
/// Sourced from OpenSimulator's `LSL_Api.cs` implementation. Additional
/// functions can be added as the script engine matures.
pub static FUNCTIONS: &[LslFunction] = &[
    // ----- Math -----
    LslFunction {
        name: "llSin",
        return_type: LslParamType::Float,
        params: params!(f: Float),
        doc: "Returns the sine of f (radians)",
    },
    LslFunction {
        name: "llCos",
        return_type: LslParamType::Float,
        params: params!(f: Float),
        doc: "Returns the cosine of f (radians)",
    },
    LslFunction {
        name: "llTan",
        return_type: LslParamType::Float,
        params: params!(f: Float),
        doc: "Returns the tangent of f (radians)",
    },
    LslFunction {
        name: "llAtan2",
        return_type: LslParamType::Float,
        params: params!(y: Float, x: Float),
        doc: "Returns the arc tangent of y/x",
    },
    LslFunction {
        name: "llSqrt",
        return_type: LslParamType::Float,
        params: params!(f: Float),
        doc: "Returns the square root of f",
    },
    LslFunction {
        name: "llPow",
        return_type: LslParamType::Float,
        params: params!(base: Float, exp: Float),
        doc: "Returns base raised to the power exp",
    },
    LslFunction {
        name: "llAbs",
        return_type: LslParamType::Integer,
        params: params!(i: Integer),
        doc: "Returns the absolute value of i",
    },
    LslFunction {
        name: "llFabs",
        return_type: LslParamType::Float,
        params: params!(f: Float),
        doc: "Returns the absolute value of f",
    },
    LslFunction {
        name: "llFrand",
        return_type: LslParamType::Float,
        params: params!(mag: Float),
        doc: "Returns a random float in [0, mag)",
    },
    LslFunction {
        name: "llFloor",
        return_type: LslParamType::Integer,
        params: params!(f: Float),
        doc: "Returns the largest integer not greater than f",
    },
    LslFunction {
        name: "llCeil",
        return_type: LslParamType::Integer,
        params: params!(f: Float),
        doc: "Returns the smallest integer not less than f",
    },
    LslFunction {
        name: "llRound",
        return_type: LslParamType::Integer,
        params: params!(f: Float),
        doc: "Returns f rounded to the nearest integer",
    },
    LslFunction {
        name: "llLog",
        return_type: LslParamType::Float,
        params: params!(f: Float),
        doc: "Returns the natural logarithm of f",
    },
    LslFunction {
        name: "llLog10",
        return_type: LslParamType::Float,
        params: params!(f: Float),
        doc: "Returns the base-10 logarithm of f",
    },
    LslFunction {
        name: "llModPow",
        return_type: LslParamType::Integer,
        params: params!(a: Integer, b: Integer, c: Integer),
        doc: "Returns (a^b) mod c",
    },
    // ----- Vector / Rotation math -----
    LslFunction {
        name: "llVecMag",
        return_type: LslParamType::Float,
        params: params!(v: Vector),
        doc: "Returns the magnitude of v",
    },
    LslFunction {
        name: "llVecNorm",
        return_type: LslParamType::Vector,
        params: params!(v: Vector),
        doc: "Returns the unit vector of v",
    },
    LslFunction {
        name: "llVecDist",
        return_type: LslParamType::Float,
        params: params!(a: Vector, b: Vector),
        doc: "Returns the distance between a and b",
    },
    LslFunction {
        name: "llRot2Euler",
        return_type: LslParamType::Vector,
        params: params!(q: Rotation),
        doc: "Returns Euler angles (radians) from rotation q",
    },
    LslFunction {
        name: "llEuler2Rot",
        return_type: LslParamType::Rotation,
        params: params!(v: Vector),
        doc: "Returns rotation from Euler angles v (radians)",
    },
    LslFunction {
        name: "llAxes2Rot",
        return_type: LslParamType::Rotation,
        params: params!(fwd: Vector, left: Vector, up: Vector),
        doc: "Returns rotation from orthogonal axes",
    },
    LslFunction {
        name: "llRot2Fwd",
        return_type: LslParamType::Vector,
        params: params!(r: Rotation),
        doc: "Returns the forward vector from rotation r",
    },
    LslFunction {
        name: "llRot2Left",
        return_type: LslParamType::Vector,
        params: params!(r: Rotation),
        doc: "Returns the left vector from rotation r",
    },
    LslFunction {
        name: "llRot2Up",
        return_type: LslParamType::Vector,
        params: params!(r: Rotation),
        doc: "Returns the up vector from rotation r",
    },
    LslFunction {
        name: "llRotBetween",
        return_type: LslParamType::Rotation,
        params: params!(a: Vector, b: Vector),
        doc: "Returns the rotation to rotate a to b",
    },
    LslFunction {
        name: "llAngleBetween",
        return_type: LslParamType::Float,
        params: params!(a: Rotation, b: Rotation),
        doc: "Returns the angle between rotations a and b",
    },
    // ----- Communication -----
    LslFunction {
        name: "llWhisper",
        return_type: LslParamType::Void,
        params: params!(channel: Integer, msg: String),
        doc: "Whispers msg on channel (10m range)",
    },
    LslFunction {
        name: "llSay",
        return_type: LslParamType::Void,
        params: params!(channel: Integer, msg: String),
        doc: "Says msg on channel (20m range)",
    },
    LslFunction {
        name: "llShout",
        return_type: LslParamType::Void,
        params: params!(channel: Integer, msg: String),
        doc: "Shouts msg on channel (100m range)",
    },
    LslFunction {
        name: "llRegionSay",
        return_type: LslParamType::Void,
        params: params!(channel: Integer, msg: String),
        doc: "Says msg on channel to entire region",
    },
    LslFunction {
        name: "llRegionSayTo",
        return_type: LslParamType::Void,
        params: params!(target: Key, channel: Integer, msg: String),
        doc: "Says msg on channel to specific target",
    },
    LslFunction {
        name: "llListen",
        return_type: LslParamType::Integer,
        params: params!(channel: Integer, name: String, id: Key, msg: String),
        doc: "Registers a listen event filter; returns handle",
    },
    LslFunction {
        name: "llListenControl",
        return_type: LslParamType::Void,
        params: params!(handle: Integer, active: Integer),
        doc: "Enables or disables a listen handle",
    },
    LslFunction {
        name: "llListenRemove",
        return_type: LslParamType::Void,
        params: params!(handle: Integer),
        doc: "Removes a listen handle",
    },
    LslFunction {
        name: "llInstantMessage",
        return_type: LslParamType::Void,
        params: params!(user: Key, msg: String),
        doc: "Sends an instant message to user",
    },
    LslFunction {
        name: "llOwnerSay",
        return_type: LslParamType::Void,
        params: params!(msg: String),
        doc: "Says msg to object owner only",
    },
    LslFunction {
        name: "llDialog",
        return_type: LslParamType::Void,
        params: params!(agent: Key, msg: String, buttons: List, channel: Integer),
        doc: "Shows a dialog to agent with buttons",
    },
    // ----- Object position / rotation / physics -----
    LslFunction {
        name: "llGetPos",
        return_type: LslParamType::Vector,
        params: params!(),
        doc: "Returns the object's region position",
    },
    LslFunction {
        name: "llSetPos",
        return_type: LslParamType::Void,
        params: params!(pos: Vector),
        doc: "Sets the object's position",
    },
    LslFunction {
        name: "llGetLocalPos",
        return_type: LslParamType::Vector,
        params: params!(),
        doc: "Returns the prim's local position",
    },
    LslFunction {
        name: "llSetRegionPos",
        return_type: LslParamType::Integer,
        params: params!(pos: Vector),
        doc: "Sets position anywhere in region; returns success",
    },
    LslFunction {
        name: "llGetRot",
        return_type: LslParamType::Rotation,
        params: params!(),
        doc: "Returns the object's rotation",
    },
    LslFunction {
        name: "llSetRot",
        return_type: LslParamType::Void,
        params: params!(rot: Rotation),
        doc: "Sets the object's rotation",
    },
    LslFunction {
        name: "llGetLocalRot",
        return_type: LslParamType::Rotation,
        params: params!(),
        doc: "Returns the prim's local rotation",
    },
    LslFunction {
        name: "llSetLocalRot",
        return_type: LslParamType::Void,
        params: params!(rot: Rotation),
        doc: "Sets the prim's local rotation",
    },
    LslFunction {
        name: "llGetScale",
        return_type: LslParamType::Vector,
        params: params!(),
        doc: "Returns the prim's scale",
    },
    LslFunction {
        name: "llSetScale",
        return_type: LslParamType::Void,
        params: params!(scale: Vector),
        doc: "Sets the prim's scale",
    },
    LslFunction {
        name: "llGetVel",
        return_type: LslParamType::Vector,
        params: params!(),
        doc: "Returns the object's velocity",
    },
    LslFunction {
        name: "llGetAccel",
        return_type: LslParamType::Vector,
        params: params!(),
        doc: "Returns the object's acceleration",
    },
    LslFunction {
        name: "llGetOmega",
        return_type: LslParamType::Vector,
        params: params!(),
        doc: "Returns the object's angular velocity",
    },
    LslFunction {
        name: "llSetStatus",
        return_type: LslParamType::Void,
        params: params!(status: Integer, value: Integer),
        doc: "Sets object status flags (physics, phantom, etc.)",
    },
    LslFunction {
        name: "llGetStatus",
        return_type: LslParamType::Integer,
        params: params!(status: Integer),
        doc: "Returns the value of an object status flag",
    },
    // ----- Physics forces -----
    LslFunction {
        name: "llSetForce",
        return_type: LslParamType::Void,
        params: params!(force: Vector, local: Integer),
        doc: "Sets a constant force on the object",
    },
    LslFunction {
        name: "llGetForce",
        return_type: LslParamType::Vector,
        params: params!(),
        doc: "Returns the force acting on the object",
    },
    LslFunction {
        name: "llSetVelocity",
        return_type: LslParamType::Void,
        params: params!(vel: Vector, local: Integer),
        doc: "Sets the object's velocity",
    },
    LslFunction {
        name: "llApplyImpulse",
        return_type: LslParamType::Void,
        params: params!(force: Vector, local: Integer),
        doc: "Applies an impulse to the object",
    },
    LslFunction {
        name: "llSetBuoyancy",
        return_type: LslParamType::Void,
        params: params!(buoyancy: Float),
        doc: "Sets the object's buoyancy (0.0=none, 1.0=neutral)",
    },
    LslFunction {
        name: "llMoveToTarget",
        return_type: LslParamType::Void,
        params: params!(target: Vector, tau: Float),
        doc: "Moves to target over tau seconds",
    },
    LslFunction {
        name: "llStopMoveToTarget",
        return_type: LslParamType::Void,
        params: params!(),
        doc: "Stops move-to-target motion",
    },
    LslFunction {
        name: "llGetMass",
        return_type: LslParamType::Float,
        params: params!(),
        doc: "Returns the object's mass",
    },
    LslFunction {
        name: "llTargetOmega",
        return_type: LslParamType::Void,
        params: params!(axis: Vector, spinrate: Float, gain: Float),
        doc: "Sets spin around axis",
    },
    // ----- Appearance -----
    LslFunction {
        name: "llSetColor",
        return_type: LslParamType::Void,
        params: params!(color: Vector, face: Integer),
        doc: "Sets the color of a face",
    },
    LslFunction {
        name: "llGetColor",
        return_type: LslParamType::Vector,
        params: params!(face: Integer),
        doc: "Returns the color of a face",
    },
    LslFunction {
        name: "llSetAlpha",
        return_type: LslParamType::Void,
        params: params!(alpha: Float, face: Integer),
        doc: "Sets the alpha (transparency) of a face",
    },
    LslFunction {
        name: "llGetAlpha",
        return_type: LslParamType::Float,
        params: params!(face: Integer),
        doc: "Returns the alpha of a face",
    },
    LslFunction {
        name: "llSetTexture",
        return_type: LslParamType::Void,
        params: params!(texture: String, face: Integer),
        doc: "Sets the texture on a face",
    },
    LslFunction {
        name: "llGetTexture",
        return_type: LslParamType::String,
        params: params!(face: Integer),
        doc: "Returns the texture name on a face",
    },
    LslFunction {
        name: "llSetText",
        return_type: LslParamType::Void,
        params: params!(text: String, color: Vector, alpha: Float),
        doc: "Sets floating text above the object",
    },
    LslFunction {
        name: "llSetClickAction",
        return_type: LslParamType::Void,
        params: params!(action: Integer),
        doc: "Sets the click action for the prim",
    },
    // ----- String functions -----
    LslFunction {
        name: "llStringLength",
        return_type: LslParamType::Integer,
        params: params!(s: String),
        doc: "Returns the length of string s",
    },
    LslFunction {
        name: "llGetSubString",
        return_type: LslParamType::String,
        params: params!(src: String, start: Integer, end: Integer),
        doc: "Returns a substring",
    },
    LslFunction {
        name: "llDeleteSubString",
        return_type: LslParamType::String,
        params: params!(src: String, start: Integer, end: Integer),
        doc: "Returns src with substring deleted",
    },
    LslFunction {
        name: "llInsertString",
        return_type: LslParamType::String,
        params: params!(dst: String, pos: Integer, src: String),
        doc: "Returns dst with src inserted at pos",
    },
    LslFunction {
        name: "llToUpper",
        return_type: LslParamType::String,
        params: params!(s: String),
        doc: "Returns s in uppercase",
    },
    LslFunction {
        name: "llToLower",
        return_type: LslParamType::String,
        params: params!(s: String),
        doc: "Returns s in lowercase",
    },
    LslFunction {
        name: "llStringTrim",
        return_type: LslParamType::String,
        params: params!(s: String, trim_type: Integer),
        doc: "Trims whitespace from s",
    },
    LslFunction {
        name: "llSubStringIndex",
        return_type: LslParamType::Integer,
        params: params!(source: String, pattern: String),
        doc: "Returns index of pattern in source, or -1",
    },
    LslFunction {
        name: "llSHA1String",
        return_type: LslParamType::String,
        params: params!(s: String),
        doc: "Returns the SHA1 hex digest of s",
    },
    LslFunction {
        name: "llSHA256String",
        return_type: LslParamType::String,
        params: params!(s: String),
        doc: "Returns the SHA256 hex digest of s",
    },
    LslFunction {
        name: "llBase64ToString",
        return_type: LslParamType::String,
        params: params!(s: String),
        doc: "Decodes base64 string to UTF-8",
    },
    LslFunction {
        name: "llStringToBase64",
        return_type: LslParamType::String,
        params: params!(s: String),
        doc: "Encodes UTF-8 string to base64",
    },
    // ----- List functions -----
    LslFunction {
        name: "llGetListLength",
        return_type: LslParamType::Integer,
        params: params!(src: List),
        doc: "Returns the number of elements in src",
    },
    LslFunction {
        name: "llList2Integer",
        return_type: LslParamType::Integer,
        params: params!(src: List, index: Integer),
        doc: "Returns element at index as integer",
    },
    LslFunction {
        name: "llList2Float",
        return_type: LslParamType::Float,
        params: params!(src: List, index: Integer),
        doc: "Returns element at index as float",
    },
    LslFunction {
        name: "llList2String",
        return_type: LslParamType::String,
        params: params!(src: List, index: Integer),
        doc: "Returns element at index as string",
    },
    LslFunction {
        name: "llList2Key",
        return_type: LslParamType::Key,
        params: params!(src: List, index: Integer),
        doc: "Returns element at index as key",
    },
    LslFunction {
        name: "llList2Vector",
        return_type: LslParamType::Vector,
        params: params!(src: List, index: Integer),
        doc: "Returns element at index as vector",
    },
    LslFunction {
        name: "llList2Rot",
        return_type: LslParamType::Rotation,
        params: params!(src: List, index: Integer),
        doc: "Returns element at index as rotation",
    },
    LslFunction {
        name: "llListSort",
        return_type: LslParamType::List,
        params: params!(src: List, stride: Integer, ascending: Integer),
        doc: "Sorts list by stride",
    },
    LslFunction {
        name: "llDeleteSubList",
        return_type: LslParamType::List,
        params: params!(src: List, start: Integer, end: Integer),
        doc: "Returns src with sublist deleted",
    },
    LslFunction {
        name: "llListFindList",
        return_type: LslParamType::Integer,
        params: params!(src: List, test: List),
        doc: "Returns index of test in src, or -1",
    },
    LslFunction {
        name: "llGetListEntryType",
        return_type: LslParamType::Integer,
        params: params!(src: List, index: Integer),
        doc: "Returns the type constant of element at index",
    },
    LslFunction {
        name: "llCSV2List",
        return_type: LslParamType::List,
        params: params!(src: String),
        doc: "Converts CSV string to list",
    },
    LslFunction {
        name: "llList2CSV",
        return_type: LslParamType::String,
        params: params!(src: List),
        doc: "Converts list to CSV string",
    },
    LslFunction {
        name: "llDumpList2String",
        return_type: LslParamType::String,
        params: params!(src: List, separator: String),
        doc: "Joins list elements with separator",
    },
    LslFunction {
        name: "llParseString2List",
        return_type: LslParamType::List,
        params: params!(src: String, separators: List, spacers: List),
        doc: "Splits string into list",
    },
    // ----- Object identity / info -----
    LslFunction {
        name: "llGetKey",
        return_type: LslParamType::Key,
        params: params!(),
        doc: "Returns the prim's UUID",
    },
    LslFunction {
        name: "llGetOwner",
        return_type: LslParamType::Key,
        params: params!(),
        doc: "Returns the owner's UUID",
    },
    LslFunction {
        name: "llGetObjectName",
        return_type: LslParamType::String,
        params: params!(),
        doc: "Returns the prim's name",
    },
    LslFunction {
        name: "llSetObjectName",
        return_type: LslParamType::Void,
        params: params!(name: String),
        doc: "Sets the prim's name",
    },
    LslFunction {
        name: "llGetObjectDesc",
        return_type: LslParamType::String,
        params: params!(),
        doc: "Returns the prim's description",
    },
    LslFunction {
        name: "llSetObjectDesc",
        return_type: LslParamType::Void,
        params: params!(desc: String),
        doc: "Sets the prim's description",
    },
    LslFunction {
        name: "llGetLinkNumber",
        return_type: LslParamType::Integer,
        params: params!(),
        doc: "Returns this prim's link number",
    },
    LslFunction {
        name: "llGetLinkKey",
        return_type: LslParamType::Key,
        params: params!(linknum: Integer),
        doc: "Returns the UUID of a linked prim",
    },
    LslFunction {
        name: "llGetLinkName",
        return_type: LslParamType::String,
        params: params!(linknum: Integer),
        doc: "Returns the name of a linked prim",
    },
    LslFunction {
        name: "llGetNumberOfPrims",
        return_type: LslParamType::Integer,
        params: params!(),
        doc: "Returns number of prims in the link set",
    },
    LslFunction {
        name: "llGetObjectDetails",
        return_type: LslParamType::List,
        params: params!(id: Key, params: List),
        doc: "Returns details about an object",
    },
    // ----- Time / timer -----
    LslFunction {
        name: "llSetTimerEvent",
        return_type: LslParamType::Void,
        params: params!(sec: Float),
        doc: "Sets the timer interval (0.0 disables)",
    },
    LslFunction {
        name: "llSleep",
        return_type: LslParamType::Void,
        params: params!(sec: Float),
        doc: "Pauses the script for sec seconds",
    },
    LslFunction {
        name: "llGetTime",
        return_type: LslParamType::Float,
        params: params!(),
        doc: "Returns seconds since last time reset",
    },
    LslFunction {
        name: "llResetTime",
        return_type: LslParamType::Void,
        params: params!(),
        doc: "Resets the script timer",
    },
    LslFunction {
        name: "llGetUnixTime",
        return_type: LslParamType::Integer,
        params: params!(),
        doc: "Returns UTC unix timestamp",
    },
    LslFunction {
        name: "llGetTimestamp",
        return_type: LslParamType::String,
        params: params!(),
        doc: "Returns UTC timestamp as ISO 8601 string",
    },
    // ----- Script control -----
    LslFunction {
        name: "llResetScript",
        return_type: LslParamType::Void,
        params: params!(),
        doc: "Resets the script",
    },
    LslFunction {
        name: "llDie",
        return_type: LslParamType::Void,
        params: params!(),
        doc: "Deletes the object",
    },
    LslFunction {
        name: "llRequestPermissions",
        return_type: LslParamType::Void,
        params: params!(agent: Key, perm: Integer),
        doc: "Requests permissions from agent",
    },
    LslFunction {
        name: "llGetPermissions",
        return_type: LslParamType::Integer,
        params: params!(),
        doc: "Returns current permissions bit field",
    },
    LslFunction {
        name: "llGetPermissionsKey",
        return_type: LslParamType::Key,
        params: params!(),
        doc: "Returns UUID of agent who granted permissions",
    },
    // ----- Sensor -----
    LslFunction {
        name: "llSensor",
        return_type: LslParamType::Void,
        params: params!(name: String, id: Key, sensor_type: Integer, range: Float, arc: Float),
        doc: "Performs a single sensor sweep",
    },
    LslFunction {
        name: "llSensorRepeat",
        return_type: LslParamType::Void,
        params: params!(name: String, id: Key, sensor_type: Integer, range: Float, arc: Float, rate: Float),
        doc: "Starts a repeating sensor",
    },
    LslFunction {
        name: "llSensorRemove",
        return_type: LslParamType::Void,
        params: params!(),
        doc: "Removes active sensor",
    },
    // ----- Detected event info -----
    LslFunction {
        name: "llDetectedName",
        return_type: LslParamType::String,
        params: params!(number: Integer),
        doc: "Returns name of detected object/agent",
    },
    LslFunction {
        name: "llDetectedKey",
        return_type: LslParamType::Key,
        params: params!(number: Integer),
        doc: "Returns UUID of detected object/agent",
    },
    LslFunction {
        name: "llDetectedOwner",
        return_type: LslParamType::Key,
        params: params!(number: Integer),
        doc: "Returns owner UUID of detected object",
    },
    LslFunction {
        name: "llDetectedPos",
        return_type: LslParamType::Vector,
        params: params!(number: Integer),
        doc: "Returns position of detected object",
    },
    LslFunction {
        name: "llDetectedType",
        return_type: LslParamType::Integer,
        params: params!(number: Integer),
        doc: "Returns type flags of detected object",
    },
    // ----- Rez / inventory -----
    LslFunction {
        name: "llRezObject",
        return_type: LslParamType::Void,
        params: params!(inventory: String, pos: Vector, vel: Vector, rot: Rotation, param: Integer),
        doc: "Rezzes an object from inventory",
    },
    LslFunction {
        name: "llGiveInventory",
        return_type: LslParamType::Void,
        params: params!(destination: Key, inventory: String),
        doc: "Gives inventory item to destination",
    },
    LslFunction {
        name: "llGetInventoryNumber",
        return_type: LslParamType::Integer,
        params: params!(inv_type: Integer),
        doc: "Returns count of inventory items by type",
    },
    LslFunction {
        name: "llGetInventoryName",
        return_type: LslParamType::String,
        params: params!(inv_type: Integer, number: Integer),
        doc: "Returns name of inventory item by type and index",
    },
    // ----- Link messages -----
    LslFunction {
        name: "llMessageLinked",
        return_type: LslParamType::Void,
        params: params!(linknum: Integer, num: Integer, str: String, id: Key),
        doc: "Sends a link message",
    },
];

// ===================================================================
// CONSTANT TABLE
// ===================================================================

/// Common LSL built-in constants.
///
/// Sourced from OpenSimulator's `LSL_Constants.cs`.
pub static CONSTANTS: &[LslConstant] = &[
    // Boolean
    LslConstant {
        name: "TRUE",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "FALSE",
        value: LslConstantValue::Integer(0),
    },
    // Math
    LslConstant {
        name: "PI",
        value: LslConstantValue::Float(3.14159274),
    },
    LslConstant {
        name: "TWO_PI",
        value: LslConstantValue::Float(6.28318548),
    },
    LslConstant {
        name: "PI_BY_TWO",
        value: LslConstantValue::Float(1.57079637),
    },
    LslConstant {
        name: "DEG_TO_RAD",
        value: LslConstantValue::Float(0.01745329238),
    },
    LslConstant {
        name: "RAD_TO_DEG",
        value: LslConstantValue::Float(57.29578),
    },
    LslConstant {
        name: "SQRT2",
        value: LslConstantValue::Float(1.414213538),
    },
    // Special values
    LslConstant {
        name: "NULL_KEY",
        value: LslConstantValue::String("00000000-0000-0000-0000-000000000000"),
    },
    LslConstant {
        name: "EOF",
        value: LslConstantValue::String("\n\n\n"),
    },
    LslConstant {
        name: "ALL_SIDES",
        value: LslConstantValue::Integer(-1),
    },
    // Status flags
    LslConstant {
        name: "STATUS_PHYSICS",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "STATUS_ROTATE_X",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "STATUS_ROTATE_Y",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "STATUS_ROTATE_Z",
        value: LslConstantValue::Integer(8),
    },
    LslConstant {
        name: "STATUS_PHANTOM",
        value: LslConstantValue::Integer(16),
    },
    LslConstant {
        name: "STATUS_SANDBOX",
        value: LslConstantValue::Integer(32),
    },
    LslConstant {
        name: "STATUS_BLOCK_GRAB",
        value: LslConstantValue::Integer(64),
    },
    LslConstant {
        name: "STATUS_DIE_AT_EDGE",
        value: LslConstantValue::Integer(128),
    },
    LslConstant {
        name: "STATUS_RETURN_AT_EDGE",
        value: LslConstantValue::Integer(256),
    },
    LslConstant {
        name: "STATUS_CAST_SHADOWS",
        value: LslConstantValue::Integer(512),
    },
    LslConstant {
        name: "STATUS_BLOCK_GRAB_OBJECT",
        value: LslConstantValue::Integer(1024),
    },
    // Agent / sensor type
    LslConstant {
        name: "AGENT",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "AGENT_BY_LEGACY_NAME",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "AGENT_BY_USERNAME",
        value: LslConstantValue::Integer(0x10),
    },
    LslConstant {
        name: "NPC",
        value: LslConstantValue::Integer(0x20),
    },
    LslConstant {
        name: "ACTIVE",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "PASSIVE",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "SCRIPTED",
        value: LslConstantValue::Integer(8),
    },
    // Control flags
    LslConstant {
        name: "CONTROL_FWD",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "CONTROL_BACK",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "CONTROL_LEFT",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "CONTROL_RIGHT",
        value: LslConstantValue::Integer(8),
    },
    LslConstant {
        name: "CONTROL_UP",
        value: LslConstantValue::Integer(16),
    },
    LslConstant {
        name: "CONTROL_DOWN",
        value: LslConstantValue::Integer(32),
    },
    LslConstant {
        name: "CONTROL_ROT_LEFT",
        value: LslConstantValue::Integer(256),
    },
    LslConstant {
        name: "CONTROL_ROT_RIGHT",
        value: LslConstantValue::Integer(512),
    },
    LslConstant {
        name: "CONTROL_LBUTTON",
        value: LslConstantValue::Integer(268435456),
    },
    LslConstant {
        name: "CONTROL_ML_LBUTTON",
        value: LslConstantValue::Integer(1073741824),
    },
    // Permissions
    LslConstant {
        name: "PERMISSION_DEBIT",
        value: LslConstantValue::Integer(0x02),
    },
    LslConstant {
        name: "PERMISSION_TAKE_CONTROLS",
        value: LslConstantValue::Integer(0x04),
    },
    LslConstant {
        name: "PERMISSION_TRIGGER_ANIMATION",
        value: LslConstantValue::Integer(0x10),
    },
    LslConstant {
        name: "PERMISSION_ATTACH",
        value: LslConstantValue::Integer(0x20),
    },
    LslConstant {
        name: "PERMISSION_CHANGE_LINKS",
        value: LslConstantValue::Integer(0x80),
    },
    LslConstant {
        name: "PERMISSION_TRACK_CAMERA",
        value: LslConstantValue::Integer(0x400),
    },
    LslConstant {
        name: "PERMISSION_CONTROL_CAMERA",
        value: LslConstantValue::Integer(0x800),
    },
    LslConstant {
        name: "PERMISSION_TELEPORT",
        value: LslConstantValue::Integer(0x1000),
    },
    LslConstant {
        name: "PERMISSION_OVERRIDE_ANIMATIONS",
        value: LslConstantValue::Integer(0x8000),
    },
    LslConstant {
        name: "PERMISSION_RETURN_OBJECTS",
        value: LslConstantValue::Integer(0x10000),
    },
    // Agent info flags
    LslConstant {
        name: "AGENT_FLYING",
        value: LslConstantValue::Integer(0x1),
    },
    LslConstant {
        name: "AGENT_ATTACHMENTS",
        value: LslConstantValue::Integer(0x2),
    },
    LslConstant {
        name: "AGENT_SCRIPTED",
        value: LslConstantValue::Integer(0x4),
    },
    LslConstant {
        name: "AGENT_MOUSELOOK",
        value: LslConstantValue::Integer(0x8),
    },
    LslConstant {
        name: "AGENT_SITTING",
        value: LslConstantValue::Integer(0x10),
    },
    LslConstant {
        name: "AGENT_ON_OBJECT",
        value: LslConstantValue::Integer(0x20),
    },
    LslConstant {
        name: "AGENT_AWAY",
        value: LslConstantValue::Integer(0x40),
    },
    LslConstant {
        name: "AGENT_WALKING",
        value: LslConstantValue::Integer(0x80),
    },
    LslConstant {
        name: "AGENT_IN_AIR",
        value: LslConstantValue::Integer(0x100),
    },
    LslConstant {
        name: "AGENT_TYPING",
        value: LslConstantValue::Integer(0x200),
    },
    LslConstant {
        name: "AGENT_CROUCHING",
        value: LslConstantValue::Integer(0x400),
    },
    LslConstant {
        name: "AGENT_BUSY",
        value: LslConstantValue::Integer(0x800),
    },
    LslConstant {
        name: "AGENT_ALWAYS_RUN",
        value: LslConstantValue::Integer(0x1000),
    },
    LslConstant {
        name: "AGENT_MALE",
        value: LslConstantValue::Integer(0x40000000),
    },
    // Changed event flags
    LslConstant {
        name: "CHANGED_INVENTORY",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "CHANGED_COLOR",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "CHANGED_SHAPE",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "CHANGED_SCALE",
        value: LslConstantValue::Integer(8),
    },
    LslConstant {
        name: "CHANGED_TEXTURE",
        value: LslConstantValue::Integer(16),
    },
    LslConstant {
        name: "CHANGED_LINK",
        value: LslConstantValue::Integer(32),
    },
    LslConstant {
        name: "CHANGED_ALLOWED_DROP",
        value: LslConstantValue::Integer(64),
    },
    LslConstant {
        name: "CHANGED_OWNER",
        value: LslConstantValue::Integer(128),
    },
    LslConstant {
        name: "CHANGED_REGION",
        value: LslConstantValue::Integer(256),
    },
    LslConstant {
        name: "CHANGED_TELEPORT",
        value: LslConstantValue::Integer(512),
    },
    LslConstant {
        name: "CHANGED_REGION_START",
        value: LslConstantValue::Integer(1024),
    },
    LslConstant {
        name: "CHANGED_MEDIA",
        value: LslConstantValue::Integer(2048),
    },
    // Type constants
    LslConstant {
        name: "TYPE_INTEGER",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "TYPE_FLOAT",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "TYPE_STRING",
        value: LslConstantValue::Integer(3),
    },
    LslConstant {
        name: "TYPE_KEY",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "TYPE_VECTOR",
        value: LslConstantValue::Integer(5),
    },
    LslConstant {
        name: "TYPE_ROTATION",
        value: LslConstantValue::Integer(6),
    },
    LslConstant {
        name: "TYPE_INVALID",
        value: LslConstantValue::Integer(0),
    },
    // Link constants
    LslConstant {
        name: "LINK_SET",
        value: LslConstantValue::Integer(-1),
    },
    LslConstant {
        name: "LINK_ROOT",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "LINK_ALL_OTHERS",
        value: LslConstantValue::Integer(-2),
    },
    LslConstant {
        name: "LINK_ALL_CHILDREN",
        value: LslConstantValue::Integer(-3),
    },
    LslConstant {
        name: "LINK_THIS",
        value: LslConstantValue::Integer(-4),
    },
    // Channels
    LslConstant {
        name: "DEBUG_CHANNEL",
        value: LslConstantValue::Integer(0x7FFFFFFF),
    },
    LslConstant {
        name: "PUBLIC_CHANNEL",
        value: LslConstantValue::Integer(0),
    },
    // Inventory types
    LslConstant {
        name: "INVENTORY_ALL",
        value: LslConstantValue::Integer(-1),
    },
    LslConstant {
        name: "INVENTORY_NONE",
        value: LslConstantValue::Integer(-1),
    },
    LslConstant {
        name: "INVENTORY_TEXTURE",
        value: LslConstantValue::Integer(0),
    },
    LslConstant {
        name: "INVENTORY_SOUND",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "INVENTORY_LANDMARK",
        value: LslConstantValue::Integer(3),
    },
    LslConstant {
        name: "INVENTORY_CLOTHING",
        value: LslConstantValue::Integer(5),
    },
    LslConstant {
        name: "INVENTORY_OBJECT",
        value: LslConstantValue::Integer(6),
    },
    LslConstant {
        name: "INVENTORY_NOTECARD",
        value: LslConstantValue::Integer(7),
    },
    LslConstant {
        name: "INVENTORY_SCRIPT",
        value: LslConstantValue::Integer(10),
    },
    LslConstant {
        name: "INVENTORY_BODYPART",
        value: LslConstantValue::Integer(13),
    },
    LslConstant {
        name: "INVENTORY_ANIMATION",
        value: LslConstantValue::Integer(20),
    },
    LslConstant {
        name: "INVENTORY_GESTURE",
        value: LslConstantValue::Integer(21),
    },
    // Permissions mask
    LslConstant {
        name: "MASK_BASE",
        value: LslConstantValue::Integer(0),
    },
    LslConstant {
        name: "MASK_OWNER",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "MASK_GROUP",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "MASK_EVERYONE",
        value: LslConstantValue::Integer(3),
    },
    LslConstant {
        name: "MASK_NEXT",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "PERM_TRANSFER",
        value: LslConstantValue::Integer(0x2000),
    },
    LslConstant {
        name: "PERM_MODIFY",
        value: LslConstantValue::Integer(0x4000),
    },
    LslConstant {
        name: "PERM_COPY",
        value: LslConstantValue::Integer(0x8000),
    },
    LslConstant {
        name: "PERM_MOVE",
        value: LslConstantValue::Integer(0x80000),
    },
    LslConstant {
        name: "PERM_ALL",
        value: LslConstantValue::Integer(0x7FFFFFFF),
    },
    // Vehicle types
    LslConstant {
        name: "VEHICLE_TYPE_NONE",
        value: LslConstantValue::Integer(0),
    },
    LslConstant {
        name: "VEHICLE_TYPE_SLED",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "VEHICLE_TYPE_CAR",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "VEHICLE_TYPE_BOAT",
        value: LslConstantValue::Integer(3),
    },
    LslConstant {
        name: "VEHICLE_TYPE_AIRPLANE",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "VEHICLE_TYPE_BALLOON",
        value: LslConstantValue::Integer(5),
    },
    // String trim
    LslConstant {
        name: "STRING_TRIM_HEAD",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "STRING_TRIM_TAIL",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "STRING_TRIM",
        value: LslConstantValue::Integer(3),
    },
    // Data server
    LslConstant {
        name: "DATA_ONLINE",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "DATA_NAME",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "DATA_BORN",
        value: LslConstantValue::Integer(3),
    },
    LslConstant {
        name: "DATA_RATING",
        value: LslConstantValue::Integer(4),
    },
    // Prim types
    LslConstant {
        name: "PRIM_TYPE_BOX",
        value: LslConstantValue::Integer(0),
    },
    LslConstant {
        name: "PRIM_TYPE_CYLINDER",
        value: LslConstantValue::Integer(1),
    },
    LslConstant {
        name: "PRIM_TYPE_PRISM",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "PRIM_TYPE_SPHERE",
        value: LslConstantValue::Integer(3),
    },
    LslConstant {
        name: "PRIM_TYPE_TORUS",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "PRIM_TYPE_TUBE",
        value: LslConstantValue::Integer(5),
    },
    LslConstant {
        name: "PRIM_TYPE_RING",
        value: LslConstantValue::Integer(6),
    },
    LslConstant {
        name: "PRIM_TYPE_SCULPT",
        value: LslConstantValue::Integer(7),
    },
    // Prim params
    LslConstant {
        name: "PRIM_MATERIAL",
        value: LslConstantValue::Integer(2),
    },
    LslConstant {
        name: "PRIM_PHYSICS",
        value: LslConstantValue::Integer(3),
    },
    LslConstant {
        name: "PRIM_TEMP_ON_REZ",
        value: LslConstantValue::Integer(4),
    },
    LslConstant {
        name: "PRIM_PHANTOM",
        value: LslConstantValue::Integer(5),
    },
    LslConstant {
        name: "PRIM_POSITION",
        value: LslConstantValue::Integer(6),
    },
    LslConstant {
        name: "PRIM_SIZE",
        value: LslConstantValue::Integer(7),
    },
    LslConstant {
        name: "PRIM_ROTATION",
        value: LslConstantValue::Integer(8),
    },
    LslConstant {
        name: "PRIM_TYPE",
        value: LslConstantValue::Integer(9),
    },
    LslConstant {
        name: "PRIM_TEXTURE",
        value: LslConstantValue::Integer(17),
    },
    LslConstant {
        name: "PRIM_COLOR",
        value: LslConstantValue::Integer(18),
    },
    LslConstant {
        name: "PRIM_FULLBRIGHT",
        value: LslConstantValue::Integer(20),
    },
    LslConstant {
        name: "PRIM_FLEXIBLE",
        value: LslConstantValue::Integer(21),
    },
    LslConstant {
        name: "PRIM_GLOW",
        value: LslConstantValue::Integer(25),
    },
    LslConstant {
        name: "PRIM_TEXT",
        value: LslConstantValue::Integer(26),
    },
    LslConstant {
        name: "PRIM_NAME",
        value: LslConstantValue::Integer(27),
    },
    LslConstant {
        name: "PRIM_DESC",
        value: LslConstantValue::Integer(28),
    },
];

// ===================================================================
// EVENT TABLE
// ===================================================================

/// All standard LSL events.
///
/// Sourced from the Second Life LSL specification and OpenSimulator's
/// script engine implementation.
pub static EVENTS: &[LslEvent] = &[
    // State / lifecycle
    LslEvent {
        name: "state_entry",
        params: params!(),
        doc: "Triggered when a state is entered",
    },
    LslEvent {
        name: "state_exit",
        params: params!(),
        doc: "Triggered when a state is exited",
    },
    LslEvent {
        name: "on_rez",
        params: params!(start_param: Integer),
        doc: "Triggered when the object is rezzed",
    },
    LslEvent {
        name: "changed",
        params: params!(change: Integer),
        doc: "Triggered when the object changes",
    },
    LslEvent {
        name: "attach",
        params: params!(id: Key),
        doc: "Triggered on attach/detach",
    },
    // Touch
    LslEvent {
        name: "touch_start",
        params: params!(num_detected: Integer),
        doc: "Triggered when touch begins",
    },
    LslEvent {
        name: "touch",
        params: params!(num_detected: Integer),
        doc: "Triggered while being touched",
    },
    LslEvent {
        name: "touch_end",
        params: params!(num_detected: Integer),
        doc: "Triggered when touch ends",
    },
    // Collision
    LslEvent {
        name: "collision_start",
        params: params!(num_detected: Integer),
        doc: "Triggered when collision begins",
    },
    LslEvent {
        name: "collision",
        params: params!(num_detected: Integer),
        doc: "Triggered while colliding",
    },
    LslEvent {
        name: "collision_end",
        params: params!(num_detected: Integer),
        doc: "Triggered when collision ends",
    },
    LslEvent {
        name: "land_collision_start",
        params: params!(pos: Vector),
        doc: "Triggered when land collision begins",
    },
    LslEvent {
        name: "land_collision",
        params: params!(pos: Vector),
        doc: "Triggered while colliding with land",
    },
    LslEvent {
        name: "land_collision_end",
        params: params!(pos: Vector),
        doc: "Triggered when land collision ends",
    },
    // Communication
    LslEvent {
        name: "listen",
        params: params!(channel: Integer, name: String, id: Key, message: String),
        doc: "Triggered by chat on a listened channel",
    },
    // Timer / sensor
    LslEvent {
        name: "timer",
        params: params!(),
        doc: "Triggered at the timer interval",
    },
    LslEvent {
        name: "sensor",
        params: params!(num_detected: Integer),
        doc: "Triggered when sensor detects objects",
    },
    LslEvent {
        name: "no_sensor",
        params: params!(),
        doc: "Triggered when sensor detects nothing",
    },
    // Money
    LslEvent {
        name: "money",
        params: params!(id: Key, amount: Integer),
        doc: "Triggered when money is paid to object",
    },
    // Control
    LslEvent {
        name: "control",
        params: params!(id: Key, held: Integer, change: Integer),
        doc: "Triggered by agent control input",
    },
    // Link message
    LslEvent {
        name: "link_message",
        params: params!(sender_num: Integer, num: Integer, str: String, id: Key),
        doc: "Triggered by llMessageLinked",
    },
    // Permissions
    LslEvent {
        name: "run_time_permissions",
        params: params!(perm: Integer),
        doc: "Triggered when permissions are granted/denied",
    },
    // Movement
    LslEvent {
        name: "moving_start",
        params: params!(),
        doc: "Triggered when the object starts moving",
    },
    LslEvent {
        name: "moving_end",
        params: params!(),
        doc: "Triggered when the object stops moving",
    },
    LslEvent {
        name: "at_target",
        params: params!(tnum: Integer, targetpos: Vector, ourpos: Vector),
        doc: "Triggered when at position target",
    },
    LslEvent {
        name: "not_at_target",
        params: params!(),
        doc: "Triggered when not at position target",
    },
    LslEvent {
        name: "at_rot_target",
        params: params!(tnum: Integer, targetrot: Rotation, ourrot: Rotation),
        doc: "Triggered when at rotation target",
    },
    LslEvent {
        name: "not_at_rot_target",
        params: params!(),
        doc: "Triggered when not at rotation target",
    },
    // Data
    LslEvent {
        name: "dataserver",
        params: params!(query_id: Key, data: String),
        doc: "Triggered with async data response",
    },
    LslEvent {
        name: "email",
        params: params!(time: String, address: String, subject: String, message: String, num_left: Integer),
        doc: "Triggered when email is received",
    },
    LslEvent {
        name: "http_response",
        params: params!(request_id: Key, status: Integer, metadata: List, body: String),
        doc: "Triggered with HTTP response",
    },
    LslEvent {
        name: "http_request",
        params: params!(request_id: Key, method: String, body: String),
        doc: "Triggered by incoming HTTP request",
    },
    LslEvent {
        name: "remote_data",
        params: params!(event_type: Integer, channel: Key, message_id: Key, sender: String, idata: Integer, sdata: String),
        doc: "Triggered by XML-RPC events",
    },
    LslEvent {
        name: "object_rez",
        params: params!(id: Key),
        doc: "Triggered when object rezzes another object",
    },
    // Experience (stub)
    LslEvent {
        name: "experience_permissions",
        params: params!(agent_id: Key),
        doc: "Triggered when experience permissions granted",
    },
    LslEvent {
        name: "experience_permissions_denied",
        params: params!(agent_id: Key, reason: Integer),
        doc: "Triggered when experience permissions denied",
    },
    LslEvent {
        name: "transaction_result",
        params: params!(id: Key, success: Integer, data: String),
        doc: "Triggered with transaction result",
    },
];

// ---------------------------------------------------------------------------
// Lookup helpers
// ---------------------------------------------------------------------------

/// Finds a function definition by name.
pub fn find_function(name: &str) -> Option<&'static LslFunction> {
    FUNCTIONS.iter().find(|f| f.name == name)
}

/// Finds a constant definition by name.
pub fn find_constant(name: &str) -> Option<&'static LslConstant> {
    CONSTANTS.iter().find(|c| c.name == name)
}

/// Finds an event definition by name.
pub fn find_event(name: &str) -> Option<&'static LslEvent> {
    EVENTS.iter().find(|e| e.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_count() {
        assert!(
            FUNCTIONS.len() >= 100,
            "expected >= 100 functions, got {}",
            FUNCTIONS.len()
        );
    }

    #[test]
    fn constant_count() {
        assert!(
            CONSTANTS.len() >= 100,
            "expected >= 100 constants, got {}",
            CONSTANTS.len()
        );
    }

    #[test]
    fn event_count() {
        assert!(
            EVENTS.len() >= 30,
            "expected >= 30 events, got {}",
            EVENTS.len()
        );
    }

    #[test]
    fn find_ll_say() {
        let f = find_function("llSay").expect("llSay should exist");
        assert_eq!(f.return_type, LslParamType::Void);
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].name, "channel");
        assert_eq!(f.params[0].param_type, LslParamType::Integer);
        assert_eq!(f.params[1].name, "msg");
        assert_eq!(f.params[1].param_type, LslParamType::String);
    }

    #[test]
    fn find_pi_constant() {
        let c = find_constant("PI").expect("PI should exist");
        match c.value {
            LslConstantValue::Float(v) => {
                assert!(
                    (v - std::f64::consts::PI).abs() < 0.01,
                    "PI should be approximately 3.14159"
                );
            }
            _ => panic!("PI should be a float"),
        }
    }

    #[test]
    fn find_state_entry_event() {
        let e = find_event("state_entry").expect("state_entry should exist");
        assert!(e.params.is_empty(), "state_entry has no parameters");
    }

    #[test]
    fn find_listen_event() {
        let e = find_event("listen").expect("listen should exist");
        assert_eq!(e.params.len(), 4);
        assert_eq!(e.params[0].name, "channel");
        assert_eq!(e.params[0].param_type, LslParamType::Integer);
    }

    #[test]
    fn unique_function_names() {
        let mut names: Vec<&str> = FUNCTIONS.iter().map(|f| f.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate function names found");
    }

    #[test]
    fn unique_constant_names() {
        let mut names: Vec<&str> = CONSTANTS.iter().map(|c| c.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate constant names found");
    }

    #[test]
    fn unique_event_names() {
        let mut names: Vec<&str> = EVENTS.iter().map(|e| e.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate event names found");
    }

    #[test]
    fn param_type_display() {
        assert_eq!(LslParamType::Integer.to_string(), "integer");
        assert_eq!(LslParamType::Float.to_string(), "float");
        assert_eq!(LslParamType::String.to_string(), "string");
        assert_eq!(LslParamType::Key.to_string(), "key");
        assert_eq!(LslParamType::Vector.to_string(), "vector");
        assert_eq!(LslParamType::Rotation.to_string(), "rotation");
        assert_eq!(LslParamType::List.to_string(), "list");
        assert_eq!(LslParamType::Void.to_string(), "void");
    }

    #[test]
    fn reexported_types() {
        // Verify re-exports from welkin-script-compiler are accessible.
        let _ = LslType::Integer;
        let v = LslValue::Integer(42);
        assert_eq!(v.lsl_type(), LslType::Integer);
    }
}
