// SPDX-License-Identifier: GPL-3.0-or-later
//! Remaining stub functions (Task 12.1): notecard extras, appearance,
//! inertia, sit, and miscellaneous.

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::ScriptContext;
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn ossl_misc_funcs() -> Vec<OsslFunc> {
    vec![
        // Notecard extras
        OsslFunc {
            name: "osLoadedCreationDate",
            level: OsslThreatLevel::Low,
            register: register_os_loaded_creation_date,
        },
        OsslFunc {
            name: "osLoadedCreationTime",
            level: OsslThreatLevel::Low,
            register: register_os_loaded_creation_time,
        },
        OsslFunc {
            name: "osLoadedCreationID",
            level: OsslThreatLevel::Low,
            register: register_os_loaded_creation_id,
        },
        // Appearance (VeryHigh)
        OsslFunc {
            name: "osOwnerSaveAppearance",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_owner_save_appearance,
        },
        OsslFunc {
            name: "osAgentSaveAppearance",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_agent_save_appearance,
        },
        OsslFunc {
            name: "osGetRegionMapTexture",
            level: OsslThreatLevel::High,
            register: register_os_get_region_map_texture,
        },
        // Inertia (None)
        OsslFunc {
            name: "osGetInertiaData",
            level: OsslThreatLevel::None,
            register: register_os_get_inertia_data,
        },
        OsslFunc {
            name: "osSetInertia",
            level: OsslThreatLevel::None,
            register: register_os_set_inertia,
        },
        OsslFunc {
            name: "osSetInertiaAsBox",
            level: OsslThreatLevel::None,
            register: register_os_set_inertia_as_box,
        },
        OsslFunc {
            name: "osSetInertiaAsSphere",
            level: OsslThreatLevel::None,
            register: register_os_set_inertia_as_sphere,
        },
        OsslFunc {
            name: "osSetInertiaAsCylinder",
            level: OsslThreatLevel::None,
            register: register_os_set_inertia_as_cylinder,
        },
        OsslFunc {
            name: "osClearInertia",
            level: OsslThreatLevel::None,
            register: register_os_clear_inertia,
        },
        // Sit
        OsslFunc {
            name: "osSitOnLink",
            level: OsslThreatLevel::Moderate,
            register: register_os_sit_on_link,
        },
        // Misc
        OsslFunc {
            name: "osClearObjectAnimations",
            level: OsslThreatLevel::VeryLow,
            register: register_os_clear_object_animations,
        },
        OsslFunc {
            name: "osResetAllScripts",
            level: OsslThreatLevel::Moderate,
            register: register_os_reset_all_scripts,
        },
        OsslFunc {
            name: "osParticleSystem",
            level: OsslThreatLevel::VeryLow,
            register: register_os_particle_system,
        },
        OsslFunc {
            name: "osLinkParticleSystem",
            level: OsslThreatLevel::VeryLow,
            register: register_os_link_particle_system,
        },
        OsslFunc {
            name: "osInviteToGroup",
            level: OsslThreatLevel::VeryLow,
            register: register_os_invite_to_group,
        },
        OsslFunc {
            name: "osEjectFromGroup",
            level: OsslThreatLevel::VeryLow,
            register: register_os_eject_from_group,
        },
        OsslFunc {
            name: "osConsoleCommand",
            level: OsslThreatLevel::Severe,
            register: register_os_console_command,
        },
        OsslFunc {
            name: "osRequestURL",
            level: OsslThreatLevel::Moderate,
            register: register_os_request_url,
        },
        OsslFunc {
            name: "osRequestSecureURL",
            level: OsslThreatLevel::Moderate,
            register: register_os_request_secure_url,
        },
        OsslFunc {
            name: "osSetRot",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_set_rot,
        },
        OsslFunc {
            name: "osGetLinkColor",
            level: OsslThreatLevel::None,
            register: register_os_get_link_color,
        },
        OsslFunc {
            name: "osKey2Name",
            level: OsslThreatLevel::Low,
            register: register_os_key2name,
        },
        OsslFunc {
            name: "osAvatarName2Key",
            level: OsslThreatLevel::Low,
            register: register_os_avatar_name2key,
        },
        OsslFunc {
            name: "osGetAvatarHomeURI",
            level: OsslThreatLevel::Low,
            register: register_os_get_avatar_home_uri,
        },
        OsslFunc {
            name: "osListenRegex",
            level: OsslThreatLevel::None,
            register: register_os_listen_regex,
        },
    ]
}

// ---------------------------------------------------------------------------
// Notecard extras
// ---------------------------------------------------------------------------

fn register_os_loaded_creation_date(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osLoadedCreationDate called (stub)");
        Ok(String::new())
    })?;
    lua.globals().set("osLoadedCreationDate", f)
}

fn register_os_loaded_creation_time(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osLoadedCreationTime called (stub)");
        Ok(String::new())
    })?;
    lua.globals().set("osLoadedCreationTime", f)
}

fn register_os_loaded_creation_id(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osLoadedCreationID called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osLoadedCreationID", f)
}

