// SPDX-License-Identifier: GPL-3.0-or-later
//! Inventory, attachment, sound, and link/prim/object stub functions (Task 11.1).

use std::sync::Arc;
use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;
use crate::builtins::ScriptContext;
use super::{OsslFunc, OsslThreatLevel};

pub(super) fn ossl_inventory_funcs() -> Vec<OsslFunc> {
    vec![
        // Inventory (VeryHigh unless noted)
        OsslFunc { name: "osGiveInventory", level: OsslThreatLevel::VeryHigh, register: register_os_give_inventory },
        OsslFunc { name: "osGiveInventoryList", level: OsslThreatLevel::VeryHigh, register: register_os_give_inventory_list },
        OsslFunc { name: "osForceDropAttachment", level: OsslThreatLevel::VeryHigh, register: register_os_force_drop_attachment },
        OsslFunc { name: "osForceDropAttachmentAt", level: OsslThreatLevel::VeryHigh, register: register_os_force_drop_attachment_at },
        OsslFunc { name: "osGetInventoryDesc", level: OsslThreatLevel::Low, register: register_os_get_inventory_desc },
        OsslFunc { name: "osGetInventoryItemKey", level: OsslThreatLevel::Low, register: register_os_get_inventory_item_key },
        OsslFunc { name: "osGetInventoryLastOwner", level: OsslThreatLevel::Low, register: register_os_get_inventory_last_owner },
        OsslFunc { name: "osGetInventoryName", level: OsslThreatLevel::Low, register: register_os_get_inventory_name },
        OsslFunc { name: "osGetInventoryNumber", level: OsslThreatLevel::Low, register: register_os_get_inventory_number },
        OsslFunc { name: "osGetInventoryPermMask", level: OsslThreatLevel::Low, register: register_os_get_inventory_perm_mask },
        OsslFunc { name: "osSetInventoryDesc", level: OsslThreatLevel::Low, register: register_os_set_inventory_desc },
        OsslFunc { name: "osRemoveInventory", level: OsslThreatLevel::VeryHigh, register: register_os_remove_inventory },
        OsslFunc { name: "osGetLastChangedEventKey", level: OsslThreatLevel::None, register: register_os_get_last_changed_event_key },
        OsslFunc { name: "osTakeObject", level: OsslThreatLevel::VeryHigh, register: register_os_take_object },
        OsslFunc { name: "osOwnerDropAttachment", level: OsslThreatLevel::None, register: register_os_owner_drop_attachment },
        OsslFunc { name: "osOwnerDropAttachmentAt", level: OsslThreatLevel::None, register: register_os_owner_drop_attachment_at },
        OsslFunc { name: "osDropAttachment", level: OsslThreatLevel::VeryHigh, register: register_os_drop_attachment },
        // Attachment
        OsslFunc { name: "osForceAttachToAvatar", level: OsslThreatLevel::VeryHigh, register: register_os_force_attach_to_avatar },
        OsslFunc { name: "osForceAttachToAvatarAt", level: OsslThreatLevel::VeryHigh, register: register_os_force_attach_to_avatar_at },
        OsslFunc { name: "osForceAttachToOtherAvatarFromInventory", level: OsslThreatLevel::Severe, register: register_os_force_attach_to_other_avatar },
        OsslFunc { name: "osForceDetachFromAvatar", level: OsslThreatLevel::VeryHigh, register: register_os_force_detach_from_avatar },
        OsslFunc { name: "osGetNumberOfAttachments", level: OsslThreatLevel::Moderate, register: register_os_get_number_of_attachments },
        OsslFunc { name: "osMessageAttachments", level: OsslThreatLevel::Moderate, register: register_os_message_attachments },
        // Sound
        OsslFunc { name: "osPlaySound", level: OsslThreatLevel::None, register: register_os_play_sound },
        OsslFunc { name: "osLoopSound", level: OsslThreatLevel::None, register: register_os_loop_sound },
        OsslFunc { name: "osLoopSoundMaster", level: OsslThreatLevel::None, register: register_os_loop_sound_master },
        OsslFunc { name: "osLoopSoundSlave", level: OsslThreatLevel::None, register: register_os_loop_sound_slave },
        OsslFunc { name: "osStopSound", level: OsslThreatLevel::None, register: register_os_stop_sound },
        OsslFunc { name: "osPreloadSound", level: OsslThreatLevel::None, register: register_os_preload_sound },
        OsslFunc { name: "osAdjustSoundVolume", level: OsslThreatLevel::None, register: register_os_adjust_sound_volume },
        OsslFunc { name: "osTriggerSound", level: OsslThreatLevel::None, register: register_os_trigger_sound },
        OsslFunc { name: "osTriggerSoundLimited", level: OsslThreatLevel::None, register: register_os_trigger_sound_limited },
        OsslFunc { name: "osSoundSetRadius", level: OsslThreatLevel::None, register: register_os_sound_set_radius },
        OsslFunc { name: "osMakeFire", level: OsslThreatLevel::VeryLow, register: register_os_make_fire },
        OsslFunc { name: "osMakeSmoke", level: OsslThreatLevel::VeryLow, register: register_os_make_smoke },
        OsslFunc { name: "osMakeExplosion", level: OsslThreatLevel::VeryLow, register: register_os_make_explosion },
        // Link/prim/object
        OsslFunc { name: "osForceCreateLink", level: OsslThreatLevel::VeryLow, register: register_os_force_create_link },
        OsslFunc { name: "osForceBreakLink", level: OsslThreatLevel::VeryLow, register: register_os_force_break_link },
        OsslFunc { name: "osForceBreakAllLinks", level: OsslThreatLevel::VeryLow, register: register_os_force_break_all_links },
        OsslFunc { name: "osDie", level: OsslThreatLevel::Low, register: register_os_die },
        OsslFunc { name: "osMessageObject", level: OsslThreatLevel::Low, register: register_os_message_object },
        OsslFunc { name: "osGetRezzingObject", level: OsslThreatLevel::None, register: register_os_get_rezzing_object },
        OsslFunc { name: "osSetContentType", level: OsslThreatLevel::High, register: register_os_set_content_type },
        OsslFunc { name: "osVolumeDetect", level: OsslThreatLevel::Low, register: register_os_volume_detect },
        OsslFunc { name: "osGetPrimCount", level: OsslThreatLevel::None, register: register_os_get_prim_count },
        OsslFunc { name: "osGetSittingAvatarsCount", level: OsslThreatLevel::None, register: register_os_get_sitting_avatars_count },
        OsslFunc { name: "osGetSittingAvatars", level: OsslThreatLevel::None, register: register_os_get_sitting_avatars },
        OsslFunc { name: "osSetProjectionParams", level: OsslThreatLevel::High, register: register_os_set_projection_params },
        OsslFunc { name: "osGetLinkPrimitiveParams", level: OsslThreatLevel::High, register: register_os_get_link_primitive_params },
        OsslFunc { name: "osSetPrimitiveParams", level: OsslThreatLevel::High, register: register_os_set_primitive_params },
        OsslFunc { name: "osGetLinkNumber", level: OsslThreatLevel::None, register: register_os_get_link_number },
    ]
}

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

