// SPDX-License-Identifier: LGPL-3.0-or-later
//! OpenSimulator OSSL extension functions for the Luau script VM.
//!
//! OSSL (OpenSim Scripting Language) extends LSL with os* functions that
//! give scripts access to simulator-specific features such as NPC control,
//! dynamic textures, teleportation, and region management.  Each function
//! carries a [`OsslThreatLevel`] that gates whether a particular region
//! configuration permits its use.
//!
//! Reference: `OpenSim/Region/ScriptEngine/Shared/Api/Implementation/OSSL_Api.cs`

pub mod agent_stubs;
pub mod crypto;
pub mod draw;
pub mod inventory_stubs;
pub mod math;
pub mod misc_stubs;
pub mod npc_stubs;
pub mod sim_info;
pub mod string;
pub mod stubs;
pub mod terrain_stubs;
pub mod time;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use crate::builtins::ScriptContext;

// ---------------------------------------------------------------------------
// OsslThreatLevel
// ---------------------------------------------------------------------------

/// Threat levels mirror the C# `ThreatLevel` enum used by OpenSimulator to
/// gate which OSSL functions a region permits.
///
/// The ordering is significant: a function is allowed when its level is
/// less than or equal to the configured maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OsslThreatLevel {
    /// No threat at all; no side effects.
    None = 0,
    /// Abuse can cause log spam or similar nuisance.
    Nuisance = 1,
    /// Extreme abuse can impair a region slightly.
    VeryLow = 2,
    /// Intentional abuse can cause recoverable crashes.
    Low = 3,
    /// Intentional abuse can cause denial of service with potential data loss.
    Moderate = 4,
    /// Casual abuse can impair functionality; intentional abuse causes data loss.
    High = 5,
    /// Normal use may cause service impairment depending on frequency.
    VeryHigh = 6,
    /// Even casual use is a danger to region stability.
    Severe = 7,
}

impl std::fmt::Display for OsslThreatLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::None => "None",
            Self::Nuisance => "Nuisance",
            Self::VeryLow => "VeryLow",
            Self::Low => "Low",
            Self::Moderate => "Moderate",
            Self::High => "High",
            Self::VeryHigh => "VeryHigh",
            Self::Severe => "Severe",
        };
        write!(f, "{name}")
    }
}

impl OsslThreatLevel {
    /// Parse a threat-level name (case-insensitive).
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "nuisance" => Some(Self::Nuisance),
            "verylow" => Some(Self::VeryLow),
            "low" => Some(Self::Low),
            "moderate" => Some(Self::Moderate),
            "high" => Some(Self::High),
            "veryhigh" => Some(Self::VeryHigh),
            "severe" => Some(Self::Severe),
            _ => Option::None,
        }
    }
}

// ---------------------------------------------------------------------------
// OsslRole & FunctionPerm  (task 1.1)
// ---------------------------------------------------------------------------

/// Caller roles for per-function permission checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OsslRole {
    Anyone,
    EstateManager,
    EstateOwner,
    ParcelOwner,
    ParcelGroupMember,
}

impl OsslRole {
    /// Parse a role name (case-insensitive, underscore-separated).
    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "ESTATE_MANAGER" => Some(Self::EstateManager),
            "ESTATE_OWNER" => Some(Self::EstateOwner),
            "PARCEL_OWNER" => Some(Self::ParcelOwner),
            "PARCEL_GROUP_MEMBER" => Some(Self::ParcelGroupMember),
            _ => Option::None,
        }
    }
}

/// Per-function permission override.
#[derive(Debug, Clone)]
pub struct FunctionPerm {
    /// The threat level assigned to this specific function override.
    pub threat_level: OsslThreatLevel,
    /// Which roles are allowed to call this function.
    pub allowed_roles: Vec<OsslRole>,
}

// ---------------------------------------------------------------------------
// OsslConfig  (task 1.1)
// ---------------------------------------------------------------------------

/// Runtime configuration that controls which OSSL functions a region
/// exposes to scripts.
#[derive(Debug, Clone)]
pub struct OsslConfig {
    /// The highest threat level that is implicitly allowed.
    pub max_threat_level: OsslThreatLevel,
    /// Function names that are explicitly allowed regardless of their
    /// threat level (e.g. `"osGetSimulatorVersion"`).
    pub allowed_functions: HashSet<String>,
    /// Per-function permission overrides (task 1.1).
    pub function_perms: HashMap<String, FunctionPerm>,
}

impl Default for OsslConfig {
    fn default() -> Self {
        Self {
            max_threat_level: OsslThreatLevel::VeryLow,
            allowed_functions: HashSet::new(),
            function_perms: HashMap::new(),
        }
    }
}