// ---------------------------------------------------------------------------
// Appearance
// ---------------------------------------------------------------------------

fn register_os_owner_save_appearance(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _notecard: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osOwnerSaveAppearance called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osOwnerSaveAppearance", f)
}

fn register_os_agent_save_appearance(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _notecard): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osAgentSaveAppearance called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osAgentSaveAppearance", f)
}

fn register_os_get_region_map_texture(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _region_name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetRegionMapTexture called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osGetRegionMapTexture", f)
}

// ---------------------------------------------------------------------------
// Inertia
// ---------------------------------------------------------------------------

fn register_os_get_inertia_data(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetInertiaData called (stub)");
        lua.create_table()
    })?;
    lua.globals().set("osGetInertiaData", f)
}

fn register_os_set_inertia(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (_mass, _com, _principal_axes, _rot): (f64, String, String, String)| {
            let c = ctx.read();
            tracing::debug!(script_id = %c.script_id, "osSetInertia called (stub)");
            Ok(())
        },
    )?;
    lua.globals().set("osSetInertia", f)
}

fn register_os_set_inertia_as_box(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (_mass, _box_size, _com, _rot): (f64, String, String, String)| {
            let c = ctx.read();
            tracing::debug!(script_id = %c.script_id, "osSetInertiaAsBox called (stub)");
            Ok(())
        },
    )?;
    lua.globals().set("osSetInertiaAsBox", f)
}

fn register_os_set_inertia_as_sphere(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (_mass, _radius, _com, _rot): (f64, f64, String, String)| {
            let c = ctx.read();
            tracing::debug!(script_id = %c.script_id, "osSetInertiaAsSphere called (stub)");
            Ok(())
        },
    )?;
    lua.globals().set("osSetInertiaAsSphere", f)
}

fn register_os_set_inertia_as_cylinder(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (_mass, _radius, _length, _com, _rot): (f64, f64, f64, String, String)| {
            let c = ctx.read();
            tracing::debug!(script_id = %c.script_id, "osSetInertiaAsCylinder called (stub)");
            Ok(())
        },
    )?;
    lua.globals().set("osSetInertiaAsCylinder", f)
}

fn register_os_clear_inertia(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osClearInertia called (stub)");
        Ok(())
    })?;
    lua.globals().set("osClearInertia", f)
}

// ---------------------------------------------------------------------------
// Sit
// ---------------------------------------------------------------------------

fn register_os_sit_on_link(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_link, _offset, _rot): (i32, String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSitOnLink called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSitOnLink", f)
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

fn register_os_clear_object_animations(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osClearObjectAnimations called (stub)");
        Ok(())
    })?;
    lua.globals().set("osClearObjectAnimations", f)
}

fn register_os_reset_all_scripts(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osResetAllScripts called (stub)");
        Ok(())
    })?;
    lua.globals().set("osResetAllScripts", f)
}

fn register_os_particle_system(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _rules: mlua::MultiValue| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osParticleSystem called (stub)");
        Ok(())
    })?;
    lua.globals().set("osParticleSystem", f)
}

fn register_os_link_particle_system(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_link, _rules): (i32, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osLinkParticleSystem called (stub)");
        Ok(())
    })?;
    lua.globals().set("osLinkParticleSystem", f)
}

fn register_os_invite_to_group(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _agent: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osInviteToGroup called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osInviteToGroup", f)
}

fn register_os_eject_from_group(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _agent: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osEjectFromGroup called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osEjectFromGroup", f)
}

fn register_os_console_command(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _command: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osConsoleCommand called (stub)");
        Ok(())
    })?;
    lua.globals().set("osConsoleCommand", f)
}

fn register_os_request_url(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _options: mlua::MultiValue| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osRequestURL called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osRequestURL", f)
}

fn register_os_request_secure_url(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _options: mlua::MultiValue| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osRequestSecureURL called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osRequestSecureURL", f)
}

fn register_os_set_rot(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_target, _rot): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetRot called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetRot", f)
}

fn register_os_get_link_color(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_link, _face): (i32, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetLinkColor called (stub)");
        Ok("<0, 0, 0>".to_string())
    })?;
    lua.globals().set("osGetLinkColor", f)
}

fn register_os_key2name(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _key: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osKey2Name called (stub)");
        Ok(String::new())
    })?;
    lua.globals().set("osKey2Name", f)
}

fn register_os_avatar_name2key(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osAvatarName2Key called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osAvatarName2Key", f)
}

fn register_os_get_avatar_home_uri(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _avatar: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetAvatarHomeURI called (stub)");
        Ok(String::new())
    })?;
    lua.globals().set("osGetAvatarHomeURI", f)
}

fn register_os_listen_regex(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_,
              (_channel, _name, _key, _regex, _regex_bitfield): (
            i32,
            String,
            String,
            String,
            i32,
        )| {
            let c = ctx.read();
            tracing::debug!(script_id = %c.script_id, "osListenRegex called (stub)");
            Ok(-1i32)
        },
    )?;
    lua.globals().set("osListenRegex", f)
}
