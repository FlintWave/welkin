// SPDX-License-Identifier: GPL-3.0-or-later
//! Tasks 13.1–13.2: OSSL proof tests ported from C# OpenSimulator test tree.
//!
//! Source C# files:
//!   - `OpenSim/Region/ScriptEngine/Shared/Tests/OSSL_ApiNpcTests.cs`
//!   - `OpenSim/Region/ScriptEngine/Shared/Tests/OSSL_ApiAttachmentTests.cs`
//!   - `OpenSim/Region/ScriptEngine/Shared/Tests/OSSL_ApiAppearanceTest.cs`
//!
//! Since NPC/attachment/appearance functions are stubs in the Rust port, these
//! tests verify:
//!   1. Functions are registered and callable (don't panic)
//!   2. Stub return values match expected defaults
//!   3. Threat-level gating works
//!   4. Function signatures match C# expectations

use std::collections::HashSet;
use std::sync::Arc;

use mlua::Lua;
use parking_lot::RwLock;
use uuid::Uuid;

use welkin_script_vm::builtins::ScriptContext;
use welkin_script_vm::ossl::{
    function_catalog, register_ossl, OsslConfig, OsslThreatLevel,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_ctx() -> Arc<RwLock<ScriptContext>> {
    Arc::new(RwLock::new(ScriptContext {
        script_id: Uuid::new_v4(),
        prim_id: Uuid::new_v4(),
        owner_id: Uuid::new_v4(),
        ..ScriptContext::default()
    }))
}

/// VM with all OSSL functions enabled (Severe threat level).
fn vm_all() -> Lua {
    let lua = Lua::new();
    let ctx = make_ctx();
    let config = OsslConfig {
        max_threat_level: OsslThreatLevel::Severe,
        ..Default::default()
    };
    register_ossl(&lua, &config, &ctx).unwrap();
    lua
}

/// VM with a specific max threat level.
fn vm_at(level: OsslThreatLevel) -> Lua {
    let lua = Lua::new();
    let ctx = make_ctx();
    let config = OsslConfig {
        max_threat_level: level,
        ..Default::default()
    };
    register_ossl(&lua, &config, &ctx).unwrap();
    lua
}

/// VM with a specific max threat level plus explicit allowed functions.
fn vm_with_allowed(level: OsslThreatLevel, allowed: &[&str]) -> Lua {
    let lua = Lua::new();
    let ctx = make_ctx();
    let config = OsslConfig {
        max_threat_level: level,
        allowed_functions: allowed.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    register_ossl(&lua, &config, &ctx).unwrap();
    lua
}

/// Assert that a Lua global is nil (not registered).
fn assert_nil(lua: &Lua, name: &str) {
    let v: mlua::Value = lua.load(&format!("return {name}")).eval().unwrap();
    assert!(
        matches!(v, mlua::Value::Nil),
        "{name} should not be registered but is: {v:?}"
    );
}

/// Assert that a Lua global is a function (registered).
fn assert_fn(lua: &Lua, name: &str) {
    let v: mlua::Value = lua.load(&format!("return {name}")).eval().unwrap();
    assert!(
        matches!(v, mlua::Value::Function(_)),
        "{name} should be registered as a function but is: {v:?}"
    );
}

// =========================================================================
// Task 13.1 — NPC stub tests
// (Ported from OSSL_ApiNpcTests.cs / OSSL_NpcApiAppearanceTest)
// =========================================================================

/// Port of TestOsNpcCreateUsingAppearanceFromNotecard (stub version).
/// The C# test creates an NPC from a notecard. Our stub version verifies
/// osNpcCreate is callable and returns a valid UUID string (random v4).
#[test]
fn npc_create_stub_returns_valid_uuid() {
    let lua = vm_all();
    let r: String = lua
        .load(r#"return osNpcCreate("Jane", "Doe", "<128,128,128>", "appearanceNc")"#)
        .eval()
        .unwrap();
    // The stub returns a random v4 UUID, verify it parses as one.
    let parsed = Uuid::parse_str(&r);
    assert!(
        parsed.is_ok(),
        "stub osNpcCreate should return a valid UUID string, got: {r}"
    );
}

/// Port of TestOsNpcRemoveOwned / TestOsNpcRemoveUnowned.
/// Verify osNpcRemove is callable and does not panic.
#[test]
fn npc_remove_stub_no_panic() {
    let lua = vm_all();
    lua.load(r#"osNpcRemove("00000000-0000-0000-0000-000000000001")"#)
        .exec()
        .unwrap();
}

/// Verify osIsNpc returns 0 (false) for any key — stubs have no NPC state.
/// C# equivalent: checks whether an agent is an NPC.
#[test]
fn npc_is_npc_returns_zero() {
    let lua = vm_all();
    let r: i32 = lua
        .load(r#"return osIsNpc("some-agent-key")"#)
        .eval()
        .unwrap();
    assert_eq!(r, 0, "osIsNpc stub should return 0 (false)");
}

/// Verify osNpcGetOwner returns nil UUID — no NPC exists in stubs.
/// C# equivalent: retrieves the owner UUID of an NPC.
#[test]
fn npc_get_owner_returns_nil_uuid() {
    let lua = vm_all();
    let r: String = lua
        .load(r#"return osNpcGetOwner("npc-key")"#)
        .eval()
        .unwrap();
    assert_eq!(r, "00000000-0000-0000-0000-000000000000");
}

/// Verify osNpcGetPos returns zero vector.
#[test]
fn npc_get_pos_returns_zero_vector() {
    let lua = vm_all();
    let r: String = lua
        .load(r#"return osNpcGetPos("npc-key")"#)
        .eval()
        .unwrap();
    assert_eq!(r, "<0, 0, 0>");
}

/// Verify osNpcGetRot returns identity quaternion.
#[test]
fn npc_get_rot_returns_identity() {
    let lua = vm_all();
    let r: String = lua
        .load(r#"return osNpcGetRot("npc-key")"#)
        .eval()
        .unwrap();
    assert_eq!(r, "<0, 0, 0, 1>");
}

/// Port of TestOsNpcLoadAppearance — verify the stub is callable.
#[test]
fn npc_load_appearance_stub_no_panic() {
    let lua = vm_all();
    lua.load(r#"osNpcLoadAppearance("npc-key", "notecard-name")"#)
        .exec()
        .unwrap();
}

/// Verify NPC movement/animation stubs don't panic.
/// C# tests exercise NPC walking, sitting, looking; our stubs just log.
#[test]
fn npc_movement_stubs_no_panic() {
    let lua = vm_all();
    lua.load(r#"osNpcMoveToTarget("npc", "<128,128,25>", 0)"#)
        .exec()
        .unwrap();
    lua.load(r#"osNpcStopMoveToTarget("npc")"#)
        .exec()
        .unwrap();
    lua.load(r#"osNpcSit("npc", "prim-key", 0)"#)
        .exec()
        .unwrap();
    lua.load(r#"osNpcStand("npc")"#).exec().unwrap();
    lua.load(r#"osNpcLookAt("npc", "<128,128,25>")"#)
        .exec()
        .unwrap();
}

/// Verify NPC communication stubs don't panic.
#[test]
fn npc_say_shout_whisper_no_panic() {
    let lua = vm_all();
    // osNpcSayTo takes (npc, target, channel, msg)
    lua.load(r#"osNpcSayTo("npc", "target", 0, "hello")"#)
        .exec()
        .unwrap();
    lua.load(r#"osNpcShout("npc", 0, "hello")"#)
        .exec()
        .unwrap();
    lua.load(r#"osNpcWhisper("npc", 0, "hello")"#)
        .exec()
        .unwrap();
}

// =========================================================================
// Task 13.1 — Attachment stub tests
// (Ported from OSSL_ApiAttachmentTests.cs)
// =========================================================================

/// Port of TestOsForceAttachToAvatarFromInventory (stub version).
/// In C# this actually attaches an object; our stub just returns without
/// error.
#[test]
fn attachment_force_attach_stub_no_panic() {
    let lua = vm_all();
    // osForceAttachToAvatar(attachment_point) — void stub
    lua.load(r#"osForceAttachToAvatar(9)"#).exec().unwrap();
}

/// Port of TestOsForceAttachToOtherAvatarFromInventory (stub version).
#[test]
fn attachment_force_attach_other_avatar_no_panic() {
    let lua = vm_all();
    lua.load(
        r#"osForceAttachToOtherAvatarFromInventory("avatar-key", "sphere", 9)"#,
    )
    .exec()
    .unwrap();
}

/// Verify osForceDetachFromAvatar stub doesn't panic.
#[test]
fn attachment_force_detach_no_panic() {
    let lua = vm_all();
    lua.load(r#"osForceDetachFromAvatar()"#).exec().unwrap();
}

// =========================================================================
// Task 13.1 — Appearance stub tests
// (Ported from OSSL_ApiAppearanceTest.cs)
// =========================================================================

/// Port of TestOsOwnerSaveAppearance.
/// C# saves appearance to notecard; our stub returns nil UUID.
#[test]
fn appearance_owner_save_returns_nil_uuid() {
    let lua = vm_all();
    let r: String = lua
        .load(r#"return osOwnerSaveAppearance("notecardName")"#)
        .eval()
        .unwrap();
    assert_eq!(r, "00000000-0000-0000-0000-000000000000");
}

/// Port of TestOsAgentSaveAppearance.
/// C# saves another agent's appearance; our stub returns nil UUID.
#[test]
fn appearance_agent_save_returns_nil_uuid() {
    let lua = vm_all();
    let r: String = lua
        .load(r#"return osAgentSaveAppearance("agent-key", "notecardName")"#)
        .eval()
        .unwrap();
    assert_eq!(r, "00000000-0000-0000-0000-000000000000");
}

/// Verify osNpcSaveAppearance stub returns nil UUID.
#[test]
fn npc_save_appearance_returns_nil_uuid() {
    let lua = vm_all();
    let r: String = lua
        .load(r#"return osNpcSaveAppearance("npc-key", "notecard-name")"#)
        .eval()
        .unwrap();
    assert_eq!(r, "00000000-0000-0000-0000-000000000000");
}

// =========================================================================
// Task 13.1 — Threat-level gating tests
// =========================================================================

/// NPC functions are High threat. At Low config they should be blocked.
#[test]
fn threat_gating_blocks_npc_at_low() {
    let lua = vm_at(OsslThreatLevel::Low);
    assert_nil(&lua, "osNpcCreate");
    assert_nil(&lua, "osIsNpc");
    assert_nil(&lua, "osNpcGetOwner");
    assert_nil(&lua, "osNpcRemove");
    assert_nil(&lua, "osNpcMoveToTarget");
}

/// At High config, NPC functions should be registered.
#[test]
fn threat_gating_allows_npc_at_high() {
    let lua = vm_at(OsslThreatLevel::High);
    assert_fn(&lua, "osNpcCreate");
    assert_fn(&lua, "osIsNpc");
    assert_fn(&lua, "osNpcGetOwner");
    assert_fn(&lua, "osNpcRemove");
}

/// Attachment functions are VeryHigh/Severe threat. At Moderate they're blocked.
#[test]
fn threat_gating_blocks_attachment_at_moderate() {
    let lua = vm_at(OsslThreatLevel::Moderate);
    assert_nil(&lua, "osForceAttachToAvatar");
    assert_nil(&lua, "osForceAttachToOtherAvatarFromInventory");
    assert_nil(&lua, "osForceDetachFromAvatar");
}

/// Appearance save functions are VeryHigh threat. At High they're blocked.
#[test]
fn threat_gating_blocks_appearance_at_high() {
    let lua = vm_at(OsslThreatLevel::High);
    assert_nil(&lua, "osOwnerSaveAppearance");
    assert_nil(&lua, "osAgentSaveAppearance");
}

/// osTeleportAgent is Severe. At VeryHigh it should be blocked.
#[test]
fn threat_gating_blocks_teleport_at_veryhigh() {
    let lua = vm_at(OsslThreatLevel::VeryHigh);
    assert_nil(&lua, "osTeleportAgent");
}

/// None-threat functions (draw, math) should be available even at None config.
#[test]
fn threat_gating_none_functions_always_available() {
    let lua = vm_at(OsslThreatLevel::None);
    assert_fn(&lua, "osDrawText");
    assert_fn(&lua, "osMin");
    assert_fn(&lua, "osSetPenColor");
}

// =========================================================================
// Task 13.1 — allowed_functions override tests
// =========================================================================

/// A High-threat function blocked by config can be permitted by allowed_functions.
#[test]
fn allowed_functions_override_permits_npc_create() {
    let lua = vm_with_allowed(OsslThreatLevel::None, &["osNpcCreate"]);
    assert_fn(&lua, "osNpcCreate");
    // Other High functions remain blocked.
    assert_nil(&lua, "osIsNpc");
    assert_nil(&lua, "osNpcGetOwner");
}

/// Multiple functions can be allowed.
#[test]
fn allowed_functions_override_permits_multiple() {
    let lua = vm_with_allowed(
        OsslThreatLevel::None,
        &["osNpcCreate", "osTeleportAgent", "osOwnerSaveAppearance"],
    );
    assert_fn(&lua, "osNpcCreate");
    assert_fn(&lua, "osTeleportAgent");
    assert_fn(&lua, "osOwnerSaveAppearance");
    // Not in the list: still blocked.
    assert_nil(&lua, "osNpcRemove");
}

/// allowed_functions override works for Severe-level functions even at None config.
#[test]
fn allowed_functions_override_severe_at_none() {
    let lua = vm_with_allowed(OsslThreatLevel::None, &["osTeleportAgent"]);
    assert_fn(&lua, "osTeleportAgent");
}

// =========================================================================
// Task 13.2 — Catalog verification tests
// =========================================================================

/// function_catalog() returns at least 200 entries.
#[test]
fn catalog_has_at_least_200_entries() {
    let catalog = function_catalog();
    assert!(
        catalog.len() >= 200,
        "expected at least 200 OSSL functions in catalog, got {}",
        catalog.len()
    );
}

/// Spot-check 10 specific functions exist with correct threat levels.
/// Threat levels are verified against the actual Rust registration code.
#[test]
fn catalog_spot_check_threat_levels() {
    let catalog = function_catalog();
    let map: std::collections::HashMap<&str, OsslThreatLevel> =
        catalog.into_iter().collect();

    let checks: &[(&str, OsslThreatLevel)] = &[
        ("osNpcCreate", OsslThreatLevel::High),
        ("osGetNotecard", OsslThreatLevel::VeryHigh),
        ("osSHA256", OsslThreatLevel::VeryLow),
        ("osGetGridName", OsslThreatLevel::None),
        ("osSetTerrainHeight", OsslThreatLevel::High),
        ("osTeleportAgent", OsslThreatLevel::Severe),
        ("osDrawText", OsslThreatLevel::None),
        ("osMatchString", OsslThreatLevel::VeryLow),
        ("osMin", OsslThreatLevel::None),
        ("osSetPenColor", OsslThreatLevel::None),
    ];

    for &(name, expected_level) in checks {
        let actual = map.get(name);
        assert!(
            actual.is_some(),
            "function {name} not found in catalog"
        );
        assert_eq!(
            *actual.unwrap(),
            expected_level,
            "threat level mismatch for {name}: expected {expected_level:?}, got {:?}",
            actual.unwrap()
        );
    }
}

/// No duplicate function names in the catalog.
#[test]
fn catalog_no_duplicates() {
    let catalog = function_catalog();
    let mut seen = HashSet::new();
    for (name, _level) in &catalog {
        assert!(
            seen.insert(*name),
            "duplicate OSSL function name in catalog: {name}"
        );
    }
}

/// All catalog entries have non-empty names that start with "os".
#[test]
fn catalog_names_start_with_os() {
    let catalog = function_catalog();
    for (name, _level) in &catalog {
        assert!(
            name.starts_with("os"),
            "OSSL function name should start with 'os': {name}"
        );
        assert!(
            name.len() > 2,
            "OSSL function name too short: {name}"
        );
    }
}

/// Verify the catalog covers all threat levels (at least one function per level).
#[test]
fn catalog_covers_all_threat_levels() {
    let catalog = function_catalog();
    let levels: HashSet<OsslThreatLevel> = catalog.iter().map(|(_, l)| *l).collect();
    for expected in [
        OsslThreatLevel::None,
        OsslThreatLevel::VeryLow,
        OsslThreatLevel::Low,
        OsslThreatLevel::Moderate,
        OsslThreatLevel::High,
        OsslThreatLevel::VeryHigh,
        OsslThreatLevel::Severe,
    ] {
        assert!(
            levels.contains(&expected),
            "no functions found at threat level {expected:?}"
        );
    }
}
