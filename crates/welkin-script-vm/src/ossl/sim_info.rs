// SPDX-License-Identifier: GPL-3.0-or-later
//! OSSL sim/grid info functions (task 7.1).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;

pub(crate) fn sim_info_functions() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osGetScriptEngineName",
            level: OsslThreatLevel::None,
            register: register_os_get_script_engine_name,
        },
        OsslFunc {
            name: "osCheckODE",
            level: OsslThreatLevel::None,
            register: register_os_check_ode,
        },
        OsslFunc {
            name: "osGetPhysicsEngineType",
            level: OsslThreatLevel::None,
            register: register_os_get_physics_engine_type,
        },
        OsslFunc {
            name: "osGetPhysicsEngineName",
            level: OsslThreatLevel::None,
            register: register_os_get_physics_engine_name,
        },
        OsslFunc {
            name: "osGetRegionSize",
            level: OsslThreatLevel::None,
            register: register_os_get_region_size,
        },
        OsslFunc {
            name: "osGetSimulatorMemory",
            level: OsslThreatLevel::Moderate,
            register: register_os_get_simulator_memory,
        },
        OsslFunc {
            name: "osGetSimulatorMemoryKB",
            level: OsslThreatLevel::Moderate,
            register: register_os_get_simulator_memory_kb,
        },
        OsslFunc {
            name: "osTemperature2sRGB",
            level: OsslThreatLevel::None,
            register: register_os_temperature_to_srgb,
        },
        // Grid info stubs
        OsslFunc {
            name: "osGetGridNick",
            level: OsslThreatLevel::None,
            register: register_os_get_grid_nick,
        },
        OsslFunc {
            name: "osGetGridName",
            level: OsslThreatLevel::None,
            register: register_os_get_grid_name,
        },
        OsslFunc {
            name: "osGetGridLoginURI",
            level: OsslThreatLevel::None,
            register: register_os_get_grid_login_uri,
        },
        OsslFunc {
            name: "osGetGridHomeURI",
            level: OsslThreatLevel::None,
            register: register_os_get_grid_home_uri,
        },
        OsslFunc {
            name: "osGetGridGatekeeperURI",
            level: OsslThreatLevel::None,
            register: register_os_get_grid_gatekeeper_uri,
        },
        OsslFunc {
            name: "osGetGridCustom",
            level: OsslThreatLevel::None,
            register: register_os_get_grid_custom,
        },
        OsslFunc {
            name: "osGetRegionStats",
            level: OsslThreatLevel::Moderate,
            register: register_os_get_region_stats,
        },
        OsslFunc {
            name: "osGetMapTexture",
            level: OsslThreatLevel::None,
            register: register_os_get_map_texture,
        },
    ]
}

// ---------------------------------------------------------------------------
// Engine identity
// ---------------------------------------------------------------------------

fn register_os_get_script_engine_name(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| Ok("welkin-luau".to_string()))?;
    lua.globals().set("osGetScriptEngineName", f)
}

fn register_os_check_ode(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    // We use Rapier, not ODE.
    let f = lua.create_function(|_, ()| Ok(0i32))?;
    lua.globals().set("osCheckODE", f)
}

fn register_os_get_physics_engine_type(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| Ok("Rapier".to_string()))?;
    lua.globals().set("osGetPhysicsEngineType", f)
}

fn register_os_get_physics_engine_name(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| Ok("welkin-physics-rapier".to_string()))?;
    lua.globals().set("osGetPhysicsEngineName", f)
}

// ---------------------------------------------------------------------------
// Region size
// ---------------------------------------------------------------------------

fn register_os_get_region_size(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| {
        // Default region size: 256x256x4096
        Ok(vec![256.0f64, 256.0, 4096.0])
    })?;
    lua.globals().set("osGetRegionSize", f)
}

// ---------------------------------------------------------------------------
// Simulator memory — read from /proc/self/status on Linux
// ---------------------------------------------------------------------------

fn get_rss_kb() -> i64 {
    // Try reading VmRSS from /proc/self/status
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let trimmed = rest.trim();
                if let Some(kb_str) = trimmed.strip_suffix("kB")
                    && let Ok(kb) = kb_str.trim().parse::<i64>()
                {
                    return kb;
                }
            }
        }
    }
    // Fallback: return 0
    0
}

fn register_os_get_simulator_memory(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| Ok(get_rss_kb() * 1024))?;
    lua.globals().set("osGetSimulatorMemory", f)
}

fn register_os_get_simulator_memory_kb(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| Ok(get_rss_kb()))?;
    lua.globals().set("osGetSimulatorMemoryKB", f)
}