fn register_os_give_inventory(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_dest, _name): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGiveInventory called (stub)");
        Ok(())
    })?;
    lua.globals().set("osGiveInventory", f)
}

fn register_os_give_inventory_list(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_dest, _category, _items): (String, String, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGiveInventoryList called (stub)");
        Ok(())
    })?;
    lua.globals().set("osGiveInventoryList", f)
}

fn register_os_force_drop_attachment(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceDropAttachment called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceDropAttachment", f)
}

fn register_os_force_drop_attachment_at(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_pos, _rot): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceDropAttachmentAt called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceDropAttachmentAt", f)
}

fn register_os_get_inventory_desc(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetInventoryDesc called (stub)");
        Ok(String::new())
    })?;
    lua.globals().set("osGetInventoryDesc", f)
}

fn register_os_get_inventory_item_key(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetInventoryItemKey called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osGetInventoryItemKey", f)
}

fn register_os_get_inventory_last_owner(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetInventoryLastOwner called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osGetInventoryLastOwner", f)
}

fn register_os_get_inventory_name(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _index: i32| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetInventoryName called (stub)");
        Ok(String::new())
    })?;
    lua.globals().set("osGetInventoryName", f)
}

fn register_os_get_inventory_number(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _type_: i32| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetInventoryNumber called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osGetInventoryNumber", f)
}

