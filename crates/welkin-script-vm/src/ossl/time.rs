// SPDX-License-Identifier: GPL-3.0-or-later
//! OSSL time functions (threat level None).

use std::sync::Arc;

use chrono::{DateTime, FixedOffset, TimeZone, Timelike, Utc};
use mlua::prelude::*;
use parking_lot::RwLock;

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;

pub(crate) fn time_functions() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osUnixTimeToTimestamp",
            level: OsslThreatLevel::None,
            register: register_os_unix_time_to_timestamp,
        },
        OsslFunc {
            name: "osGetPSTWallclock",
            level: OsslThreatLevel::None,
            register: register_os_get_pst_wallclock,
        },
        OsslFunc {
            name: "osGetApparentTime",
            level: OsslThreatLevel::None,
            register: register_os_get_apparent_time,
        },
        OsslFunc {
            name: "osGetApparentTimeString",
            level: OsslThreatLevel::None,
            register: register_os_get_apparent_time_string,
        },
        OsslFunc {
            name: "osGetApparentRegionTime",
            level: OsslThreatLevel::None,
            register: register_os_get_apparent_region_time,
        },
        OsslFunc {
            name: "osGetApparentRegionTimeString",
            level: OsslThreatLevel::None,
            register: register_os_get_apparent_region_time_string,
        },
    ]
}

/// PST offset: UTC-8
const PST_OFFSET_SECONDS: i32 = -8 * 3600;

fn pst_offset() -> FixedOffset {
    FixedOffset::west_opt(-PST_OFFSET_SECONDS).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap())
}

/// Seconds since midnight for a given chrono DateTime.
fn seconds_since_midnight<Tz: TimeZone>(dt: &DateTime<Tz>) -> f64 {
    let h = dt.hour() as f64;
    let m = dt.minute() as f64;
    let s = dt.second() as f64;
    h * 3600.0 + m * 60.0 + s
}

// ---------------------------------------------------------------------------
// osUnixTimeToTimestamp
// ---------------------------------------------------------------------------

fn register_os_unix_time_to_timestamp(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, epoch: i64| match DateTime::from_timestamp(epoch, 0) {
        Some(dt) => Ok(dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        None => Ok(String::new()),
    })?;
    lua.globals().set("osUnixTimeToTimestamp", f)
}

// ---------------------------------------------------------------------------
// osGetPSTWallclock — seconds since midnight PST
// ---------------------------------------------------------------------------

fn register_os_get_pst_wallclock(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| {
        let now_pst = Utc::now().with_timezone(&pst_offset());
        Ok(seconds_since_midnight(&now_pst))
    })?;
    lua.globals().set("osGetPSTWallclock", f)
}

// ---------------------------------------------------------------------------
// osGetApparentTime / osGetApparentTimeString
// ---------------------------------------------------------------------------

fn register_os_get_apparent_time(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| {
        let now = Utc::now();
        Ok(seconds_since_midnight(&now))
    })?;
    lua.globals().set("osGetApparentTime", f)
}

fn register_os_get_apparent_time_string(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| {
        let now = Utc::now();
        Ok(now.format("%H:%M:%S").to_string())
    })?;
    lua.globals().set("osGetApparentTimeString", f)
}

// ---------------------------------------------------------------------------
// osGetApparentRegionTime / osGetApparentRegionTimeString
// ---------------------------------------------------------------------------

fn register_os_get_apparent_region_time(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    // For now, region time = UTC
    let f = lua.create_function(|_, ()| {
        let now = Utc::now();
        Ok(seconds_since_midnight(&now))
    })?;
    lua.globals().set("osGetApparentRegionTime", f)
}

fn register_os_get_apparent_region_time_string(
    lua: &Lua,
    _ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let f = lua.create_function(|_, ()| {
        let now = Utc::now();
        Ok(now.format("%H:%M:%S").to_string())
    })?;
    lua.globals().set("osGetApparentRegionTimeString", f)
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
            ..ScriptContext::default()
        }))
    }

    fn make_time_vm() -> Lua {
        let lua = Lua::new();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::None,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();
        lua
    }

    #[test]
    fn test_unix_time_to_timestamp() {
        let lua = make_time_vm();
        let r: String = lua.load("return osUnixTimeToTimestamp(0)").eval().unwrap();
        assert_eq!(r, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_unix_time_to_timestamp_specific() {
        let lua = make_time_vm();
        // 2024-01-01 00:00:00 UTC = 1704067200
        let r: String = lua
            .load("return osUnixTimeToTimestamp(1704067200)")
            .eval()
            .unwrap();
        assert_eq!(r, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_pst_wallclock_returns_number() {
        let lua = make_time_vm();
        let r: f64 = lua.load("return osGetPSTWallclock()").eval().unwrap();
        // Should be between 0 and 86400 (seconds in a day)
        assert!((0.0..86400.0).contains(&r));
    }

    #[test]
    fn test_apparent_time_returns_number() {
        let lua = make_time_vm();
        let r: f64 = lua.load("return osGetApparentTime()").eval().unwrap();
        assert!((0.0..86400.0).contains(&r));
    }

    #[test]
    fn test_apparent_time_string_format() {
        let lua = make_time_vm();
        let r: String = lua.load("return osGetApparentTimeString()").eval().unwrap();
        // Should match HH:MM:SS format
        assert_eq!(r.len(), 8);
        assert_eq!(r.chars().nth(2), Some(':'));
        assert_eq!(r.chars().nth(5), Some(':'));
    }

    #[test]
    fn test_apparent_region_time_string_format() {
        let lua = make_time_vm();
        let r: String = lua
            .load("return osGetApparentRegionTimeString()")
            .eval()
            .unwrap();
        assert_eq!(r.len(), 8);
        assert_eq!(r.chars().nth(2), Some(':'));
    }
}