impl OsslConfig {
    /// Returns `true` if `func_name` at the given `level` is permitted
    /// under this configuration.
    ///
    /// When `caller_role` is provided, per-function permission overrides
    /// are checked first.
    pub fn is_allowed(
        &self,
        func_name: &str,
        level: OsslThreatLevel,
        caller_role: Option<OsslRole>,
    ) -> bool {
        // Check per-function overrides first.
        if let Some(perm) = self.function_perms.get(func_name) {
            // If the perm has allowed_roles, check the caller.
            if !perm.allowed_roles.is_empty() {
                if let Some(role) = caller_role {
                    return perm.allowed_roles.contains(&role);
                }
                // No caller role provided but roles are required — denied.
                return false;
            }
            // Override has no role restriction: use its threat level.
            return perm.threat_level <= self.max_threat_level;
        }

        // Fall back to global max_threat_level + allowed_functions.
        level <= self.max_threat_level || self.allowed_functions.contains(func_name)
    }

    /// Backward-compatible `is_allowed` without a caller role.
    pub fn is_allowed_simple(&self, func_name: &str, level: OsslThreatLevel) -> bool {
        self.is_allowed(func_name, level, Option::None)
    }
}

// ---------------------------------------------------------------------------
// INI parser for [OSSL] section  (task 1.2)
// ---------------------------------------------------------------------------

/// Known macro expansions for `${OSSL|...}` references in INI values.
fn expand_macro(macro_name: &str) -> Option<&'static str> {
    match macro_name {
        "osslParcelO" => Some("PARCEL_OWNER"),
        "osslParcelOG" => Some("PARCEL_OWNER,PARCEL_GROUP_MEMBER"),
        "osslNPC" => Some("ESTATE_MANAGER,ESTATE_OWNER"),
        "osslParcelP" => Some("PARCEL_OWNER,PARCEL_GROUP_MEMBER,ESTATE_MANAGER,ESTATE_OWNER"),
        "osslEstateO" => Some("ESTATE_OWNER"),
        "osslEstateM" => Some("ESTATE_MANAGER"),
        _ => Option::None,
    }
}

/// Expand `${OSSL|name}` macros in a value string.
fn expand_macros(value: &str) -> String {
    let mut result = value.to_string();
    // Simple loop: keep expanding until no more macros.
    loop {
        let Some(start) = result.find("${OSSL|") else {
            break;
        };
        let Some(end) = result[start..].find('}') else {
            break;
        };
        let end = start + end;
        let macro_name = &result[start + 7..end];
        let replacement = expand_macro(macro_name).unwrap_or("");
        result = format!("{}{}{}", &result[..start], replacement, &result[end + 1..]);
    }
    result
}

/// Parse an INI `[OSSL]` section body into an `OsslConfig`.
///
/// Supported keys:
/// - `OSFunctionThreatLevel = VeryLow`
/// - `Allow_osXxx = true` (adds to allowed_functions)
/// - `Allow_osXxx = ESTATE_MANAGER,ESTATE_OWNER` (adds per-function perm)
///
/// Malformed lines are logged and skipped.
pub fn parse_ossl_ini(ini_body: &str) -> OsslConfig {
    let mut config = OsslConfig::default();

    for (line_no, raw_line) in ini_body.lines().enumerate() {
        let line = raw_line.trim();

        // Skip comments and blank lines.
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            tracing::warn!(line = line_no + 1, content = %line, "malformed OSSL INI line: no '='");
            continue;
        };

        let key = key.trim();
        let value = value.trim();

        if key.eq_ignore_ascii_case("OSFunctionThreatLevel") {
            match OsslThreatLevel::from_name(value) {
                Some(level) => config.max_threat_level = level,
                Option::None => {
                    tracing::warn!(
                        line = line_no + 1,
                        value = %value,
                        "unknown OSSL threat level"
                    );
                }
            }
            continue;
        }

        if let Some(func_name) = key.strip_prefix("Allow_") {
            let expanded = expand_macros(value);
            let trimmed = expanded.trim();

            if trimmed.eq_ignore_ascii_case("true") {
                config.allowed_functions.insert(func_name.to_string());
            } else if trimmed.eq_ignore_ascii_case("false") {
                config.allowed_functions.remove(func_name);
            } else {
                // Parse as comma-separated role list.
                let roles: Vec<OsslRole> = trimmed
                    .split(',')
                    .filter_map(|r| OsslRole::from_name(r.trim()))
                    .collect();

                if roles.is_empty() {
                    tracing::warn!(
                        line = line_no + 1,
                        value = %trimmed,
                        func = %func_name,
                        "no valid roles parsed from Allow_ value"
                    );
                } else {
                    config.function_perms.insert(
                        func_name.to_string(),
                        FunctionPerm {
                            threat_level: OsslThreatLevel::Severe,
                            allowed_roles: roles,
                        },
                    );
                }
            }
            continue;
        }

        tracing::warn!(line = line_no + 1, key = %key, "unknown OSSL INI key");
    }

    config
}