fn register_os_get_inventory_perm_mask(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_name, _category): (String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetInventoryPermMask called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osGetInventoryPermMask", f)
}

fn register_os_set_inventory_desc(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_name, _desc): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetInventoryDesc called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetInventoryDesc", f)
}

fn register_os_remove_inventory(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osRemoveInventory called (stub)");
        Ok(())
    })?;
    lua.globals().set("osRemoveInventory", f)
}

fn register_os_get_last_changed_event_key(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetLastChangedEventKey called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osGetLastChangedEventKey", f)
}

fn register_os_take_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osTakeObject called (stub)");
        Ok(())
    })?;
    lua.globals().set("osTakeObject", f)
}

fn register_os_owner_drop_attachment(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osOwnerDropAttachment called (stub)");
        Ok(())
    })?;
    lua.globals().set("osOwnerDropAttachment", f)
}

fn register_os_owner_drop_attachment_at(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_pos, _rot): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osOwnerDropAttachmentAt called (stub)");
        Ok(())
    })?;
    lua.globals().set("osOwnerDropAttachmentAt", f)
}

fn register_os_drop_attachment(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osDropAttachment called (stub)");
        Ok(())
    })?;
    lua.globals().set("osDropAttachment", f)
}

// ---------------------------------------------------------------------------
// Attachment
// ---------------------------------------------------------------------------

fn register_os_force_attach_to_avatar(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _attach_point: i32| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceAttachToAvatar called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceAttachToAvatar", f)
}

fn register_os_force_attach_to_avatar_at(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _attach_point, _pos, _rot): (String, i32, String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceAttachToAvatarAt called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceAttachToAvatarAt", f)
}

fn register_os_force_attach_to_other_avatar(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _name, _attach_point): (String, String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceAttachToOtherAvatarFromInventory called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceAttachToOtherAvatarFromInventory", f)
}

fn register_os_force_detach_from_avatar(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceDetachFromAvatar called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceDetachFromAvatar", f)
}

fn register_os_get_number_of_attachments(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, (_avatar, _points): (String, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetNumberOfAttachments called (stub)");
        lua.create_table()
    })?;
    lua.globals().set("osGetNumberOfAttachments", f)
}

fn register_os_message_attachments(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_avatar, _msg, _points, _flags): (String, String, String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osMessageAttachments called (stub)");
        Ok(())
    })?;
    lua.globals().set("osMessageAttachments", f)
}

// ---------------------------------------------------------------------------
// Sound
// ---------------------------------------------------------------------------

fn register_os_play_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_linknum, _sound, _volume): (i32, String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osPlaySound called (stub)");
        Ok(())
    })?;
    lua.globals().set("osPlaySound", f)
}

fn register_os_loop_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_linknum, _sound, _volume): (i32, String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osLoopSound called (stub)");
        Ok(())
    })?;
    lua.globals().set("osLoopSound", f)
}

fn register_os_loop_sound_master(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_linknum, _sound, _volume): (i32, String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osLoopSoundMaster called (stub)");
        Ok(())
    })?;
    lua.globals().set("osLoopSoundMaster", f)
}

fn register_os_loop_sound_slave(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_linknum, _sound, _volume): (i32, String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osLoopSoundSlave called (stub)");
        Ok(())
    })?;
    lua.globals().set("osLoopSoundSlave", f)
}

fn register_os_stop_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _linknum: i32| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osStopSound called (stub)");
        Ok(())
    })?;
    lua.globals().set("osStopSound", f)
}

fn register_os_preload_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_linknum, _sound): (i32, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osPreloadSound called (stub)");
        Ok(())
    })?;
    lua.globals().set("osPreloadSound", f)
}