// ---------------------------------------------------------------------------
// osTemperature2sRGB — color temperature to approximate sRGB
// ---------------------------------------------------------------------------

/// Attempt the Tanner Helland algorithm for color temperature to RGB.
/// Temperature in Kelvin (1000..40000).
fn temperature_to_srgb(kelvin: f64) -> (u8, u8, u8) {
    let temp = kelvin.clamp(1000.0, 40000.0) / 100.0;

    let r = if temp <= 66.0 {
        255.0
    } else {
        let r = 329.698727446 * (temp - 60.0).powf(-0.1332047592);
        r.clamp(0.0, 255.0)
    };

    let g = if temp <= 66.0 {
        let g = 99.4708025861 * temp.ln() - 161.1195681661;
        g.clamp(0.0, 255.0)
    } else {
        let g = 288.1221695283 * (temp - 60.0).powf(-0.0755148492);
        g.clamp(0.0, 255.0)
    };

    let b = if temp >= 66.0 {
        255.0
    } else if temp <= 19.0 {
        0.0
    } else {
        let b = 138.5177312231 * (temp - 10.0).ln() - 305.0447927307;
        b.clamp(0.0, 255.0)
    };

    (r as u8, g as u8, b as u8)
}

fn register_os_temperature_to_srgb(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, kelvin: f64| {
        let (r, g, b) = temperature_to_srgb(kelvin);
        Ok(format!("{r},{g},{b}"))
    })?;
    lua.globals().set("osTemperature2sRGB", f)
}

// ---------------------------------------------------------------------------
// Grid info stubs
// ---------------------------------------------------------------------------

fn register_os_get_grid_nick(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        Ok(c.grid_nick.clone())
    })?;
    lua.globals().set("osGetGridNick", f)
}

fn register_os_get_grid_name(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        Ok(c.grid_name.clone())
    })?;
    lua.globals().set("osGetGridName", f)
}

fn register_os_get_grid_login_uri(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        Ok(c.grid_login_uri.clone())
    })?;
    lua.globals().set("osGetGridLoginURI", f)
}

fn register_os_get_grid_home_uri(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        Ok(c.grid_home_uri.clone())
    })?;
    lua.globals().set("osGetGridHomeURI", f)
}

fn register_os_get_grid_gatekeeper_uri(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        Ok(c.grid_gatekeeper_uri.clone())
    })?;
    lua.globals().set("osGetGridGatekeeperURI", f)
}

fn register_os_get_grid_custom(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, key: String| {
        let c = ctx.read();
        Ok(c.grid_custom.get(&key).cloned().unwrap_or_default())
    })?;
    lua.globals().set("osGetGridCustom", f)
}

// ---------------------------------------------------------------------------
// Region stats / map texture
// ---------------------------------------------------------------------------

fn register_os_get_region_stats(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        // Return stats in the same order as OpenSim: TimeDilation, SimFPS, PhysFPS,
        // AgentUpdates, RootAgents, ChildAgents, TotalPrim, ActivePrim, ...
        let stats: Vec<f64> = vec![
            c.time_dilation,
            c.sim_fps,
            c.physics_fps,
            0.0, // agent updates per sec
            c.presence_list.iter().filter(|p| !p.is_npc).count() as f64, // root agents
            0.0, // child agents
            c.total_prims as f64,
            c.total_prims as f64, // active prims (same as total for now)
            0.0, // frame time ms
            0.0, // net time ms
            0.0, // physics time ms
            0.0, // simulation time ms
            0.0, // agent time ms
            0.0, // image time ms
            0.0, // other time ms
            c.active_scripts as f64,
            c.script_lps,
            0.0, // packets in
            0.0, // packets out
            0.0, // pending downloads
            0.0, // pending uploads
            0.0, // unacked bytes
        ];
        Ok(stats)
    })?;
    lua.globals().set("osGetRegionStats", f)
}