// ---------------------------------------------------------------------------
// Function registry  (name, threat-level, registration closure)
// ---------------------------------------------------------------------------

/// Describes one OSSL function for the purpose of registration.
pub(crate) struct OsslFunc {
    pub(crate) name: &'static str,
    pub(crate) level: OsslThreatLevel,
    /// Closure that registers the Lua global, capturing the context arc.
    pub(crate) register: fn(&Lua, &Arc<RwLock<ScriptContext>>) -> LuaResult<()>,
}

/// Master list of OSSL functions implemented by this module.
fn ossl_functions() -> Vec<OsslFunc> {
    let mut funcs = Vec::new();

    // Stubs: notecard, dynamic texture, parcel, agent, teleport, NPC, etc.
    funcs.extend(stubs::stub_functions());

    // Draw functions (pure string manipulation)
    funcs.extend(draw::draw_functions());

    // String/regex functions
    funcs.extend(string::string_functions());

    // Math/utility functions
    funcs.extend(math::math_functions());

    // Crypto functions
    funcs.extend(crypto::crypto_functions());

    // Time functions
    funcs.extend(time::time_functions());

    // Sim/grid info functions
    funcs.extend(sim_info::sim_info_functions());

    // Tasks 8.1–12.1: extended stubs
    funcs.extend(npc_stubs::ossl_npc_funcs());
    funcs.extend(agent_stubs::ossl_agent_funcs());
    funcs.extend(terrain_stubs::ossl_terrain_funcs());
    funcs.extend(inventory_stubs::ossl_inventory_funcs());
    funcs.extend(misc_stubs::ossl_misc_funcs());

    funcs
}

// ---------------------------------------------------------------------------
// Public registration entry point
// ---------------------------------------------------------------------------

/// Register OSSL functions into the Lua VM, gated by the provided config.
///
/// Only functions whose threat level is at or below `config.max_threat_level`,
/// or that appear in `config.allowed_functions`, are registered.  All others
/// are silently skipped.
pub fn register_ossl(
    lua: &Lua,
    config: &OsslConfig,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    for func in ossl_functions() {
        if config.is_allowed_simple(func.name, func.level) {
            (func.register)(lua, ctx)?;
            tracing::debug!(
                name = func.name,
                level = %func.level,
                "registered OSSL function"
            );
        } else {
            tracing::debug!(
                name = func.name,
                level = %func.level,
                max = %config.max_threat_level,
                "skipped OSSL function (threat level too high)"
            );
        }
    }

    // Register the OSSL version constant
    lua.globals().set("OS_VERSION", "welkin 0.1.0")?;

    Ok(())
}

