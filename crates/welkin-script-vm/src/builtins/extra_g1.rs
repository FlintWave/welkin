// SPDX-License-Identifier: LGPL-3.0-or-later

use std::sync::Arc;

use parking_lot::RwLock;

use super::ScriptContext;

pub fn register_extra_g1(
    lua: &mlua::Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llGetFreeMemory
    {
        let ctx = Arc::clone(ctx);
        let _ = ctx;
        lua.globals().set(
            "llGetFreeMemory",
            lua.create_function(move |_lua, ()| Ok(65536i64))?,
        )?;
    }

    // llGetLocalPos
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetLocalPos",
            lua.create_function(move |lua, ()| {
                let ctx = ctx.read();
                let table = lua.create_table()?;
                table.set("x", ctx.position[0])?;
                table.set("y", ctx.position[1])?;
                table.set("z", ctx.position[2])?;
                Ok(table)
            })?,
        )?;
    }

    // llGetLocalRot
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetLocalRot",
            lua.create_function(move |lua, ()| {
                let ctx = ctx.read();
                let table = lua.create_table()?;
                table.set("x", ctx.rotation[0])?;
                table.set("y", ctx.rotation[1])?;
                table.set("z", ctx.rotation[2])?;
                table.set("w", ctx.rotation[3])?;
                Ok(table)
            })?,
        )?;
    }

    // llGetMass
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetMass",
            lua.create_function(move |_lua, ()| {
                let ctx = ctx.read();
                Ok(ctx.prim_mass)
            })?,
        )?;
    }

    // llGetRegionName
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetRegionName",
            lua.create_function(move |_lua, ()| {
                let ctx = ctx.read();
                Ok(ctx.region_name.clone())
            })?,
        )?;
    }

    // llGetRegionTimeDilation
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetRegionTimeDilation",
            lua.create_function(move |_lua, ()| {
                let ctx = ctx.read();
                Ok(ctx.time_dilation)
            })?,
        )?;
    }

    // llGetRootRotation
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetRootRotation",
            lua.create_function(move |lua, ()| {
                let ctx = ctx.read();
                let table = lua.create_table()?;
                table.set("x", ctx.rotation[0])?;
                table.set("y", ctx.rotation[1])?;
                table.set("z", ctx.rotation[2])?;
                table.set("w", ctx.rotation[3])?;
                Ok(table)
            })?,
        )?;
    }

    // llGetScriptName
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetScriptName",
            lua.create_function(move |_lua, ()| {
                let ctx = ctx.read();
                Ok(ctx.script_name.clone())
            })?,
        )?;
    }

    // llGetStartParameter
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetStartParameter",
            lua.create_function(move |_lua, ()| {
                let ctx = ctx.read();
                Ok(ctx.start_param as i64)
            })?,
        )?;
    }

    // llGetStatus
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetStatus",
            lua.create_function(move |_lua, status: i64| {
                let ctx = ctx.read();
                if (ctx.status_flags as i64 & status) != 0 {
                    Ok(1i64)
                } else {
                    Ok(0i64)
                }
            })?,
        )?;
    }

    // llGetForce
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetForce",
            lua.create_function(move |lua, ()| {
                let snapshot = {
                    let ctx = ctx.read();
                    (
                        ctx.chat_sender.clone(),
                        ctx.prim_id,
                        ctx.prim_name.clone(),
                        ctx.position,
                        ctx.owner_id,
                    )
                };
                let (sender, prim_id, prim_name, source_pos, owner_id) = snapshot;
                if let Some(sender) = sender {
                    let _ = sender.send(crate::ChatMessage {
                        source_id: prim_id,
                        source_name: prim_name,
                        source_pos,
                        owner_id,
                        channel: 0,
                        message: "llGetForce is not supported on this region".to_string(),
                        chat_type: crate::ChatType::Owner,
                        target_id: Some(owner_id),
                    });
                }
                let table = lua.create_table()?;
                table.set("x", 0.0f64)?;
                table.set("y", 0.0f64)?;
                table.set("z", 0.0f64)?;
                Ok(table)
            })?,
        )?;
    }

    // llGetLinkName
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetLinkName",
            lua.create_function(move |_lua, linknum: i64| {
                let ctx = ctx.read();
                if linknum == ctx.link_num as i64 {
                    Ok(ctx.prim_name.clone())
                } else {
                    Ok(String::new())
                }
            })?,
        )?;
    }

    // llGetInventoryCreator
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llGetInventoryCreator",
            lua.create_function(move |_lua, item: String| {
                let ctx = ctx.read();
                for entry in &ctx.inventory {
                    if entry.name == item {
                        return Ok(entry.creator_id.to_string());
                    }
                }
                Ok("00000000-0000-0000-0000-000000000000".to_string())
            })?,
        )?;
    }

    // llKey2Name
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llKey2Name",
            lua.create_function(move |_lua, key: String| {
                let ctx = ctx.read();
                let parsed = match uuid::Uuid::parse_str(&key) {
                    Ok(value) => value,
                    Err(_) => return Ok(String::new()),
                };
                if parsed.is_nil() {
                    return Ok(String::new());
                }
                for presence in &ctx.presence_list {
                    if presence.uuid == parsed {
                        return Ok(format!(
                            "{} {}",
                            presence.first_name, presence.last_name
                        ));
                    }
                }
                Ok(String::new())
            })?,
        )?;
    }

    // llSameGroup
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSameGroup",
            lua.create_function(move |_lua, key: String| {
                let ctx = ctx.read();
                match uuid::Uuid::parse_str(&key) {
                    Ok(value) if value == ctx.owner_id => Ok(1i64),
                    _ => Ok(0i64),
                }
            })?,
        )?;
    }

    // llSleep
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSleep",
            lua.create_function(move |_lua, seconds: f64| {
                let mut ctx = ctx.write();
                ctx.sleep_requested += seconds.max(0.0);
                Ok(())
            })?,
        )?;
    }

    // llResetScript
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llResetScript",
            lua.create_function(move |_lua, ()| {
                let mut ctx = ctx.write();
                ctx.reset_requested = true;
                Ok(())
            })?,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::ScriptContext;
    use mlua::Lua;
    use std::sync::Arc;

    fn ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext::default()))
    }

    #[test]
    fn free_memory_returns_positive() {
        let lua = Lua::new();
        let ctx = ctx();
        register_extra_g1(&lua, &ctx).unwrap();
        let value: i64 = lua.load("return llGetFreeMemory()").eval().unwrap();
        assert!(value > 0);
    }

    #[test]
    fn local_pos_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.position = [1.0, 2.0, 3.0];
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let x: f64 = lua.load("local v = llGetLocalPos(); return v.x").eval().unwrap();
        let y: f64 = lua.load("local v = llGetLocalPos(); return v.y").eval().unwrap();
        let z: f64 = lua.load("local v = llGetLocalPos(); return v.z").eval().unwrap();
        assert_eq!(x, 1.0);
        assert_eq!(y, 2.0);
        assert_eq!(z, 3.0);
    }

    #[test]
    fn local_rot_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.rotation = [0.1, 0.2, 0.3, 0.4];
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let x: f64 = lua.load("local r = llGetLocalRot(); return r.x").eval().unwrap();
        let w: f64 = lua.load("local r = llGetLocalRot(); return r.w").eval().unwrap();
        assert_eq!(x, 0.1);
        assert_eq!(w, 0.4);
    }

    #[test]
    fn mass_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.prim_mass = 42.5;
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let value: f64 = lua.load("return llGetMass()").eval().unwrap();
        assert_eq!(value, 42.5);
    }

    #[test]
    fn region_name_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.region_name = "Test Region".to_string();
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let value: String = lua.load("return llGetRegionName()").eval().unwrap();
        assert_eq!(value, "Test Region");
    }

    #[test]
    fn region_time_dilation_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.time_dilation = 0.987;
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let value: f64 = lua.load("return llGetRegionTimeDilation()").eval().unwrap();
        assert_eq!(value, 0.987);
    }

    #[test]
    fn root_rotation_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.rotation = [1.0, 0.0, 0.0, 0.0];
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let x: f64 = lua.load("local r = llGetRootRotation(); return r.x").eval().unwrap();
        assert_eq!(x, 1.0);
    }

    #[test]
    fn script_name_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.script_name = "script.lsl".to_string();
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let value: String = lua.load("return llGetScriptName()").eval().unwrap();
        assert_eq!(value, "script.lsl");
    }

    #[test]
    fn start_parameter_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.start_param = 7;
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let value: i64 = lua.load("return llGetStartParameter()").eval().unwrap();
        assert_eq!(value, 7);
    }

    #[test]
    fn status_reads_flags() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.status_flags = 0b1010;
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let set: i64 = lua.load("return llGetStatus(0x0008)").eval().unwrap();
        let unset: i64 = lua.load("return llGetStatus(0x0004)").eval().unwrap();
        assert_eq!(set, 1);
        assert_eq!(unset, 0);
    }

    #[test]
    fn force_returns_zero_vector() {
        let lua = Lua::new();
        let ctx = ctx();
        register_extra_g1(&lua, &ctx).unwrap();
        let x: f64 = lua.load("local v = llGetForce(); return v.x").eval().unwrap();
        let y: f64 = lua.load("local v = llGetForce(); return v.y").eval().unwrap();
        let z: f64 = lua.load("local v = llGetForce(); return v.z").eval().unwrap();
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn link_name_current_link_reads_context() {
        let lua = Lua::new();
        let ctx = ctx();
        {
            let mut guard = ctx.write();
            guard.link_num = 3;
            guard.prim_name = "My Prim".to_string();
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let value: String = lua.load("return llGetLinkName(3)").eval().unwrap();
        assert_eq!(value, "My Prim");
    }

    #[test]
    fn inventory_creator_defaults_to_nil() {
        let lua = Lua::new();
        let ctx = ctx();
        register_extra_g1(&lua, &ctx).unwrap();
        let value: String = lua
            .load("return llGetInventoryCreator(\"missing\")")
            .eval()
            .unwrap();
        assert_eq!(value, "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn key_to_name_defaults_to_empty() {
        let lua = Lua::new();
        let ctx = ctx();
        register_extra_g1(&lua, &ctx).unwrap();
        let value: String = lua
            .load("return llKey2Name(\"00000000-0000-0000-0000-000000000000\")")
            .eval()
            .unwrap();
        assert_eq!(value, "");
    }

    #[test]
    fn same_group_compares_owner() {
        let lua = Lua::new();
        let ctx = ctx();
        let owner = uuid::Uuid::new_v4();
        {
            let mut guard = ctx.write();
            guard.owner_id = owner;
        }
        register_extra_g1(&lua, &ctx).unwrap();
        let same: i64 = lua
            .load(&format!("return llSameGroup(\"{}\")", owner))
            .eval()
            .unwrap();
        let other: i64 = lua
            .load("return llSameGroup(\"11111111-2222-3333-4444-555555555555\")")
            .eval()
            .unwrap();
        assert_eq!(same, 1);
        assert_eq!(other, 0);
    }

    #[test]
    fn sleep_records_requested_time() {
        let lua = Lua::new();
        let ctx = ctx();
        register_extra_g1(&lua, &ctx).unwrap();
        lua.load("llSleep(1.5)").exec().unwrap();
        lua.load("llSleep(-3)").exec().unwrap();
        let requested = ctx.read().sleep_requested;
        assert_eq!(requested, 1.5);
    }

    #[test]
    fn reset_script_sets_flag() {
        let lua = Lua::new();
        let ctx = ctx();
        register_extra_g1(&lua, &ctx).unwrap();
        lua.load("llResetScript()").exec().unwrap();
        assert!(ctx.read().reset_requested);
    }
}
