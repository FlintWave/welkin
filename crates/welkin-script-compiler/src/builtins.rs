// SPDX-License-Identifier: LGPL-3.0-or-later
//! LSL built-in function and constant tables for the analyzer.
//!
//! These tables mirror the authoritative definitions in `welkin-script-api`
//! but use [`LslType`] directly, avoiding a circular dependency.

use crate::LslType;

/// A built-in LSL function signature.
pub struct BuiltinFunc {
    pub name: &'static str,
    pub return_type: LslType,
    pub params: &'static [LslType],
}

/// A built-in LSL constant's name and type.
pub struct BuiltinConst {
    pub name: &'static str,
    pub const_type: LslType,
}

macro_rules! params {
    () => { &[] as &[LslType] };
    ($($ty:ident),+ $(,)?) => {
        &[$(LslType::$ty),+]
    };
}

// ===================================================================
// FUNCTION TABLE
// ===================================================================

pub static BUILTIN_FUNCTIONS: &[BuiltinFunc] = &[
    // ----- Math -----
    BuiltinFunc { name: "llSin", return_type: LslType::Float, params: params!(Float) },
    BuiltinFunc { name: "llCos", return_type: LslType::Float, params: params!(Float) },
    BuiltinFunc { name: "llTan", return_type: LslType::Float, params: params!(Float) },
    BuiltinFunc { name: "llAtan2", return_type: LslType::Float, params: params!(Float, Float) },
    BuiltinFunc { name: "llSqrt", return_type: LslType::Float, params: params!(Float) },
    BuiltinFunc { name: "llPow", return_type: LslType::Float, params: params!(Float, Float) },
    BuiltinFunc { name: "llAbs", return_type: LslType::Integer, params: params!(Integer) },
    BuiltinFunc { name: "llFabs", return_type: LslType::Float, params: params!(Float) },
    BuiltinFunc { name: "llFrand", return_type: LslType::Float, params: params!(Float) },
    BuiltinFunc { name: "llFloor", return_type: LslType::Integer, params: params!(Float) },
    BuiltinFunc { name: "llCeil", return_type: LslType::Integer, params: params!(Float) },
    BuiltinFunc { name: "llRound", return_type: LslType::Integer, params: params!(Float) },
    BuiltinFunc { name: "llLog", return_type: LslType::Float, params: params!(Float) },
    BuiltinFunc { name: "llLog10", return_type: LslType::Float, params: params!(Float) },
    BuiltinFunc { name: "llModPow", return_type: LslType::Integer, params: params!(Integer, Integer, Integer) },
    // ----- Vector / Rotation math -----
    BuiltinFunc { name: "llVecMag", return_type: LslType::Float, params: params!(Vector) },
    BuiltinFunc { name: "llVecNorm", return_type: LslType::Vector, params: params!(Vector) },
    BuiltinFunc { name: "llVecDist", return_type: LslType::Float, params: params!(Vector, Vector) },
    BuiltinFunc { name: "llRot2Euler", return_type: LslType::Vector, params: params!(Rotation) },
    BuiltinFunc { name: "llEuler2Rot", return_type: LslType::Rotation, params: params!(Vector) },
    BuiltinFunc { name: "llAxes2Rot", return_type: LslType::Rotation, params: params!(Vector, Vector, Vector) },
    BuiltinFunc { name: "llRot2Fwd", return_type: LslType::Vector, params: params!(Rotation) },
    BuiltinFunc { name: "llRot2Left", return_type: LslType::Vector, params: params!(Rotation) },
    BuiltinFunc { name: "llRot2Up", return_type: LslType::Vector, params: params!(Rotation) },
    BuiltinFunc { name: "llRotBetween", return_type: LslType::Rotation, params: params!(Vector, Vector) },
    BuiltinFunc { name: "llAngleBetween", return_type: LslType::Float, params: params!(Rotation, Rotation) },
    // ----- Communication -----
    BuiltinFunc { name: "llWhisper", return_type: LslType::Void, params: params!(Integer, String) },
    BuiltinFunc { name: "llSay", return_type: LslType::Void, params: params!(Integer, String) },
    BuiltinFunc { name: "llShout", return_type: LslType::Void, params: params!(Integer, String) },
    BuiltinFunc { name: "llRegionSay", return_type: LslType::Void, params: params!(Integer, String) },
    BuiltinFunc { name: "llRegionSayTo", return_type: LslType::Void, params: params!(Key, Integer, String) },
    BuiltinFunc { name: "llListen", return_type: LslType::Integer, params: params!(Integer, String, Key, String) },
    BuiltinFunc { name: "llListenControl", return_type: LslType::Void, params: params!(Integer, Integer) },
    BuiltinFunc { name: "llListenRemove", return_type: LslType::Void, params: params!(Integer) },
    BuiltinFunc { name: "llInstantMessage", return_type: LslType::Void, params: params!(Key, String) },
    BuiltinFunc { name: "llOwnerSay", return_type: LslType::Void, params: params!(String) },
    BuiltinFunc { name: "llDialog", return_type: LslType::Void, params: params!(Key, String, List, Integer) },
    // ----- Object position / rotation / physics -----
    BuiltinFunc { name: "llGetPos", return_type: LslType::Vector, params: params!() },
    BuiltinFunc { name: "llSetPos", return_type: LslType::Void, params: params!(Vector) },
    BuiltinFunc { name: "llGetLocalPos", return_type: LslType::Vector, params: params!() },
    BuiltinFunc { name: "llSetRegionPos", return_type: LslType::Integer, params: params!(Vector) },
    BuiltinFunc { name: "llGetRot", return_type: LslType::Rotation, params: params!() },
    BuiltinFunc { name: "llSetRot", return_type: LslType::Void, params: params!(Rotation) },
    BuiltinFunc { name: "llGetLocalRot", return_type: LslType::Rotation, params: params!() },
    BuiltinFunc { name: "llSetLocalRot", return_type: LslType::Void, params: params!(Rotation) },
    BuiltinFunc { name: "llGetScale", return_type: LslType::Vector, params: params!() },
    BuiltinFunc { name: "llSetScale", return_type: LslType::Void, params: params!(Vector) },
    BuiltinFunc { name: "llGetVel", return_type: LslType::Vector, params: params!() },
    BuiltinFunc { name: "llGetAccel", return_type: LslType::Vector, params: params!() },
    BuiltinFunc { name: "llGetOmega", return_type: LslType::Vector, params: params!() },
    BuiltinFunc { name: "llSetStatus", return_type: LslType::Void, params: params!(Integer, Integer) },
    BuiltinFunc { name: "llGetStatus", return_type: LslType::Integer, params: params!(Integer) },
    // ----- Physics forces -----
    BuiltinFunc { name: "llSetForce", return_type: LslType::Void, params: params!(Vector, Integer) },
    BuiltinFunc { name: "llGetForce", return_type: LslType::Vector, params: params!() },
    BuiltinFunc { name: "llSetVelocity", return_type: LslType::Void, params: params!(Vector, Integer) },
    BuiltinFunc { name: "llApplyImpulse", return_type: LslType::Void, params: params!(Vector, Integer) },
    BuiltinFunc { name: "llSetBuoyancy", return_type: LslType::Void, params: params!(Float) },
    BuiltinFunc { name: "llMoveToTarget", return_type: LslType::Void, params: params!(Vector, Float) },
    BuiltinFunc { name: "llStopMoveToTarget", return_type: LslType::Void, params: params!() },
    BuiltinFunc { name: "llGetMass", return_type: LslType::Float, params: params!() },
    BuiltinFunc { name: "llTargetOmega", return_type: LslType::Void, params: params!(Vector, Float, Float) },
    // ----- Appearance -----
    BuiltinFunc { name: "llSetColor", return_type: LslType::Void, params: params!(Vector, Integer) },
    BuiltinFunc { name: "llGetColor", return_type: LslType::Vector, params: params!(Integer) },
    BuiltinFunc { name: "llSetAlpha", return_type: LslType::Void, params: params!(Float, Integer) },
    BuiltinFunc { name: "llGetAlpha", return_type: LslType::Float, params: params!(Integer) },
    BuiltinFunc { name: "llSetTexture", return_type: LslType::Void, params: params!(String, Integer) },
    BuiltinFunc { name: "llGetTexture", return_type: LslType::String, params: params!(Integer) },
    BuiltinFunc { name: "llSetText", return_type: LslType::Void, params: params!(String, Vector, Float) },
    BuiltinFunc { name: "llSetClickAction", return_type: LslType::Void, params: params!(Integer) },
    // ----- String functions -----
    BuiltinFunc { name: "llStringLength", return_type: LslType::Integer, params: params!(String) },
    BuiltinFunc { name: "llGetSubString", return_type: LslType::String, params: params!(String, Integer, Integer) },
    BuiltinFunc { name: "llDeleteSubString", return_type: LslType::String, params: params!(String, Integer, Integer) },
    BuiltinFunc { name: "llInsertString", return_type: LslType::String, params: params!(String, Integer, String) },
    BuiltinFunc { name: "llToUpper", return_type: LslType::String, params: params!(String) },
    BuiltinFunc { name: "llToLower", return_type: LslType::String, params: params!(String) },
    BuiltinFunc { name: "llStringTrim", return_type: LslType::String, params: params!(String, Integer) },
    BuiltinFunc { name: "llSubStringIndex", return_type: LslType::Integer, params: params!(String, String) },
    BuiltinFunc { name: "llSHA1String", return_type: LslType::String, params: params!(String) },
    BuiltinFunc { name: "llSHA256String", return_type: LslType::String, params: params!(String) },
    BuiltinFunc { name: "llBase64ToString", return_type: LslType::String, params: params!(String) },
    BuiltinFunc { name: "llStringToBase64", return_type: LslType::String, params: params!(String) },
    // ----- List functions -----
    BuiltinFunc { name: "llGetListLength", return_type: LslType::Integer, params: params!(List) },
    BuiltinFunc { name: "llList2Integer", return_type: LslType::Integer, params: params!(List, Integer) },
    BuiltinFunc { name: "llList2Float", return_type: LslType::Float, params: params!(List, Integer) },
    BuiltinFunc { name: "llList2String", return_type: LslType::String, params: params!(List, Integer) },
    BuiltinFunc { name: "llList2Key", return_type: LslType::Key, params: params!(List, Integer) },
    BuiltinFunc { name: "llList2Vector", return_type: LslType::Vector, params: params!(List, Integer) },
    BuiltinFunc { name: "llList2Rot", return_type: LslType::Rotation, params: params!(List, Integer) },
    BuiltinFunc { name: "llListSort", return_type: LslType::List, params: params!(List, Integer, Integer) },
    BuiltinFunc { name: "llDeleteSubList", return_type: LslType::List, params: params!(List, Integer, Integer) },
    BuiltinFunc { name: "llListFindList", return_type: LslType::Integer, params: params!(List, List) },
    BuiltinFunc { name: "llGetListEntryType", return_type: LslType::Integer, params: params!(List, Integer) },
    BuiltinFunc { name: "llCSV2List", return_type: LslType::List, params: params!(String) },
    BuiltinFunc { name: "llList2CSV", return_type: LslType::String, params: params!(List) },
    BuiltinFunc { name: "llDumpList2String", return_type: LslType::String, params: params!(List, String) },
    BuiltinFunc { name: "llParseString2List", return_type: LslType::List, params: params!(String, List, List) },
    // ----- Object identity / info -----
    BuiltinFunc { name: "llGetKey", return_type: LslType::Key, params: params!() },
    BuiltinFunc { name: "llGetOwner", return_type: LslType::Key, params: params!() },
    BuiltinFunc { name: "llGetObjectName", return_type: LslType::String, params: params!() },
    BuiltinFunc { name: "llSetObjectName", return_type: LslType::Void, params: params!(String) },
    BuiltinFunc { name: "llGetObjectDesc", return_type: LslType::String, params: params!() },
    BuiltinFunc { name: "llSetObjectDesc", return_type: LslType::Void, params: params!(String) },
    BuiltinFunc { name: "llGetLinkNumber", return_type: LslType::Integer, params: params!() },
    BuiltinFunc { name: "llGetLinkKey", return_type: LslType::Key, params: params!(Integer) },
    BuiltinFunc { name: "llGetLinkName", return_type: LslType::String, params: params!(Integer) },
    BuiltinFunc { name: "llGetNumberOfPrims", return_type: LslType::Integer, params: params!() },
    BuiltinFunc { name: "llGetObjectDetails", return_type: LslType::List, params: params!(Key, List) },
    // ----- Time / timer -----
    BuiltinFunc { name: "llSetTimerEvent", return_type: LslType::Void, params: params!(Float) },
    BuiltinFunc { name: "llSleep", return_type: LslType::Void, params: params!(Float) },
    BuiltinFunc { name: "llGetTime", return_type: LslType::Float, params: params!() },
    BuiltinFunc { name: "llResetTime", return_type: LslType::Void, params: params!() },
    BuiltinFunc { name: "llGetUnixTime", return_type: LslType::Integer, params: params!() },
    BuiltinFunc { name: "llGetTimestamp", return_type: LslType::String, params: params!() },
    // ----- Script control -----
    BuiltinFunc { name: "llResetScript", return_type: LslType::Void, params: params!() },
    BuiltinFunc { name: "llDie", return_type: LslType::Void, params: params!() },
    BuiltinFunc { name: "llRequestPermissions", return_type: LslType::Void, params: params!(Key, Integer) },
    BuiltinFunc { name: "llGetPermissions", return_type: LslType::Integer, params: params!() },
    BuiltinFunc { name: "llGetPermissionsKey", return_type: LslType::Key, params: params!() },
    // ----- Sensor -----
    BuiltinFunc { name: "llSensor", return_type: LslType::Void, params: params!(String, Key, Integer, Float, Float) },
    BuiltinFunc { name: "llSensorRepeat", return_type: LslType::Void, params: params!(String, Key, Integer, Float, Float, Float) },
    BuiltinFunc { name: "llSensorRemove", return_type: LslType::Void, params: params!() },
    // ----- Detected event info -----
    BuiltinFunc { name: "llDetectedName", return_type: LslType::String, params: params!(Integer) },
    BuiltinFunc { name: "llDetectedKey", return_type: LslType::Key, params: params!(Integer) },
    BuiltinFunc { name: "llDetectedOwner", return_type: LslType::Key, params: params!(Integer) },
    BuiltinFunc { name: "llDetectedPos", return_type: LslType::Vector, params: params!(Integer) },
    BuiltinFunc { name: "llDetectedType", return_type: LslType::Integer, params: params!(Integer) },
    // ----- Rez / inventory -----
    BuiltinFunc { name: "llRezObject", return_type: LslType::Void, params: params!(String, Vector, Vector, Rotation, Integer) },
    BuiltinFunc { name: "llGiveInventory", return_type: LslType::Void, params: params!(Key, String) },
    BuiltinFunc { name: "llGetInventoryNumber", return_type: LslType::Integer, params: params!(Integer) },
    BuiltinFunc { name: "llGetInventoryName", return_type: LslType::String, params: params!(Integer, Integer) },
    // ----- Link messages -----
    BuiltinFunc { name: "llMessageLinked", return_type: LslType::Void, params: params!(Integer, Integer, String, Key) },
    // ----- Vector / Rotation math (additions) -----
    BuiltinFunc { name: "llAxisAngle2Rot", return_type: LslType::Rotation, params: params!(Vector, Float) },
    // ----- Communication (additions) -----
    BuiltinFunc { name: "llEmail", return_type: LslType::Void, params: params!(String, String, String) },
    BuiltinFunc { name: "llLoadURL", return_type: LslType::Void, params: params!(String, String, String) },
    // ----- Sound -----
    BuiltinFunc { name: "llAdjustSoundVolume", return_type: LslType::Void, params: params!(Float) },
    BuiltinFunc { name: "llCollisionSound", return_type: LslType::Void, params: params!(String, Float) },
    BuiltinFunc { name: "llLoopSound", return_type: LslType::Void, params: params!(String, Float) },
    BuiltinFunc { name: "llLoopSoundMaster", return_type: LslType::Void, params: params!(String, Float) },
    BuiltinFunc { name: "llLoopSoundSlave", return_type: LslType::Void, params: params!(String, Float) },
    BuiltinFunc { name: "llPlaySoundSlave", return_type: LslType::Void, params: params!(String, Float) },
    BuiltinFunc { name: "llSetSoundRadius", return_type: LslType::Void, params: params!(Float) },
    BuiltinFunc { name: "llTriggerSound", return_type: LslType::Void, params: params!(String, Float) },
    BuiltinFunc { name: "llTriggerSoundLimited", return_type: LslType::Void, params: params!(String, Float, Vector, Vector) },
    BuiltinFunc { name: "llPlaySound", return_type: LslType::Void, params: params!(String, Float) },
    BuiltinFunc { name: "llPreloadSound", return_type: LslType::Void, params: params!(String) },
    BuiltinFunc { name: "llSetSoundQueueing", return_type: LslType::Void, params: params!(Integer) },
    BuiltinFunc { name: "llStopSound", return_type: LslType::Void, params: params!() },
    // ----- Animation / Attachment -----
    BuiltinFunc { name: "llAttachToAvatar", return_type: LslType::Void, params: params!(Integer) },
    BuiltinFunc { name: "llDetachFromAvatar", return_type: LslType::Void, params: params!() },
    BuiltinFunc { name: "llStartAnimation", return_type: LslType::Void, params: params!(String) },
    BuiltinFunc { name: "llStopAnimation", return_type: LslType::Void, params: params!(String) },
    // ----- Object position / rotation (additions) -----
    BuiltinFunc { name: "llAvatarOnSitTarget", return_type: LslType::Key, params: params!() },
    BuiltinFunc { name: "llGetRootRotation", return_type: LslType::Rotation, params: params!() },
    BuiltinFunc { name: "llRotLookAt", return_type: LslType::Void, params: params!(Rotation, Float, Float) },
    BuiltinFunc { name: "llSitTarget", return_type: LslType::Void, params: params!(Vector, Rotation) },
    BuiltinFunc { name: "llStopLookAt", return_type: LslType::Void, params: params!() },
    BuiltinFunc { name: "llUnSit", return_type: LslType::Void, params: params!(String) },
    // ----- Physics / collision (additions) -----
    BuiltinFunc { name: "llCollisionFilter", return_type: LslType::Void, params: params!(String, Key, Integer) },
    BuiltinFunc { name: "llPassCollisions", return_type: LslType::Void, params: params!(Integer) },
    BuiltinFunc { name: "llVolumeDetect", return_type: LslType::Void, params: params!(Integer) },
    // ----- Vehicle -----
    BuiltinFunc { name: "llRemoveVehicleFlags", return_type: LslType::Void, params: params!(Integer) },
    BuiltinFunc { name: "llSetVehicleFlags", return_type: LslType::Void, params: params!(Integer) },
    BuiltinFunc { name: "llSetVehicleFloatParam", return_type: LslType::Void, params: params!(Integer, Float) },
    BuiltinFunc { name: "llSetVehicleType", return_type: LslType::Void, params: params!(Integer) },
    BuiltinFunc { name: "llSetVehicleVectorParam", return_type: LslType::Void, params: params!(Integer, Vector) },
    // ----- Appearance (additions) -----
    BuiltinFunc { name: "llRotateTexture", return_type: LslType::Void, params: params!(Float, Integer) },
    BuiltinFunc { name: "llSetCameraAtOffset", return_type: LslType::Void, params: params!(Vector) },
    BuiltinFunc { name: "llSetCameraEyeOffset", return_type: LslType::Void, params: params!(Vector) },
    BuiltinFunc { name: "llSetCameraParams", return_type: LslType::Void, params: params!(List) },
    BuiltinFunc { name: "llSetSitText", return_type: LslType::Void, params: params!(String) },
    // ----- String functions (additions) -----
    BuiltinFunc { name: "llEscapeURL", return_type: LslType::String, params: params!(String) },
    BuiltinFunc { name: "llXorBase64StringsCorrect", return_type: LslType::String, params: params!(String, String) },
    // ----- List functions (additions) -----
    BuiltinFunc { name: "llList2ListStrided", return_type: LslType::List, params: params!(List, Integer, Integer, Integer) },
    BuiltinFunc { name: "llListInsertList", return_type: LslType::List, params: params!(List, List, Integer) },
    BuiltinFunc { name: "llListRandomize", return_type: LslType::List, params: params!(List, Integer) },
    BuiltinFunc { name: "llListReplaceList", return_type: LslType::List, params: params!(List, List, Integer, Integer) },
    BuiltinFunc { name: "llParseStringKeepNulls", return_type: LslType::List, params: params!(String, List, List) },
    // ----- Object / region info (additions) -----
    BuiltinFunc { name: "llGetAgentSize", return_type: LslType::Vector, params: params!(Key) },
    BuiltinFunc { name: "llGetAnimation", return_type: LslType::String, params: params!(Key) },
    BuiltinFunc { name: "llGetAnimationList", return_type: LslType::List, params: params!(Key) },
    BuiltinFunc { name: "llGetFreeMemory", return_type: LslType::Integer, params: params!() },
    BuiltinFunc { name: "llGetRegionName", return_type: LslType::String, params: params!() },
    BuiltinFunc { name: "llGetRegionTimeDilation", return_type: LslType::Float, params: params!() },
    BuiltinFunc { name: "llGetScriptName", return_type: LslType::String, params: params!() },
    BuiltinFunc { name: "llGetStartParameter", return_type: LslType::Integer, params: params!() },
    BuiltinFunc { name: "llGround", return_type: LslType::Float, params: params!(Vector) },
    BuiltinFunc { name: "llKey2Name", return_type: LslType::String, params: params!(Key) },
    BuiltinFunc { name: "llSameGroup", return_type: LslType::Integer, params: params!(String) },
    // ----- Detected event info (additions) -----
    BuiltinFunc { name: "llDetectedLinkNumber", return_type: LslType::Integer, params: params!(Integer) },
    BuiltinFunc { name: "llDetectedRot", return_type: LslType::Rotation, params: params!(Integer) },
    BuiltinFunc { name: "llDetectedTouchFace", return_type: LslType::Integer, params: params!(Integer) },
    BuiltinFunc { name: "llDetectedTouchST", return_type: LslType::Vector, params: params!(Integer) },
    BuiltinFunc { name: "llDetectedVel", return_type: LslType::Vector, params: params!(Integer) },
    // ----- Script control (additions) -----
    BuiltinFunc { name: "llReleaseControls", return_type: LslType::Void, params: params!() },
    BuiltinFunc { name: "llSetRemoteScriptAccessPin", return_type: LslType::Void, params: params!(Integer) },
    BuiltinFunc { name: "llTakeControls", return_type: LslType::Void, params: params!(Integer, Integer, Integer) },
    // ----- Inventory (additions) -----
    BuiltinFunc { name: "llGetInventoryCreator", return_type: LslType::Key, params: params!(String) },
    BuiltinFunc { name: "llGetInventoryKey", return_type: LslType::Key, params: params!(String) },
    BuiltinFunc { name: "llGetInventoryType", return_type: LslType::Integer, params: params!(String) },
    BuiltinFunc { name: "llGetNextEmail", return_type: LslType::Void, params: params!(String, String) },
    BuiltinFunc { name: "llGetNotecardLine", return_type: LslType::Key, params: params!(String, Integer) },
    BuiltinFunc { name: "llRemoveInventory", return_type: LslType::Void, params: params!(String) },
    // ----- HTTP / data requests -----
    BuiltinFunc { name: "llHTTPRequest", return_type: LslType::Key, params: params!(String, List, String) },
    BuiltinFunc { name: "llRequestSimulatorData", return_type: LslType::Key, params: params!(String, Integer) },
    // ----- Navigation -----
    BuiltinFunc { name: "llMapDestination", return_type: LslType::Void, params: params!(String, Vector, Vector) },
    // ----- Link / prim params (additions) -----
    BuiltinFunc { name: "llLinkParticleSystem", return_type: LslType::Void, params: params!(Integer, List) },
    BuiltinFunc { name: "llSetLinkColor", return_type: LslType::Void, params: params!(Integer, Vector, Integer) },
    BuiltinFunc { name: "llSetLinkMedia", return_type: LslType::Integer, params: params!(Integer, Integer, List) },
    BuiltinFunc { name: "llSetLinkPrimitiveParams", return_type: LslType::Void, params: params!(Integer, List) },
    BuiltinFunc { name: "llSetLinkPrimitiveParamsFast", return_type: LslType::Void, params: params!(Integer, List) },
    BuiltinFunc { name: "llSetPrimitiveParams", return_type: LslType::Void, params: params!(List) },
    // ===== OSSL Functions =====
    // ----- OSSL Animation -----
    BuiltinFunc { name: "osAvatarPlayAnimation", return_type: LslType::Void, params: params!(Key, String) },
    BuiltinFunc { name: "osAvatarStopAnimation", return_type: LslType::Void, params: params!(Key, String) },
    // ----- OSSL Inventory -----
    BuiltinFunc { name: "osGetInventoryDesc", return_type: LslType::String, params: params!(String) },
    BuiltinFunc { name: "osGetNotecard", return_type: LslType::String, params: params!(String) },
    // ----- OSSL NPC -----
    BuiltinFunc { name: "osIsNpc", return_type: LslType::Integer, params: params!(Key) },
    BuiltinFunc { name: "osNpcCreate", return_type: LslType::Key, params: params!(String, String, Vector, String) },
    BuiltinFunc { name: "osNpcCreate", return_type: LslType::Key, params: params!(String, String, Vector, String, Integer) },
    BuiltinFunc { name: "osNpcLoadAppearance", return_type: LslType::Void, params: params!(Key, String) },
    BuiltinFunc { name: "osNpcMoveToTarget", return_type: LslType::Void, params: params!(Key, Vector, Integer) },
    BuiltinFunc { name: "osNpcPlayAnimation", return_type: LslType::Void, params: params!(Key, String) },
    BuiltinFunc { name: "osNpcRemove", return_type: LslType::Void, params: params!(Key) },
    BuiltinFunc { name: "osNpcSay", return_type: LslType::Void, params: params!(Key, String) },
    BuiltinFunc { name: "osNpcSay", return_type: LslType::Void, params: params!(Key, Integer, String) },
    BuiltinFunc { name: "osNpcSetRot", return_type: LslType::Void, params: params!(Key, Rotation) },
    BuiltinFunc { name: "osNpcSit", return_type: LslType::Void, params: params!(Key, Key, Integer) },
    BuiltinFunc { name: "osNpcStand", return_type: LslType::Void, params: params!(Key) },
    BuiltinFunc { name: "osNpcStopAnimation", return_type: LslType::Void, params: params!(Key, String) },
    BuiltinFunc { name: "osNpcTouch", return_type: LslType::Void, params: params!(Key, Key, Integer) },
];