/// Returns the list of OSSL function names and their threat levels.
///
/// Useful for tooling and introspection.
pub fn function_catalog() -> Vec<(&'static str, OsslThreatLevel)> {
    ossl_functions()
        .into_iter()
        .map(|f| (f.name, f.level))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            ..ScriptContext::default()
        }))
    }

    fn make_vm() -> Lua {
        Lua::new()
    }

    // 1. OsslThreatLevel ordering
    #[test]
    fn threat_level_ordering() {
        assert!(OsslThreatLevel::None < OsslThreatLevel::Nuisance);
        assert!(OsslThreatLevel::Nuisance < OsslThreatLevel::VeryLow);
        assert!(OsslThreatLevel::VeryLow < OsslThreatLevel::Low);
        assert!(OsslThreatLevel::Low < OsslThreatLevel::Moderate);
        assert!(OsslThreatLevel::Moderate < OsslThreatLevel::High);
        assert!(OsslThreatLevel::High < OsslThreatLevel::VeryHigh);
        assert!(OsslThreatLevel::VeryHigh < OsslThreatLevel::Severe);
        assert_eq!(OsslThreatLevel::High, OsslThreatLevel::High);
        assert_eq!(OsslThreatLevel::Severe as u8, 7);
    }

    // 2. Config allows appropriate functions
    #[test]
    fn config_allows_appropriate_functions() {
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Moderate,
            allowed_functions: HashSet::new(),
            function_perms: HashMap::new(),
        };

        assert!(config.is_allowed_simple("osDrawText", OsslThreatLevel::None));
        assert!(config.is_allowed_simple("osFormatString", OsslThreatLevel::VeryLow));
        assert!(config.is_allowed_simple("osSetSpeed", OsslThreatLevel::Moderate));
        assert!(!config.is_allowed_simple("osNpcCreate", OsslThreatLevel::High));
        assert!(!config.is_allowed_simple("osGetAgentIP", OsslThreatLevel::Severe));
    }

    // 3. osGetSimulatorVersion returns correct string
    #[test]
    fn os_get_simulator_version_returns_correct_string() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Severe,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        let version: String = lua.load("return osGetSimulatorVersion()").eval().unwrap();
        assert_eq!(version, "welkin 0.1.0");
    }

    // 4. High threat function blocked at Low config
    #[test]
    fn high_threat_blocked_at_low_config() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Low,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        // osNpcCreate (High) should not be registered
        let result: LuaResult<mlua::Value> = lua.load("return osNpcCreate").eval();
        if let Ok(mlua::Value::Function(_)) = result {
            panic!("osNpcCreate should not be registered at Low threat level")
        }

        // osGetSimulatorVersion (High) also blocked
        let result: LuaResult<mlua::Value> = lua.load("return osGetSimulatorVersion").eval();
        if let Ok(mlua::Value::Function(_)) = result {
            panic!("osGetSimulatorVersion should not be registered at Low threat level")
        }

        // But osDrawText (None) should work
        let result: String = lua
            .load(r#"return osDrawText("", "hello")"#)
            .eval()
            .unwrap();
        assert_eq!(result, "Text hello; ");
    }

    // 5. Function in allowed list overrides threat level
    #[test]
    fn allowed_list_overrides_threat_level() {
        let lua = make_vm();
        let ctx = make_ctx();

        let mut allowed = HashSet::new();
        allowed.insert("osGetSimulatorVersion".to_string());

        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::None,
            allowed_functions: allowed,
            function_perms: HashMap::new(),
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        let version: String = lua.load("return osGetSimulatorVersion()").eval().unwrap();
        assert_eq!(version, "welkin 0.1.0");

        let result: LuaResult<mlua::Value> = lua.load("return osNpcCreate").eval();
        if let Ok(mlua::Value::Function(_)) = result {
            panic!("osNpcCreate should still be blocked")
        }
    }

    // 6. Registration doesn't panic
    #[test]
    fn registration_does_not_panic() {
        let ctx = make_ctx();
        for level in [
            OsslThreatLevel::None,
            OsslThreatLevel::Nuisance,
            OsslThreatLevel::VeryLow,
            OsslThreatLevel::Low,
            OsslThreatLevel::Moderate,
            OsslThreatLevel::High,
            OsslThreatLevel::VeryHigh,
            OsslThreatLevel::Severe,
        ] {
            let config = OsslConfig {
                max_threat_level: level,
                ..Default::default()
            };
            let fresh_lua = make_vm();
            register_ossl(&fresh_lua, &config, &ctx).unwrap();
        }
    }

    // 7. Full roundtrip: create VM, register OSSL at Moderate, verify availability
    #[test]
    fn full_roundtrip_moderate_level() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Moderate,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        // Functions at or below Moderate should be available:
        let result: String = lua
            .load(r#"return osDrawText("prefix;", "hello")"#)
            .eval()
            .unwrap();
        assert_eq!(result, "prefix;Text hello; ");

        let result: String = lua
            .load(r#"return osDrawLine("", 0, 0, 100, 100)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "MoveTo 0,0; LineTo 100,100; ");

        let result: String = lua
            .load(r#"return osDrawRectangle("", 50, 30)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "Rectangle 50,30; ");

        let result: String = lua
            .load(r#"return osDrawEllipse("", 20, 10)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "Ellipse 20,10; ");

        let result: String = lua
            .load(r#"return osFormatString("{0} is {1}", {"Hello", "world"})"#)
            .eval()
            .unwrap();
        assert_eq!(result, "Hello is world");

        let result = lua.load(r#"osSetSpeed("some-uuid", 2.0)"#).exec();
        assert!(result.is_ok());

        let speed: f64 = lua
            .load(r#"return osGetSpeed("some-uuid")"#)
            .eval()
            .unwrap();
        assert!((speed - 2.0).abs() < f64::EPSILON);

        let hour: f64 = lua.load("return osGetCurrentSunHour()").eval().unwrap();
        assert!((hour - 6.0).abs() < f64::EPSILON);

        // Functions above Moderate should NOT be available:
        let v: mlua::Value = lua.load("return osNpcCreate").eval().unwrap();
        assert!(
            matches!(v, mlua::Value::Nil),
            "osNpcCreate should not be registered"
        );

        let v: mlua::Value = lua.load("return osGetSimulatorVersion").eval().unwrap();
        assert!(
            matches!(v, mlua::Value::Nil),
            "osGetSimulatorVersion should not be registered"
        );

        let v: mlua::Value = lua.load("return osGetAgentIP").eval().unwrap();
        assert!(
            matches!(v, mlua::Value::Nil),
            "osGetAgentIP should not be registered"
        );

        let v: mlua::Value = lua.load("return osTeleportAgent").eval().unwrap();
        assert!(
            matches!(v, mlua::Value::Nil),
            "osTeleportAgent should not be registered"
        );

        let v: mlua::Value = lua.load("return osRegionRestart").eval().unwrap();
        assert!(
            matches!(v, mlua::Value::Nil),
            "osRegionRestart should not be registered"
        );

        let ver: String = lua.load("return OS_VERSION").eval().unwrap();
        assert_eq!(ver, "welkin 0.1.0");
    }

    // 8. osFormatString basic formatting
    #[test]
    fn os_format_string_basic() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::VeryLow,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        let r: String = lua
            .load(r#"return osFormatString("{0} {1}!", {"Hello", "World"})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "Hello World!");

        let r: String = lua
            .load(r#"return osFormatString("{0} and {0}", {"echo"})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "echo and echo");

        let r: String = lua
            .load(r#"return osFormatString("plain text", {})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "plain text");

        let r: String = lua
            .load(r#"return osFormatString("{1} before {0}", {"second", "first"})"#)
            .eval()
            .unwrap();
        assert_eq!(r, "first before second");
    }

    // 9. osMakeHash produces deterministic output
    #[test]
    fn os_make_hash_deterministic() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::VeryLow,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        let h1: String = lua.load(r#"return osMakeHash("test")"#).eval().unwrap();
        let h2: String = lua.load(r#"return osMakeHash("test")"#).eval().unwrap();
        assert_eq!(h1, h2, "same input must produce same hash");
        assert_eq!(h1.len(), 16, "hash should be 16 hex chars");

        let h3: String = lua
            .load(r#"return osMakeHash("different")"#)
            .eval()
            .unwrap();
        assert_ne!(h1, h3, "different inputs should produce different hashes");
    }

    // 10. Draw functions chain correctly
    #[test]
    fn draw_functions_chain() {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::None,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();

        let result: String = lua
            .load(
                r#"
                local dl = ""
                dl = osDrawLine(dl, 0, 0, 100, 100)
                dl = osDrawRectangle(dl, 50, 50)
                dl = osDrawText(dl, "label")
                return dl
            "#,
            )
            .eval()
            .unwrap();

        assert_eq!(
            result,
            "MoveTo 0,0; LineTo 100,100; Rectangle 50,50; Text label; "
        );
    }

    // 11. function_catalog returns all functions
    #[test]
    fn function_catalog_completeness() {
        let catalog = function_catalog();
        // Tasks 8.1–12.1 added ~138 new stubs on top of the base ~30
        assert!(
            catalog.len() >= 160,
            "expected at least 160 OSSL functions, got {}",
            catalog.len()
        );

        let names: Vec<&str> = catalog.iter().map(|(n, _)| *n).collect();
        // Original functions
        assert!(names.contains(&"osGetSimulatorVersion"));
        assert!(names.contains(&"osNpcCreate"));
        assert!(names.contains(&"osDrawText"));
        assert!(names.contains(&"osFormatString"));
        assert!(names.contains(&"osRegionRestart"));
        // Task 8.1 NPC stubs
        assert!(names.contains(&"osIsNpc"));
        assert!(names.contains(&"osNpcSaveAppearance"));
        assert!(names.contains(&"osNpcLookAt"));
        // Task 9.1 agent stubs
        assert!(names.contains(&"osGetAgents"));
        assert!(names.contains(&"osGetHealth"));
        assert!(names.contains(&"osKickAvatar"));
        assert!(names.contains(&"osTeleportObject"));
        // Task 10.1 terrain stubs
        assert!(names.contains(&"osTerrainSetHeight"));
        assert!(names.contains(&"osGetRegionWaterHeight"));
        assert!(names.contains(&"osParcelSetDetails"));
        // Task 11.1 inventory stubs
        assert!(names.contains(&"osGiveInventory"));
        assert!(names.contains(&"osPlaySound"));
        assert!(names.contains(&"osForceCreateLink"));
        assert!(names.contains(&"osGetLinkNumber"));
        // Task 12.1 misc stubs
        assert!(names.contains(&"osLoadedCreationDate"));
        assert!(names.contains(&"osGetInertiaData"));
        assert!(names.contains(&"osConsoleCommand"));
        assert!(names.contains(&"osListenRegex"));
    }

    // 12. Default config blocks High functions
    #[test]
    fn default_config_blocks_high() {
        let config = OsslConfig::default();
        assert_eq!(config.max_threat_level, OsslThreatLevel::VeryLow);

        assert!(config.is_allowed_simple("osDrawText", OsslThreatLevel::None));
        assert!(config.is_allowed_simple("osFormatString", OsslThreatLevel::VeryLow));
        assert!(!config.is_allowed_simple("osSetSpeed", OsslThreatLevel::Moderate));
        assert!(!config.is_allowed_simple("osNpcCreate", OsslThreatLevel::High));
        assert!(!config.is_allowed_simple("osGetAgentIP", OsslThreatLevel::Severe));
    }

    // -- Task 1.1: Per-function permissions with roles --

    #[test]
    fn per_function_perm_with_roles() {
        let mut config = OsslConfig::default();
        config.function_perms.insert(
            "osNpcCreate".to_string(),
            FunctionPerm {
                threat_level: OsslThreatLevel::High,
                allowed_roles: vec![OsslRole::EstateManager, OsslRole::EstateOwner],
            },
        );

        // Estate manager should be allowed
        assert!(config.is_allowed(
            "osNpcCreate",
            OsslThreatLevel::High,
            Some(OsslRole::EstateManager)
        ));
        // Estate owner too
        assert!(config.is_allowed(
            "osNpcCreate",
            OsslThreatLevel::High,
            Some(OsslRole::EstateOwner)
        ));
        // Anyone denied
        assert!(!config.is_allowed("osNpcCreate", OsslThreatLevel::High, Some(OsslRole::Anyone)));
        // No role denied
        assert!(!config.is_allowed("osNpcCreate", OsslThreatLevel::High, Option::None));
    }

    // -- Task 1.2: INI parsing --

    #[test]
    fn parse_ini_threat_level() {
        let ini = "OSFunctionThreatLevel = Moderate\n";
        let config = parse_ossl_ini(ini);
        assert_eq!(config.max_threat_level, OsslThreatLevel::Moderate);
    }

    #[test]
    fn parse_ini_allow_true() {
        let ini = "Allow_osGetSimulatorVersion = true\n";
        let config = parse_ossl_ini(ini);
        assert!(config.allowed_functions.contains("osGetSimulatorVersion"));
    }

    #[test]
    fn parse_ini_allow_roles() {
        let ini = "Allow_osNpcCreate = ESTATE_MANAGER,ESTATE_OWNER\n";
        let config = parse_ossl_ini(ini);
        let perm = config.function_perms.get("osNpcCreate").unwrap();
        assert_eq!(perm.allowed_roles.len(), 2);
        assert!(perm.allowed_roles.contains(&OsslRole::EstateManager));
        assert!(perm.allowed_roles.contains(&OsslRole::EstateOwner));
    }

    #[test]
    fn parse_ini_macro_expansion() {
        let ini = "Allow_osNpcCreate = ${OSSL|osslParcelO}\n";
        let config = parse_ossl_ini(ini);
        let perm = config.function_perms.get("osNpcCreate").unwrap();
        assert!(perm.allowed_roles.contains(&OsslRole::ParcelOwner));
    }

    #[test]
    fn parse_ini_macro_parcel_og() {
        let ini = "Allow_osNpcCreate = ${OSSL|osslParcelOG}\n";
        let config = parse_ossl_ini(ini);
        let perm = config.function_perms.get("osNpcCreate").unwrap();
        assert_eq!(perm.allowed_roles.len(), 2);
        assert!(perm.allowed_roles.contains(&OsslRole::ParcelOwner));
        assert!(perm.allowed_roles.contains(&OsslRole::ParcelGroupMember));
    }

    #[test]
    fn parse_ini_malformed_skipped() {
        let ini = "\
; comment line
bad line without equals
OSFunctionThreatLevel = Low
Allow_osDrawText = true
";
        let config = parse_ossl_ini(ini);
        assert_eq!(config.max_threat_level, OsslThreatLevel::Low);
        assert!(config.allowed_functions.contains("osDrawText"));
    }

    #[test]
    fn parse_ini_unknown_threat_level() {
        let ini = "OSFunctionThreatLevel = SuperDuper\n";
        let config = parse_ossl_ini(ini);
        // Falls back to default (VeryLow)
        assert_eq!(config.max_threat_level, OsslThreatLevel::VeryLow);
    }

    // -- Tasks 8.1–12.1: stub registration tests --

    /// Helper: create VM with all functions registered at Severe level
    fn vm_all_enabled() -> (Lua, Arc<RwLock<ScriptContext>>) {
        let lua = make_vm();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Severe,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();
        (lua, ctx)
    }

    // Task 8.1: NPC stubs
    #[test]
    fn npc_stubs_no_panic() {
        let (lua, _ctx) = vm_all_enabled();
        // Verify key NPC functions are registered and callable
        let r: i32 = lua.load(r#"return osIsNpc("some-key")"#).eval().unwrap();
        assert_eq!(r, 0);

        let r: String = lua.load(r#"return osNpcGetPos("npc-key")"#).eval().unwrap();
        assert_eq!(r, "<0, 0, 0>");

        let r: String = lua.load(r#"return osNpcGetRot("npc-key")"#).eval().unwrap();
        assert_eq!(r, "<0, 0, 0, 1>");

        let r: String = lua
            .load(r#"return osNpcGetOwner("npc-key")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "00000000-0000-0000-0000-000000000000");

        // Void stubs should not panic
        lua.load(r#"osNpcLoadAppearance("npc", "notecard")"#)
            .exec()
            .unwrap();
        lua.load(r#"osNpcSetRot("npc", "<0,0,0,1>")"#)
            .exec()
            .unwrap();
        lua.load(r#"osNpcStand("npc")"#).exec().unwrap();
        lua.load(r#"osNpcPlayAnimation("npc", "walk")"#)
            .exec()
            .unwrap();
        let r: i32 = lua.load(r#"return osNpcLookAt("12345678-1234-1234-1234-123456789abc", 1, "00000000-0000-0000-0000-000000000000", "<128,128,25>")"#)
            .eval()
            .unwrap();
        // Returns -3 because the test context has no NPC registered.
        assert_eq!(r, -3);
    }

    // Task 9.1: Agent functions (real implementations)
    #[test]
    fn agent_funcs_handle_invalid_uuids() {
        let (lua, _ctx) = vm_all_enabled();

        // Invalid UUID -> -1 (agent not found)
        let health: f64 = lua
            .load(r#"return osGetHealth("avatar-key")"#)
            .eval()
            .unwrap();
        assert!((health - (-1.0)).abs() < f64::EPSILON);

        // Invalid UUID -> 0 (default heal rate)
        let heal_rate: f64 = lua
            .load(r#"return osGetHealRate("avatar-key")"#)
            .eval()
            .unwrap();
        assert!((heal_rate - 0.0).abs() < f64::EPSILON);

        // Invalid UUID -> -1 (parse failure)
        let avatar_type: i32 = lua
            .load(r#"return osAvatarType("avatar-key")"#)
            .eval()
            .unwrap();
        assert_eq!(avatar_type, -1);

        // Invalid UUID -> "unknown"
        let gender: String = lua
            .load(r#"return osGetGender("avatar-key")"#)
            .eval()
            .unwrap();
        assert_eq!(gender, "unknown");

        // Invalid UUID -> -1 (parse failure)
        let tp_result: i32 = lua
            .load(r#"return osTeleportObject("obj", 1000, 1000, "<128,128,25>", "<0,0,0,1>")"#)
            .eval()
            .unwrap();
        assert_eq!(tp_result, -1);

        // Void functions with invalid UUIDs: no-ops, must not panic
        lua.load(r#"osCauseHealing("avatar", 10.0)"#)
            .exec()
            .unwrap();
        lua.load(r#"osCauseDamage("avatar", 10.0)"#).exec().unwrap();
        lua.load(r#"osKickAvatar("First", "Last", "reason")"#)
            .exec()
            .unwrap();
    }

    // Task 10.1: Terrain/environment/parcel functions (real implementations)
    #[test]
    fn terrain_functions_real() {
        let (lua, _ctx) = vm_all_enabled();

        // Default terrain height is 21.0 (matches welkin-scene DEFAULT_TERRAIN_HEIGHT)
        let h: f64 = lua
            .load("return osTerrainGetHeight(128, 128)")
            .eval()
            .unwrap();
        assert!((h - 21.0).abs() < f64::EPSILON);

        let h2: f64 = lua
            .load("return osGetTerrainHeight(64, 64)")
            .eval()
            .unwrap();
        assert!((h2 - 21.0).abs() < f64::EPSILON);

        let water: f64 = lua.load("return osGetRegionWaterHeight()").eval().unwrap();
        assert!((water - 20.0).abs() < f64::EPSILON);

        // day_length defaults to 14400 seconds (4 real-time hours)
        let sun: f64 = lua
            .load(r#"return osSunGetParam("day_length")"#)
            .eval()
            .unwrap();
        assert!((sun - 14400.0).abs() < f64::EPSILON);

        let wind: f64 = lua
            .load(r#"return osGetWindParam("SimpleRandomWind", "strength")"#)
            .eval()
            .unwrap();
        assert!((wind - 0.0).abs() < f64::EPSILON);

        let dwell: f64 = lua
            .load(r#"return osGetParcelDwell("<128,128,0>")"#)
            .eval()
            .unwrap();
        assert!((dwell - 0.0).abs() < f64::EPSILON);

        // Terrain set returns 1 on success and modifies the heightmap
        let r: i32 = lua
            .load("return osTerrainSetHeight(128, 128, 30.0)")
            .eval()
            .unwrap();
        assert_eq!(r, 1);
        let h3: f64 = lua
            .load("return osTerrainGetHeight(128, 128)")
            .eval()
            .unwrap();
        assert!((h3 - 30.0).abs() < f64::EPSILON);

        lua.load("osTerrainFlush()").exec().unwrap();
    }

    // Task 11.1: Inventory/attachment/sound/link stubs
    #[test]
    fn inventory_stubs_no_panic() {
        let (lua, _ctx) = vm_all_enabled();

        let desc: String = lua
            .load(r#"return osGetInventoryDesc("item")"#)
            .eval()
            .unwrap();
        assert_eq!(desc, "");

        let key: String = lua
            .load(r#"return osGetInventoryItemKey("item")"#)
            .eval()
            .unwrap();
        assert_eq!(key, "00000000-0000-0000-0000-000000000000");

        let num: i32 = lua.load("return osGetInventoryNumber(0)").eval().unwrap();
        assert_eq!(num, 0);

        let link_num: i32 = lua
            .load(r#"return osGetLinkNumber("child")"#)
            .eval()
            .unwrap();
        assert_eq!(link_num, -1);

        let rezzing: String = lua.load("return osGetRezzingObject()").eval().unwrap();
        assert_eq!(rezzing, "00000000-0000-0000-0000-000000000000");

        let prim_count: i32 = lua
            .load(r#"return osGetPrimCount("<128,128,0>")"#)
            .eval()
            .unwrap();
        // Default prim_count is 1 (host object has at least 1 prim).
        assert_eq!(prim_count, 1);

        let sitting: i32 = lua
            .load(r#"return osGetSittingAvatarsCount("prim-key")"#)
            .eval()
            .unwrap();
        assert_eq!(sitting, 0);

        // Sound stubs
        lua.load(r#"osPlaySound(0, "sound-uuid", 1.0)"#)
            .exec()
            .unwrap();
        lua.load("osStopSound(0)").exec().unwrap();

        // Link stubs
        lua.load(r#"osForceCreateLink("target", 1)"#)
            .exec()
            .unwrap();
        lua.load("osForceBreakAllLinks()").exec().unwrap();
    }

    // Task 12.1: Remaining misc stubs
    #[test]
    fn misc_stubs_no_panic() {
        let (lua, _ctx) = vm_all_enabled();

        let date: String = lua.load("return osLoadedCreationDate()").eval().unwrap();
        assert_eq!(date, "");

        let id: String = lua.load("return osLoadedCreationID()").eval().unwrap();
        // Default region_loaded_creation_id is empty.
        assert_eq!(id, "");

        let appearance: String = lua
            .load(r#"return osOwnerSaveAppearance("notecard")"#)
            .eval()
            .unwrap();
        assert_eq!(appearance, "00000000-0000-0000-0000-000000000000");

        let map: String = lua
            .load(r#"return osGetRegionMapTexture("Test Region")"#)
            .eval()
            .unwrap();
        assert_eq!(map, "00000000-0000-0000-0000-000000000000");

        let invite: i32 = lua
            .load(r#"return osInviteToGroup("agent-key")"#)
            .eval()
            .unwrap();
        assert_eq!(invite, 0);

        let listen: i32 = lua
            .load(r#"return osListenRegex(0, "", "", "pattern", 0)"#)
            .eval()
            .unwrap();
        // osListenRegex returns a positive handle (next_listen_handle starts at 1).
        assert!(listen > 0);

        let color: String = lua.load("return osGetLinkColor(0, 0)").eval().unwrap();
        // Default color is white <1, 1, 1>.
        assert_eq!(color, "<1, 1, 1>");

        let name: String = lua.load(r#"return osKey2Name("some-key")"#).eval().unwrap();
        assert_eq!(name, "");

        // Void stubs
        lua.load("osClearInertia()").exec().unwrap();
        lua.load("osClearObjectAnimations()").exec().unwrap();
        lua.load("osResetAllScripts()").exec().unwrap();
        lua.load(r#"osConsoleCommand("help")"#).exec().unwrap();
    }

    // Verify no duplicate function names in the catalog
    #[test]
    fn no_duplicate_function_names() {
        let catalog = function_catalog();
        let mut seen = HashSet::new();
        for (name, _level) in &catalog {
            assert!(seen.insert(*name), "duplicate OSSL function name: {name}");
        }
    }
}