fn register_os_adjust_sound_volume(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_linknum, _volume): (i32, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osAdjustSoundVolume called (stub)");
        Ok(())
    })?;
    lua.globals().set("osAdjustSoundVolume", f)
}

fn register_os_trigger_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_linknum, _sound, _volume): (i32, String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osTriggerSound called (stub)");
        Ok(())
    })?;
    lua.globals().set("osTriggerSound", f)
}

fn register_os_trigger_sound_limited(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_sound, _volume, _tne, _bsw): (String, f64, String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osTriggerSoundLimited called (stub)");
        Ok(())
    })?;
    lua.globals().set("osTriggerSoundLimited", f)
}

fn register_os_sound_set_radius(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_sound, _radius): (String, f64)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSoundSetRadius called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSoundSetRadius", f)
}

fn register_os_make_fire(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _args: mlua::MultiValue| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osMakeFire called (stub)");
        Ok(())
    })?;
    lua.globals().set("osMakeFire", f)
}

fn register_os_make_smoke(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _args: mlua::MultiValue| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osMakeSmoke called (stub)");
        Ok(())
    })?;
    lua.globals().set("osMakeSmoke", f)
}

fn register_os_make_explosion(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _args: mlua::MultiValue| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osMakeExplosion called (stub)");
        Ok(())
    })?;
    lua.globals().set("osMakeExplosion", f)
}

// ---------------------------------------------------------------------------
// Link/prim/object
// ---------------------------------------------------------------------------

fn register_os_force_create_link(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_target, _parent): (String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceCreateLink called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceCreateLink", f)
}

fn register_os_force_break_link(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _link: i32| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceBreakLink called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceBreakLink", f)
}

fn register_os_force_break_all_links(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osForceBreakAllLinks called (stub)");
        Ok(())
    })?;
    lua.globals().set("osForceBreakAllLinks", f)
}

fn register_os_die(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _obj_key: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osDie called (stub)");
        Ok(())
    })?;
    lua.globals().set("osDie", f)
}

fn register_os_message_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_obj_key, _msg): (String, String)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osMessageObject called (stub)");
        Ok(())
    })?;
    lua.globals().set("osMessageObject", f)
}

fn register_os_get_rezzing_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetRezzingObject called (stub)");
        Ok(Uuid::nil().to_string())
    })?;
    lua.globals().set("osGetRezzingObject", f)
}

fn register_os_set_content_type(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_id, _type_): (String, i32)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetContentType called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetContentType", f)
}

fn register_os_volume_detect(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _enable: i32| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osVolumeDetect called (stub)");
        Ok(())
    })?;
    lua.globals().set("osVolumeDetect", f)
}

fn register_os_get_prim_count(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _pos: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetPrimCount called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osGetPrimCount", f)
}

fn register_os_get_sitting_avatars_count(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _prim: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetSittingAvatarsCount called (stub)");
        Ok(0i32)
    })?;
    lua.globals().set("osGetSittingAvatarsCount", f)
}

fn register_os_get_sitting_avatars(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, _prim: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetSittingAvatars called (stub)");
        lua.create_table()
    })?;
    lua.globals().set("osGetSittingAvatars", f)
}

fn register_os_set_projection_params(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _args: mlua::MultiValue| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetProjectionParams called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetProjectionParams", f)
}

fn register_os_get_link_primitive_params(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, (_link, _params): (i32, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetLinkPrimitiveParams called (stub)");
        lua.create_table()
    })?;
    lua.globals().set("osGetLinkPrimitiveParams", f)
}

fn register_os_set_primitive_params(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_prim, _params): (String, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osSetPrimitiveParams called (stub)");
        Ok(())
    })?;
    lua.globals().set("osSetPrimitiveParams", f)
}

fn register_os_get_link_number(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _name: String| {
        let c = ctx.read();
        tracing::debug!(script_id = %c.script_id, "osGetLinkNumber called (stub)");
        Ok(-1i32)
    })?;
    lua.globals().set("osGetLinkNumber", f)
}