// ===================================================================
// CONSTANT TABLE
// ===================================================================

pub static BUILTIN_CONSTANTS: &[BuiltinConst] = &[
    // Boolean
    BuiltinConst { name: "TRUE", const_type: LslType::Integer },
    BuiltinConst { name: "FALSE", const_type: LslType::Integer },
    // Math
    BuiltinConst { name: "PI", const_type: LslType::Float },
    BuiltinConst { name: "TWO_PI", const_type: LslType::Float },
    BuiltinConst { name: "PI_BY_TWO", const_type: LslType::Float },
    BuiltinConst { name: "DEG_TO_RAD", const_type: LslType::Float },
    BuiltinConst { name: "RAD_TO_DEG", const_type: LslType::Float },
    BuiltinConst { name: "SQRT2", const_type: LslType::Float },
    // Special values
    BuiltinConst { name: "NULL_KEY", const_type: LslType::String },
    BuiltinConst { name: "EOF", const_type: LslType::String },
    BuiltinConst { name: "ALL_SIDES", const_type: LslType::Integer },
    // Status flags
    BuiltinConst { name: "STATUS_PHYSICS", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_ROTATE_X", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_ROTATE_Y", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_ROTATE_Z", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_PHANTOM", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_SANDBOX", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_BLOCK_GRAB", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_DIE_AT_EDGE", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_RETURN_AT_EDGE", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_CAST_SHADOWS", const_type: LslType::Integer },
    BuiltinConst { name: "STATUS_BLOCK_GRAB_OBJECT", const_type: LslType::Integer },
    // Agent / sensor type
    BuiltinConst { name: "AGENT", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_BY_LEGACY_NAME", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_BY_USERNAME", const_type: LslType::Integer },
    BuiltinConst { name: "NPC", const_type: LslType::Integer },
    BuiltinConst { name: "ACTIVE", const_type: LslType::Integer },
    BuiltinConst { name: "PASSIVE", const_type: LslType::Integer },
    BuiltinConst { name: "SCRIPTED", const_type: LslType::Integer },
    // Control flags
    BuiltinConst { name: "CONTROL_FWD", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_BACK", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_LEFT", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_RIGHT", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_UP", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_DOWN", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_ROT_LEFT", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_ROT_RIGHT", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_LBUTTON", const_type: LslType::Integer },
    BuiltinConst { name: "CONTROL_ML_LBUTTON", const_type: LslType::Integer },
    // Permissions
    BuiltinConst { name: "PERMISSION_DEBIT", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_TAKE_CONTROLS", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_TRIGGER_ANIMATION", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_ATTACH", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_CHANGE_LINKS", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_TRACK_CAMERA", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_CONTROL_CAMERA", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_TELEPORT", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_OVERRIDE_ANIMATIONS", const_type: LslType::Integer },
    BuiltinConst { name: "PERMISSION_RETURN_OBJECTS", const_type: LslType::Integer },
    // Agent info flags
    BuiltinConst { name: "AGENT_FLYING", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_ATTACHMENTS", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_SCRIPTED", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_MOUSELOOK", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_SITTING", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_ON_OBJECT", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_AWAY", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_WALKING", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_IN_AIR", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_TYPING", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_CROUCHING", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_BUSY", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_ALWAYS_RUN", const_type: LslType::Integer },
    BuiltinConst { name: "AGENT_MALE", const_type: LslType::Integer },
    // Changed event flags
    BuiltinConst { name: "CHANGED_INVENTORY", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_COLOR", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_SHAPE", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_SCALE", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_TEXTURE", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_LINK", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_ALLOWED_DROP", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_OWNER", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_REGION", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_TELEPORT", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_REGION_START", const_type: LslType::Integer },
    BuiltinConst { name: "CHANGED_MEDIA", const_type: LslType::Integer },
    // Type constants
    BuiltinConst { name: "TYPE_INTEGER", const_type: LslType::Integer },
    BuiltinConst { name: "TYPE_FLOAT", const_type: LslType::Integer },
    BuiltinConst { name: "TYPE_STRING", const_type: LslType::Integer },
    BuiltinConst { name: "TYPE_KEY", const_type: LslType::Integer },
    BuiltinConst { name: "TYPE_VECTOR", const_type: LslType::Integer },
    BuiltinConst { name: "TYPE_ROTATION", const_type: LslType::Integer },
    BuiltinConst { name: "TYPE_INVALID", const_type: LslType::Integer },
    // Link constants
    BuiltinConst { name: "LINK_SET", const_type: LslType::Integer },
    BuiltinConst { name: "LINK_ROOT", const_type: LslType::Integer },
    BuiltinConst { name: "LINK_ALL_OTHERS", const_type: LslType::Integer },
    BuiltinConst { name: "LINK_ALL_CHILDREN", const_type: LslType::Integer },
    BuiltinConst { name: "LINK_THIS", const_type: LslType::Integer },
    // Channels
    BuiltinConst { name: "DEBUG_CHANNEL", const_type: LslType::Integer },
    BuiltinConst { name: "PUBLIC_CHANNEL", const_type: LslType::Integer },
    // Inventory types
    BuiltinConst { name: "INVENTORY_ALL", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_NONE", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_TEXTURE", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_SOUND", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_LANDMARK", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_CLOTHING", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_OBJECT", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_NOTECARD", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_SCRIPT", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_BODYPART", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_ANIMATION", const_type: LslType::Integer },
    BuiltinConst { name: "INVENTORY_GESTURE", const_type: LslType::Integer },
    // Permissions mask
    BuiltinConst { name: "MASK_BASE", const_type: LslType::Integer },
    BuiltinConst { name: "MASK_OWNER", const_type: LslType::Integer },
    BuiltinConst { name: "MASK_GROUP", const_type: LslType::Integer },
    BuiltinConst { name: "MASK_EVERYONE", const_type: LslType::Integer },
    BuiltinConst { name: "MASK_NEXT", const_type: LslType::Integer },
    BuiltinConst { name: "PERM_TRANSFER", const_type: LslType::Integer },
    BuiltinConst { name: "PERM_MODIFY", const_type: LslType::Integer },
    BuiltinConst { name: "PERM_COPY", const_type: LslType::Integer },
    BuiltinConst { name: "PERM_MOVE", const_type: LslType::Integer },
    BuiltinConst { name: "PERM_ALL", const_type: LslType::Integer },
    // Vehicle types
    BuiltinConst { name: "VEHICLE_TYPE_NONE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_TYPE_SLED", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_TYPE_CAR", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_TYPE_BOAT", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_TYPE_AIRPLANE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_TYPE_BALLOON", const_type: LslType::Integer },
    // String trim
    BuiltinConst { name: "STRING_TRIM_HEAD", const_type: LslType::Integer },
    BuiltinConst { name: "STRING_TRIM_TAIL", const_type: LslType::Integer },
    BuiltinConst { name: "STRING_TRIM", const_type: LslType::Integer },
    // Data server
    BuiltinConst { name: "DATA_ONLINE", const_type: LslType::Integer },
    BuiltinConst { name: "DATA_NAME", const_type: LslType::Integer },
    BuiltinConst { name: "DATA_BORN", const_type: LslType::Integer },
    BuiltinConst { name: "DATA_RATING", const_type: LslType::Integer },
    // Prim types
    BuiltinConst { name: "PRIM_TYPE_BOX", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TYPE_CYLINDER", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TYPE_PRISM", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TYPE_SPHERE", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TYPE_TORUS", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TYPE_TUBE", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TYPE_RING", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TYPE_SCULPT", const_type: LslType::Integer },
    // Prim params
    BuiltinConst { name: "PRIM_MATERIAL", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_PHYSICS", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TEMP_ON_REZ", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_PHANTOM", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_POSITION", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_SIZE", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_ROTATION", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TYPE", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TEXTURE", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_COLOR", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_FULLBRIGHT", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_FLEXIBLE", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_GLOW", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_TEXT", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_NAME", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_DESC", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_POINT_LIGHT", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_ROT_LOCAL", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_POS_LOCAL", const_type: LslType::Integer },
    // Prim media params
    BuiltinConst { name: "PRIM_MEDIA_ALT_IMAGE_ENABLE", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_MEDIA_CURRENT_URL", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_MEDIA_AUTO_PLAY", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_MEDIA_AUTO_SCALE", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_MEDIA_WIDTH_PIXELS", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_MEDIA_HEIGHT_PIXELS", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_MEDIA_PERMS_INTERACT", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_MEDIA_PERMS_CONTROL", const_type: LslType::Integer },
    BuiltinConst { name: "PRIM_MEDIA_PERM_NONE", const_type: LslType::Integer },
    // Attach points
    BuiltinConst { name: "ATTACH_LSHOULDER", const_type: LslType::Integer },
    // Camera constants
    BuiltinConst { name: "CAMERA_PITCH", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_FOCUS_OFFSET", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_POSITION_LAG", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_FOCUS_LAG", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_DISTANCE", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_BEHINDNESS_ANGLE", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_BEHINDNESS_LAG", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_POSITION_THRESHOLD", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_FOCUS_THRESHOLD", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_ACTIVE", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_POSITION_LOCKED", const_type: LslType::Integer },
    BuiltinConst { name: "CAMERA_FOCUS_LOCKED", const_type: LslType::Integer },
    // Click action
    BuiltinConst { name: "CLICK_ACTION_SIT", const_type: LslType::Integer },
    BuiltinConst { name: "CLICK_ACTION_TOUCH", const_type: LslType::Integer },
    // Data server
    BuiltinConst { name: "DATA_SIM_POS", const_type: LslType::Integer },
    BuiltinConst { name: "DATA_SIM_STATUS", const_type: LslType::Integer },
    // HTTP
    BuiltinConst { name: "HTTP_METHOD", const_type: LslType::Integer },
    // Object details
    BuiltinConst { name: "OBJECT_NAME", const_type: LslType::Integer },
    BuiltinConst { name: "OBJECT_POS", const_type: LslType::Integer },
    BuiltinConst { name: "OBJECT_CREATOR", const_type: LslType::Integer },
    // OSSL NPC constants
    BuiltinConst { name: "OS_NPC_CREATOR_OWNED", const_type: LslType::Integer },
    BuiltinConst { name: "OS_NPC_NO_FLY", const_type: LslType::Integer },
    BuiltinConst { name: "OS_NPC_SIT_NOW", const_type: LslType::Integer },
    // Particle system flags
    BuiltinConst { name: "PSYS_PART_INTERP_COLOR_MASK", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_INTERP_SCALE_MASK", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_FOLLOW_SRC_MASK", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_FOLLOW_VELOCITY_MASK", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_TARGET_POS_MASK", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_EMISSIVE_MASK", const_type: LslType::Integer },
    // Particle system params
    BuiltinConst { name: "PSYS_PART_FLAGS", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_START_COLOR", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_START_ALPHA", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_END_COLOR", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_END_ALPHA", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_START_SCALE", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_END_SCALE", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_PART_MAX_AGE", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_ACCEL", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_PATTERN", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_TEXTURE", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_BURST_RATE", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_BURST_PART_COUNT", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_BURST_RADIUS", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_BURST_SPEED_MIN", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_BURST_SPEED_MAX", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_MAX_AGE", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_TARGET_KEY", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_OMEGA", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_ANGLE_BEGIN", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_ANGLE_END", const_type: LslType::Integer },
    // Particle source patterns
    BuiltinConst { name: "PSYS_SRC_PATTERN_EXPLODE", const_type: LslType::Integer },
    BuiltinConst { name: "PSYS_SRC_PATTERN_ANGLE_CONE", const_type: LslType::Integer },
    // Texture constants
    BuiltinConst { name: "TEXTURE_MEDIA", const_type: LslType::String },
    // Touch constants
    BuiltinConst { name: "TOUCH_INVALID_TEXCOORD", const_type: LslType::Vector },
    // Vehicle params
    BuiltinConst { name: "VEHICLE_LINEAR_FRICTION_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_ANGULAR_FRICTION_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_LINEAR_MOTOR_DIRECTION", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_LINEAR_MOTOR_OFFSET", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_ANGULAR_MOTOR_DIRECTION", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_HOVER_HEIGHT", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_HOVER_EFFICIENCY", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_HOVER_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_BUOYANCY", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_LINEAR_DEFLECTION_EFFICIENCY", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_LINEAR_DEFLECTION_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_LINEAR_MOTOR_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_LINEAR_MOTOR_DECAY_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_ANGULAR_DEFLECTION_EFFICIENCY", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_ANGULAR_DEFLECTION_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_ANGULAR_MOTOR_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_ANGULAR_MOTOR_DECAY_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_VERTICAL_ATTRACTION_EFFICIENCY", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_VERTICAL_ATTRACTION_TIMESCALE", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_BANKING_EFFICIENCY", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_BANKING_MIX", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_BANKING_TIMESCALE", const_type: LslType::Integer },
    // Vehicle flags
    BuiltinConst { name: "VEHICLE_FLAG_NO_DEFLECTION_UP", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_FLAG_HOVER_WATER_ONLY", const_type: LslType::Integer },
    BuiltinConst { name: "VEHICLE_FLAG_HOVER_UP_ONLY", const_type: LslType::Integer },
    // Special values
    BuiltinConst { name: "ZERO_VECTOR", const_type: LslType::Vector },
    BuiltinConst { name: "ZERO_ROTATION", const_type: LslType::Rotation },
];

// ===================================================================
// Lookup helpers
// ===================================================================

pub fn find_builtin_func(name: &str, arg_count: usize) -> Option<&'static BuiltinFunc> {
    // Prefer exact arity match (handles overloads), fall back to name-only
    // so the caller can still report arg-count mismatches.
    BUILTIN_FUNCTIONS.iter().find(|f| f.name == name && f.params.len() == arg_count)
        .or_else(|| BUILTIN_FUNCTIONS.iter().find(|f| f.name == name))
}

pub fn find_builtin_const(name: &str) -> Option<&'static BuiltinConst> {
    BUILTIN_CONSTANTS.iter().find(|c| c.name == name)
}