fn register_os_get_map_texture(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        Ok(c.region_map_texture.to_string())
    })?;
    lua.globals().set("osGetMapTexture", f)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::ScriptContext;
    use crate::ossl::{OsslConfig, OsslThreatLevel, register_ossl};
    use uuid::Uuid;

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            grid_nick: "test-grid".to_string(),
            grid_name: "Test Grid".to_string(),
            grid_login_uri: "http://localhost:8002".to_string(),
            grid_home_uri: "http://localhost:8002".to_string(),
            grid_gatekeeper_uri: "http://localhost:8002".to_string(),
            sim_fps: 55.0,
            physics_fps: 55.0,
            time_dilation: 0.98,
            total_prims: 1500,
            active_scripts: 42,
            script_lps: 100.0,
            ..ScriptContext::default()
        }))
    }

    fn make_sim_vm() -> Lua {
        let lua = Lua::new();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Moderate,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();
        lua
    }

    #[test]
    fn test_script_engine_name() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetScriptEngineName()").eval().unwrap();
        assert_eq!(r, "welkin-luau");
    }

    #[test]
    fn test_check_ode() {
        let lua = make_sim_vm();
        let r: i32 = lua.load("return osCheckODE()").eval().unwrap();
        assert_eq!(r, 0);
    }

    #[test]
    fn test_physics_engine_type() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetPhysicsEngineType()").eval().unwrap();
        assert_eq!(r, "Rapier");
    }

    #[test]
    fn test_physics_engine_name() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetPhysicsEngineName()").eval().unwrap();
        assert_eq!(r, "welkin-physics-rapier");
    }

    #[test]
    fn test_region_size() {
        let lua = make_sim_vm();
        let r: Vec<f64> = lua.load("return osGetRegionSize()").eval().unwrap();
        assert_eq!(r, vec![256.0, 256.0, 4096.0]);
    }

    #[test]
    fn test_simulator_memory_returns_number() {
        let lua = make_sim_vm();
        let r: i64 = lua.load("return osGetSimulatorMemory()").eval().unwrap();
        // On Linux with /proc, should be > 0. On other platforms, 0 is acceptable.
        assert!(r >= 0);
    }

    #[test]
    fn test_simulator_memory_kb() {
        let lua = make_sim_vm();
        let r: i64 = lua.load("return osGetSimulatorMemoryKB()").eval().unwrap();
        assert!(r >= 0);
    }

    #[test]
    fn test_temperature_to_srgb() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osTemperature2sRGB(6500)").eval().unwrap();
        // Should be close to white at 6500K
        let parts: Vec<u8> = r.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(parts.len(), 3);
        // At 6500K, all channels should be near 255
        assert!(parts[0] > 200);
        assert!(parts[1] > 200);
        assert!(parts[2] > 200);
    }

    #[test]
    fn test_temperature_warm() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osTemperature2sRGB(2000)").eval().unwrap();
        let parts: Vec<u8> = r.split(',').map(|s| s.parse().unwrap()).collect();
        // At 2000K, should be warm (high R, lower G, very low B)
        assert_eq!(parts[0], 255); // R should be max
        assert!(parts[2] < parts[0]); // B should be much less than R
    }

    #[test]
    fn test_grid_nick() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetGridNick()").eval().unwrap();
        assert_eq!(r, "test-grid");
    }

    #[test]
    fn test_grid_name() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetGridName()").eval().unwrap();
        assert_eq!(r, "Test Grid");
    }

    #[test]
    fn test_grid_login_uri() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetGridLoginURI()").eval().unwrap();
        assert_eq!(r, "http://localhost:8002");
    }

    #[test]
    fn test_grid_home_uri() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetGridHomeURI()").eval().unwrap();
        assert_eq!(r, "http://localhost:8002");
    }

    #[test]
    fn test_grid_gatekeeper_uri() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetGridGatekeeperURI()").eval().unwrap();
        assert_eq!(r, "http://localhost:8002");
    }

    #[test]
    fn test_grid_custom_returns_empty() {
        let lua = make_sim_vm();
        let r: String = lua
            .load(r#"return osGetGridCustom("anything")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "");
    }

    #[test]
    fn test_grid_custom_returns_value() {
        let lua = Lua::new();
        let ctx = make_ctx();
        {
            let mut c = ctx.write();
            c.grid_custom.insert("currency".to_string(), "G$".to_string());
        }
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Moderate,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();
        let r: String = lua
            .load(r#"return osGetGridCustom("currency")"#)
            .eval()
            .unwrap();
        assert_eq!(r, "G$");
    }

    #[test]
    fn test_region_stats_returns_vec() {
        let lua = make_sim_vm();
        let r: Vec<f64> = lua.load("return osGetRegionStats()").eval().unwrap();
        assert_eq!(r.len(), 22);
        // time_dilation
        assert!((r[0] - 0.98).abs() < 0.01);
        // sim_fps
        assert!((r[1] - 55.0).abs() < 0.01);
        // physics_fps
        assert!((r[2] - 55.0).abs() < 0.01);
        // total_prims
        assert!((r[6] - 1500.0).abs() < 0.01);
        // active_scripts
        assert!((r[15] - 42.0).abs() < 0.01);
        // script_lps
        assert!((r[16] - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_map_texture_returns_nil_uuid() {
        let lua = make_sim_vm();
        let r: String = lua.load("return osGetMapTexture()").eval().unwrap();
        assert_eq!(r, "00000000-0000-0000-0000-000000000000");
    }
}
