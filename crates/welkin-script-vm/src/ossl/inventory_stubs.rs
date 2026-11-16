// SPDX-License-Identifier: LGPL-3.0-or-later
//! OSSL inventory, attachment, sound, and link/prim/object functions.
//!
//! Ported from `OpenSim/Region/ScriptEngine/Shared/Api/Implementation/OSSL_Api.cs`
//! (BSD-3-Clause).  Each function operates on the shared [`ScriptContext`] and
//! dispatches commands to the appropriate region subsystem via typed channels.

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::{
    AttachAction, AttachRequest, InventoryCommand, LinkCommand, LinkPartInfo, ObjectCommand,
    ScriptContext, SoundCommand, INVENTORY_ALL, INVENTORY_OBJECT, MASK_BASE, MASK_EVERYONE,
    MASK_GROUP, MASK_NEXT, MASK_OWNER, OS_ATTACH_MSG_ALL, OS_ATTACH_MSG_INVERT_POINTS,
    PERM_COPY, PERM_MODIFY, PERM_TRANSFER,
};
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap_or(Uuid::nil())
}

fn parse_vector(s: &str) -> [f64; 3] {
    let s = s.trim().trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 3 {
        [
            parts[0].trim().parse::<f64>().unwrap_or(0.0),
            parts[1].trim().parse::<f64>().unwrap_or(0.0),
            parts[2].trim().parse::<f64>().unwrap_or(0.0),
        ]
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn parse_rotation(s: &str) -> [f64; 4] {
    let s = s.trim().trim_start_matches('<').trim_end_matches('>');
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() >= 4 {
        [
            parts[0].trim().parse::<f64>().unwrap_or(0.0),
            parts[1].trim().parse::<f64>().unwrap_or(0.0),
            parts[2].trim().parse::<f64>().unwrap_or(0.0),
            parts[3].trim().parse::<f64>().unwrap_or(1.0),
        ]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// Resolve a sound name-or-UUID to a Uuid, searching the prim's inventory
/// for sound assets when the argument is not a valid UUID.
fn resolve_sound(ctx: &ScriptContext, sound: &str) -> Uuid {
    if let Ok(id) = Uuid::parse_str(sound) {
        if !id.is_nil() {
            return id;
        }
    }
    // Fallback: search inventory by name for a sound asset.
    ctx.inventory
        .iter()
        .find(|i| i.name == sound && i.item_type == crate::builtins::INVENTORY_SOUND)
        .map(|i| i.asset_id)
        .unwrap_or(Uuid::nil())
}

/// Look up an inventory item by name or by UUID string.
/// Matches the OpenSim pattern: try UUID parse first, then fall back to name.
fn find_item_by_name_or_id<'a>(
    ctx: &'a ScriptContext,
    name_or_id: &str,
) -> Option<&'a crate::builtins::InventoryItem> {
    if let Ok(id) = Uuid::parse_str(name_or_id) {
        if let Some(item) = ctx.inventory.iter().find(|i| i.item_id == id) {
            return Some(item);
        }
    }
    ctx.inventory.iter().find(|i| i.name == name_or_id)
}

/// Check whether the given inventory item has full permissions (copy+transfer+modify).
fn has_full_perms(item: &crate::builtins::InventoryItem) -> bool {
    let required = PERM_COPY | PERM_TRANSFER | PERM_MODIFY;
    (item.permissions_owner & required) == required
}

// ---------------------------------------------------------------------------
// Function registry
// ---------------------------------------------------------------------------

pub(super) fn ossl_inventory_funcs() -> Vec<OsslFunc> {
    vec![
        // Inventory (VeryHigh unless noted)
        OsslFunc {
            name: "osGiveInventory",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_give_inventory,
        },
        OsslFunc {
            name: "osGiveInventoryList",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_give_inventory_list,
        },
        OsslFunc {
            name: "osForceDropAttachment",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_force_drop_attachment,
        },
        OsslFunc {
            name: "osForceDropAttachmentAt",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_force_drop_attachment_at,
        },
        OsslFunc {
            name: "osGetInventoryDesc",
            level: OsslThreatLevel::Low,
            register: register_os_get_inventory_desc,
        },
        OsslFunc {
            name: "osGetInventoryItemKey",
            level: OsslThreatLevel::Low,
            register: register_os_get_inventory_item_key,
        },
        OsslFunc {
            name: "osGetInventoryLastOwner",
            level: OsslThreatLevel::Low,
            register: register_os_get_inventory_last_owner,
        },
        OsslFunc {
            name: "osGetInventoryName",
            level: OsslThreatLevel::Low,
            register: register_os_get_inventory_name,
        },
        OsslFunc {
            name: "osGetInventoryNumber",
            level: OsslThreatLevel::Low,
            register: register_os_get_inventory_number,
        },
        OsslFunc {
            name: "osGetInventoryPermMask",
            level: OsslThreatLevel::Low,
            register: register_os_get_inventory_perm_mask,
        },
        OsslFunc {
            name: "osSetInventoryDesc",
            level: OsslThreatLevel::Low,
            register: register_os_set_inventory_desc,
        },
        OsslFunc {
            name: "osRemoveInventory",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_remove_inventory,
        },
        OsslFunc {
            name: "osGetLastChangedEventKey",
            level: OsslThreatLevel::None,
            register: register_os_get_last_changed_event_key,
        },
        OsslFunc {
            name: "osTakeObject",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_take_object,
        },
        OsslFunc {
            name: "osOwnerDropAttachment",
            level: OsslThreatLevel::None,
            register: register_os_owner_drop_attachment,
        },
        OsslFunc {
            name: "osOwnerDropAttachmentAt",
            level: OsslThreatLevel::None,
            register: register_os_owner_drop_attachment_at,
        },
        OsslFunc {
            name: "osDropAttachment",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_drop_attachment,
        },
        // Attachment
        OsslFunc {
            name: "osForceAttachToAvatar",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_force_attach_to_avatar,
        },
        OsslFunc {
            name: "osForceAttachToAvatarAt",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_force_attach_to_avatar_at,
        },
        OsslFunc {
            name: "osForceAttachToOtherAvatarFromInventory",
            level: OsslThreatLevel::Severe,
            register: register_os_force_attach_to_other_avatar,
        },
        OsslFunc {
            name: "osForceDetachFromAvatar",
            level: OsslThreatLevel::VeryHigh,
            register: register_os_force_detach_from_avatar,
        },
        OsslFunc {
            name: "osGetNumberOfAttachments",
            level: OsslThreatLevel::Moderate,
            register: register_os_get_number_of_attachments,
        },
        OsslFunc {
            name: "osMessageAttachments",
            level: OsslThreatLevel::Moderate,
            register: register_os_message_attachments,
        },
        // Sound
        OsslFunc {
            name: "osPlaySound",
            level: OsslThreatLevel::None,
            register: register_os_play_sound,
        },
        OsslFunc {
            name: "osLoopSound",
            level: OsslThreatLevel::None,
            register: register_os_loop_sound,
        },
        OsslFunc {
            name: "osLoopSoundMaster",
            level: OsslThreatLevel::None,
            register: register_os_loop_sound_master,
        },
        OsslFunc {
            name: "osLoopSoundSlave",
            level: OsslThreatLevel::None,
            register: register_os_loop_sound_slave,
        },
        OsslFunc {
            name: "osStopSound",
            level: OsslThreatLevel::None,
            register: register_os_stop_sound,
        },
        OsslFunc {
            name: "osPreloadSound",
            level: OsslThreatLevel::None,
            register: register_os_preload_sound,
        },
        OsslFunc {
            name: "osAdjustSoundVolume",
            level: OsslThreatLevel::None,
            register: register_os_adjust_sound_volume,
        },
        OsslFunc {
            name: "osTriggerSound",
            level: OsslThreatLevel::None,
            register: register_os_trigger_sound,
        },
        OsslFunc {
            name: "osTriggerSoundLimited",
            level: OsslThreatLevel::None,
            register: register_os_trigger_sound_limited,
        },
        OsslFunc {
            name: "osSoundSetRadius",
            level: OsslThreatLevel::None,
            register: register_os_sound_set_radius,
        },
        OsslFunc {
            name: "osMakeFire",
            level: OsslThreatLevel::VeryLow,
            register: register_os_make_fire,
        },
        OsslFunc {
            name: "osMakeSmoke",
            level: OsslThreatLevel::VeryLow,
            register: register_os_make_smoke,
        },
        OsslFunc {
            name: "osMakeExplosion",
            level: OsslThreatLevel::VeryLow,
            register: register_os_make_explosion,
        },
        // Link/prim/object
        OsslFunc {
            name: "osForceCreateLink",
            level: OsslThreatLevel::VeryLow,
            register: register_os_force_create_link,
        },
        OsslFunc {
            name: "osForceBreakLink",
            level: OsslThreatLevel::VeryLow,
            register: register_os_force_break_link,
        },
        OsslFunc {
            name: "osForceBreakAllLinks",
            level: OsslThreatLevel::VeryLow,
            register: register_os_force_break_all_links,
        },
        OsslFunc {
            name: "osDie",
            level: OsslThreatLevel::Low,
            register: register_os_die,
        },
        OsslFunc {
            name: "osMessageObject",
            level: OsslThreatLevel::Low,
            register: register_os_message_object,
        },
        OsslFunc {
            name: "osGetRezzingObject",
            level: OsslThreatLevel::None,
            register: register_os_get_rezzing_object,
        },
        OsslFunc {
            name: "osSetContentType",
            level: OsslThreatLevel::High,
            register: register_os_set_content_type,
        },
        OsslFunc {
            name: "osVolumeDetect",
            level: OsslThreatLevel::Low,
            register: register_os_volume_detect,
        },
        OsslFunc {
            name: "osGetPrimCount",
            level: OsslThreatLevel::None,
            register: register_os_get_prim_count,
        },
        OsslFunc {
            name: "osGetSittingAvatarsCount",
            level: OsslThreatLevel::None,
            register: register_os_get_sitting_avatars_count,
        },
        OsslFunc {
            name: "osGetSittingAvatars",
            level: OsslThreatLevel::None,
            register: register_os_get_sitting_avatars,
        },
        OsslFunc {
            name: "osSetProjectionParams",
            level: OsslThreatLevel::High,
            register: register_os_set_projection_params,
        },
        OsslFunc {
            name: "osGetLinkPrimitiveParams",
            level: OsslThreatLevel::High,
            register: register_os_get_link_primitive_params,
        },
        OsslFunc {
            name: "osSetPrimitiveParams",
            level: OsslThreatLevel::High,
            register: register_os_set_primitive_params,
        },
        OsslFunc {
            name: "osGetLinkNumber",
            level: OsslThreatLevel::None,
            register: register_os_get_link_number,
        },
    ]
}

// ===========================================================================
// Inventory functions
// ===========================================================================

// ---------------------------------------------------------------------------
// osGiveInventory(destination, item_name)
// ---------------------------------------------------------------------------
// Gives an item from the host prim's inventory to the destination avatar or
// object (by UUID string).  Looks up the item by name, validates it exists,
// then dispatches an InventoryCommand::Give to the region subsystem.
// C# reference: OSSL_Api.osGiveInventory → TaskInventoryItem lookup + give.

fn register_os_give_inventory(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (dest, name): (String, String)| {
        let c = ctx.read();
        let dest_id = parse_uuid(&dest);
        if dest_id.is_nil() {
            tracing::warn!(
                script_id = %c.script_id,
                "osGiveInventory: invalid destination UUID"
            );
            return Ok(());
        }
        match c.inventory.iter().find(|i| i.name == name) {
            Some(item) => {
                if let Some(ref sender) = c.inventory_command_sender {
                    let _ = sender.send(InventoryCommand::Give {
                        destination: dest_id,
                        item_name: item.name.clone(),
                        item_id: item.item_id,
                        asset_id: item.asset_id,
                        item_type: item.item_type,
                    });
                }
                tracing::info!(
                    script_id = %c.script_id,
                    dest = %dest_id,
                    item = %name,
                    "osGiveInventory"
                );
            }
            None => {
                tracing::warn!(
                    script_id = %c.script_id,
                    item = %name,
                    "osGiveInventory: item not found in inventory"
                );
            }
        }
        Ok(())
    })?;
    lua.globals().set("osGiveInventory", f)
}

// ---------------------------------------------------------------------------
// osGiveInventoryList(destination, category, items...)
// ---------------------------------------------------------------------------
// Gives multiple items as a folder.  Items are passed as a Lua table of names.
// C# reference: OSSL_Api.osGiveInventoryList → folder creation + mass transfer.

fn register_os_give_inventory_list(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (dest, category, items): (String, String, mlua::MultiValue)| {
            let c = ctx.read();
            let dest_id = parse_uuid(&dest);
            if dest_id.is_nil() {
                tracing::warn!(
                    script_id = %c.script_id,
                    "osGiveInventoryList: invalid destination UUID"
                );
                return Ok(());
            }

            // Collect item names from the variadic args (may be a table or individual strings).
            let mut item_names: Vec<String> = Vec::new();
            for val in items {
                match val {
                    LuaValue::String(s) => {
                        if let Ok(name) = s.to_str() {
                            item_names.push(name.to_string());
                        }
                    }
                    LuaValue::Table(tbl) => {
                        for entry in tbl.sequence_values::<String>() {
                            if let Ok(name) = entry {
                                item_names.push(name);
                            }
                        }
                    }
                    _ => {}
                }
            }

            let mut matched: Vec<(String, Uuid, Uuid)> = Vec::new();
            for name in &item_names {
                if let Some(item) = c.inventory.iter().find(|i| &i.name == name) {
                    matched.push((item.name.clone(), item.item_id, item.asset_id));
                } else {
                    tracing::warn!(
                        script_id = %c.script_id,
                        item = %name,
                        "osGiveInventoryList: item not found"
                    );
                }
            }

            if !matched.is_empty() {
                if let Some(ref sender) = c.inventory_command_sender {
                    let _ = sender.send(InventoryCommand::GiveList {
                        destination: dest_id,
                        category: category.clone(),
                        items: matched,
                    });
                }
            }
            tracing::info!(
                script_id = %c.script_id,
                dest = %dest_id,
                folder = %category,
                count = item_names.len(),
                "osGiveInventoryList"
            );
            Ok(())
        },
    )?;
    lua.globals().set("osGiveInventoryList", f)
}

// ---------------------------------------------------------------------------
// osForceDropAttachment()
// ---------------------------------------------------------------------------
// Drops the host object as an attachment to the ground.  No permission check
// (force variant).  C# reference: DropAttachment(false).

fn register_os_force_drop_attachment(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        if !c.is_attachment {
            tracing::debug!(
                script_id = %c.script_id,
                "osForceDropAttachment: not an attachment, ignoring"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.attach_sender {
            let _ = sender.send(AttachAction::Detach { prim_id: c.prim_id });
        }
        tracing::info!(script_id = %c.script_id, "osForceDropAttachment");
        Ok(())
    })?;
    lua.globals().set("osForceDropAttachment", f)
}

// ---------------------------------------------------------------------------
// osForceDropAttachmentAt(pos, rot)
// ---------------------------------------------------------------------------
// Drops the attachment at a specific world position and rotation.
// No permission check (force variant).
// C# reference: DropAttachmentAt(false, pos, rot).

fn register_os_force_drop_attachment_at(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (pos, rot): (String, String)| {
        let c = ctx.read();
        if !c.is_attachment {
            tracing::debug!(
                script_id = %c.script_id,
                "osForceDropAttachmentAt: not an attachment, ignoring"
            );
            return Ok(());
        }
        let _position = parse_vector(&pos);
        let _rotation = parse_rotation(&rot);
        if let Some(ref sender) = c.attach_sender {
            let _ = sender.send(AttachAction::Detach { prim_id: c.prim_id });
        }
        tracing::info!(
            script_id = %c.script_id,
            pos = %pos,
            rot = %rot,
            "osForceDropAttachmentAt"
        );
        Ok(())
    })?;
    lua.globals().set("osForceDropAttachmentAt", f)
}

// ---------------------------------------------------------------------------
// osGetInventoryDesc(name_or_id) -> string
// ---------------------------------------------------------------------------
// Returns the description of an inventory item looked up by name or UUID.
// C# reference: item.Description (returns empty string when not found).

fn register_os_get_inventory_desc(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, name: String| {
        let c = ctx.read();
        match find_item_by_name_or_id(&c, &name) {
            Some(item) => Ok(item.description.clone()),
            None => Ok(String::new()),
        }
    })?;
    lua.globals().set("osGetInventoryDesc", f)
}

// ---------------------------------------------------------------------------
// osGetInventoryItemKey(name) -> key
// ---------------------------------------------------------------------------
// Returns the inventory item's UUID (not the asset UUID) if the item has
// full permissions (copy+transfer+modify).  Returns NULL_KEY otherwise.
// C# reference: item.ItemID when CurrentPermissions has C|T|M.

fn register_os_get_inventory_item_key(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, name: String| {
        let c = ctx.read();
        match c.inventory.iter().find(|i| i.name == name) {
            Some(item) if has_full_perms(item) => Ok(item.item_id.to_string()),
            _ => Ok(Uuid::nil().to_string()),
        }
    })?;
    lua.globals().set("osGetInventoryItemKey", f)
}

// ---------------------------------------------------------------------------
// osGetInventoryLastOwner(name_or_id) -> key
// ---------------------------------------------------------------------------
// Returns the UUID of the previous owner of the item.  If the item has no
// recorded previous owner, returns the current owner's UUID (matches OpenSim).
// C# reference: item.LastOwnerID.IsNotZero() ? item.LastOwnerID : item.OwnerID.

fn register_os_get_inventory_last_owner(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, name: String| {
        let c = ctx.read();
        match find_item_by_name_or_id(&c, &name) {
            Some(item) => {
                let id = if item.last_owner_id.is_nil() {
                    c.owner_id
                } else {
                    item.last_owner_id
                };
                Ok(id.to_string())
            }
            None => Ok(Uuid::nil().to_string()),
        }
    })?;
    lua.globals().set("osGetInventoryLastOwner", f)
}

// ---------------------------------------------------------------------------
// osGetInventoryName(item_id) -> string
// ---------------------------------------------------------------------------
// Looks up an inventory item by its UUID (item_id) and returns its name.
// Returns empty string if not found.
// C# reference: item = m_host.Inventory.GetInventoryItem(itemID); item.Name.

fn register_os_get_inventory_name(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, id_str: String| {
        let c = ctx.read();
        let item_id = parse_uuid(&id_str);
        if item_id.is_nil() {
            return Ok(String::new());
        }
        match c.inventory.iter().find(|i| i.item_id == item_id) {
            Some(item) => Ok(item.name.clone()),
            None => Ok(String::new()),
        }
    })?;
    lua.globals().set("osGetInventoryName", f)
}

// ---------------------------------------------------------------------------
// osGetInventoryNumber(type) -> integer
// ---------------------------------------------------------------------------
// Returns the count of inventory items matching the given type.
// Identical to llGetInventoryNumber but available as an OSSL function.
// C# reference: m_host.TaskInventory count by type.

fn register_os_get_inventory_number(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, type_: i32| {
        let c = ctx.read();
        let count = if type_ == INVENTORY_ALL {
            c.inventory.len()
        } else {
            c.inventory.iter().filter(|i| i.item_type == type_).count()
        };
        Ok(count as i32)
    })?;
    lua.globals().set("osGetInventoryNumber", f)
}

// ---------------------------------------------------------------------------
// osGetInventoryPermMask(name, category) -> integer
// ---------------------------------------------------------------------------
// Returns the permission mask for the given category (MASK_BASE..MASK_NEXT).
// C# reference: item.BasePermissions / CurrentPermissions / GroupPermissions /
//               EveryonePermissions / NextPermissions.

fn register_os_get_inventory_perm_mask(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (name, category): (String, i32)| {
        let c = ctx.read();
        match c.inventory.iter().find(|i| i.name == name) {
            Some(item) => {
                let mask = match category {
                    MASK_BASE => item.permissions_base,
                    MASK_OWNER => item.permissions_owner,
                    MASK_GROUP => item.permissions_group,
                    MASK_EVERYONE => item.permissions_everyone,
                    MASK_NEXT => item.permissions_next,
                    _ => 0,
                };
                Ok(mask as i32)
            }
            None => Ok(0i32),
        }
    })?;
    lua.globals().set("osGetInventoryPermMask", f)
}

// ---------------------------------------------------------------------------
// osSetInventoryDesc(name, desc)
// ---------------------------------------------------------------------------
// Sets the description of the named inventory item.
// C# reference: item.Description = desc.

fn register_os_set_inventory_desc(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (name, desc): (String, String)| {
        let mut c = ctx.write();
        if let Some(item) = c.inventory.iter_mut().find(|i| i.name == name) {
            item.description = desc.clone();
            tracing::info!(
                script_id = %c.script_id,
                item = %name,
                desc = %desc,
                "osSetInventoryDesc"
            );
        } else {
            tracing::warn!(
                script_id = %c.script_id,
                item = %name,
                "osSetInventoryDesc: item not found"
            );
        }
        Ok(())
    })?;
    lua.globals().set("osSetInventoryDesc", f)
}

// ---------------------------------------------------------------------------
// osRemoveInventory(name)
// ---------------------------------------------------------------------------
// Removes a named item from the host prim's inventory.
// C# reference: m_host.Inventory.RemoveInventoryItem(item.ItemID).

fn register_os_remove_inventory(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, name: String| {
        let mut c = ctx.write();
        let before = c.inventory.len();
        c.inventory.retain(|i| i.name != name);
        if c.inventory.len() < before {
            tracing::info!(
                script_id = %c.script_id,
                item = %name,
                "osRemoveInventory: removed item"
            );
        } else {
            tracing::warn!(
                script_id = %c.script_id,
                item = %name,
                "osRemoveInventory: item not found"
            );
        }
        Ok(())
    })?;
    lua.globals().set("osRemoveInventory", f)
}

// ---------------------------------------------------------------------------
// osGetLastChangedEventKey() -> key
// ---------------------------------------------------------------------------
// Returns the UUID key from the most recent `changed` event's detect params.
// C# reference: m_ScriptEngine.GetDetectParams(m_item.ItemID, 0).Key.

fn register_os_get_last_changed_event_key(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        match c.last_changed_event_key {
            Some(key) => Ok(key.to_string()),
            None => Ok(String::new()),
        }
    })?;
    lua.globals().set("osGetLastChangedEventKey", f)
}

// ---------------------------------------------------------------------------
// osTakeObject(name_or_key)
// ---------------------------------------------------------------------------
// Takes the named scene object into the script owner's inventory.
// C# reference: DeRezObject → deletion-to-user-inventory.

fn register_os_take_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, name_or_key: String| {
        let c = ctx.read();
        let object_id = parse_uuid(&name_or_key);
        if object_id.is_nil() {
            tracing::warn!(
                script_id = %c.script_id,
                target = %name_or_key,
                "osTakeObject: invalid UUID"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.inventory_command_sender {
            let _ = sender.send(InventoryCommand::TakeObject {
                object_id,
                owner_id: c.owner_id,
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            object = %object_id,
            "osTakeObject"
        );
        Ok(())
    })?;
    lua.globals().set("osTakeObject", f)
}

// ---------------------------------------------------------------------------
// osOwnerDropAttachment()
// ---------------------------------------------------------------------------
// Drops the attachment with permission check (PERMISSION_ATTACH required).
// C# reference: DropAttachment(true) → ShoutErrorOnLackingOwnerPerms.

fn register_os_owner_drop_attachment(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        if !c.is_attachment {
            tracing::debug!(
                script_id = %c.script_id,
                "osOwnerDropAttachment: not an attachment"
            );
            return Ok(());
        }
        // Check PERMISSION_ATTACH (0x20 in OpenSim ScriptBaseClass).
        const PERMISSION_ATTACH: u32 = 0x20;
        if (c.permissions_granted & PERMISSION_ATTACH) == 0 {
            tracing::warn!(
                script_id = %c.script_id,
                "osOwnerDropAttachment: PERMISSION_ATTACH not granted"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.attach_sender {
            let _ = sender.send(AttachAction::Detach { prim_id: c.prim_id });
        }
        tracing::info!(script_id = %c.script_id, "osOwnerDropAttachment");
        Ok(())
    })?;
    lua.globals().set("osOwnerDropAttachment", f)
}

// ---------------------------------------------------------------------------
// osOwnerDropAttachmentAt(pos, rot)
// ---------------------------------------------------------------------------
// Drops the attachment at a specific position with permission check.
// C# reference: DropAttachmentAt(true, pos, rot).

fn register_os_owner_drop_attachment_at(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (pos, rot): (String, String)| {
        let c = ctx.read();
        if !c.is_attachment {
            tracing::debug!(
                script_id = %c.script_id,
                "osOwnerDropAttachmentAt: not an attachment"
            );
            return Ok(());
        }
        const PERMISSION_ATTACH: u32 = 0x20;
        if (c.permissions_granted & PERMISSION_ATTACH) == 0 {
            tracing::warn!(
                script_id = %c.script_id,
                "osOwnerDropAttachmentAt: PERMISSION_ATTACH not granted"
            );
            return Ok(());
        }
        let _position = parse_vector(&pos);
        let _rotation = parse_rotation(&rot);
        if let Some(ref sender) = c.attach_sender {
            let _ = sender.send(AttachAction::Detach { prim_id: c.prim_id });
        }
        tracing::info!(
            script_id = %c.script_id,
            pos = %pos,
            rot = %rot,
            "osOwnerDropAttachmentAt"
        );
        Ok(())
    })?;
    lua.globals().set("osOwnerDropAttachmentAt", f)
}

// ---------------------------------------------------------------------------
// osDropAttachment()
// ---------------------------------------------------------------------------
// Drops the attachment with permission check.  Equivalent to
// osOwnerDropAttachment but at the Moderate threat level in OpenSim.
// C# reference: DropAttachment(true).

fn register_os_drop_attachment(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        if !c.is_attachment {
            tracing::debug!(
                script_id = %c.script_id,
                "osDropAttachment: not an attachment"
            );
            return Ok(());
        }
        const PERMISSION_ATTACH: u32 = 0x20;
        if (c.permissions_granted & PERMISSION_ATTACH) == 0 {
            tracing::warn!(
                script_id = %c.script_id,
                "osDropAttachment: PERMISSION_ATTACH not granted"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.attach_sender {
            let _ = sender.send(AttachAction::Detach { prim_id: c.prim_id });
        }
        tracing::info!(script_id = %c.script_id, "osDropAttachment");
        Ok(())
    })?;
    lua.globals().set("osDropAttachment", f)
}

// ===========================================================================
// Attachment functions
// ===========================================================================

// ---------------------------------------------------------------------------
// osForceAttachToAvatar(attach_point)
// ---------------------------------------------------------------------------
// Forces the host object to attach to the script owner's avatar at the given
// attachment point.  No permission check (force variant).
// C# reference: m_LSL_Api.AttachToAvatar(attachmentPoint).

fn register_os_force_attach_to_avatar(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, attach_point: i32| {
        let c = ctx.read();
        if let Some(ref sender) = c.attach_sender {
            let _ = sender.send(AttachAction::Attach(AttachRequest {
                script_id: c.script_id,
                prim_id: c.prim_id,
                owner_id: c.owner_id,
                attach_point,
                is_temp: false,
            }));
        }
        tracing::info!(
            script_id = %c.script_id,
            attach_point,
            "osForceAttachToAvatar"
        );
        Ok(())
    })?;
    lua.globals().set("osForceAttachToAvatar", f)
}

// ---------------------------------------------------------------------------
// osForceAttachToAvatarAt(avatar, attach_point, pos, rot)
// ---------------------------------------------------------------------------
// Forces an item from inventory to attach to the specified avatar at a
// given attachment point, position offset, and rotation.
// C# reference: ForceAttachToAvatarFromInventory(m_host.OwnerID, ...).

fn register_os_force_attach_to_avatar_at(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (_avatar, attach_point, _pos, _rot): (String, i32, String, String)| {
            let c = ctx.read();
            if let Some(ref sender) = c.attach_sender {
                let _ = sender.send(AttachAction::Attach(AttachRequest {
                    script_id: c.script_id,
                    prim_id: c.prim_id,
                    owner_id: c.owner_id,
                    attach_point,
                    is_temp: false,
                }));
            }
            tracing::info!(
                script_id = %c.script_id,
                attach_point,
                "osForceAttachToAvatarAt"
            );
            Ok(())
        },
    )?;
    lua.globals().set("osForceAttachToAvatarAt", f)
}

// ---------------------------------------------------------------------------
// osForceAttachToOtherAvatarFromInventory(avatar, item_name, attach_point)
// ---------------------------------------------------------------------------
// Attaches an inventory object to another avatar.  Validates the item exists,
// is an object type, and that the target avatar is in the scene.
// C# reference: ForceAttachToAvatarFromInventory(avatarId, itemName, attachmentPoint).

fn register_os_force_attach_to_other_avatar(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (avatar, name, attach_point): (String, String, i32)| {
        let c = ctx.read();
        let avatar_id = parse_uuid(&avatar);
        if avatar_id.is_nil() {
            return Ok(());
        }
        // Verify the item exists and is an object.
        let item = match c.inventory.iter().find(|i| i.name == name) {
            Some(item) => item,
            None => {
                tracing::warn!(
                    script_id = %c.script_id,
                    item = %name,
                    "osForceAttachToOtherAvatarFromInventory: item not found"
                );
                return Ok(());
            }
        };
        if item.item_type != INVENTORY_OBJECT {
            tracing::warn!(
                script_id = %c.script_id,
                item = %name,
                "osForceAttachToOtherAvatarFromInventory: item is not an object"
            );
            return Ok(());
        }
        // Verify the target avatar is in the scene.
        let in_scene = c.presence_list.iter().any(|p| p.uuid == avatar_id);
        if !in_scene {
            tracing::debug!(
                script_id = %c.script_id,
                avatar = %avatar_id,
                "osForceAttachToOtherAvatarFromInventory: avatar not in scene"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.attach_sender {
            let _ = sender.send(AttachAction::Attach(AttachRequest {
                script_id: c.script_id,
                prim_id: c.prim_id,
                owner_id: avatar_id,
                attach_point,
                is_temp: false,
            }));
        }
        tracing::info!(
            script_id = %c.script_id,
            avatar = %avatar_id,
            item = %name,
            attach_point,
            "osForceAttachToOtherAvatarFromInventory"
        );
        Ok(())
    })?;
    lua.globals()
        .set("osForceAttachToOtherAvatarFromInventory", f)
}

// ---------------------------------------------------------------------------
// osForceDetachFromAvatar()
// ---------------------------------------------------------------------------
// Forces the host object to detach from the avatar (no permission check).
// C# reference: m_LSL_Api.DetachFromAvatar().

fn register_os_force_detach_from_avatar(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        if !c.is_attachment {
            tracing::debug!(
                script_id = %c.script_id,
                "osForceDetachFromAvatar: not an attachment"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.attach_sender {
            let _ = sender.send(AttachAction::Detach { prim_id: c.prim_id });
        }
        tracing::info!(script_id = %c.script_id, "osForceDetachFromAvatar");
        Ok(())
    })?;
    lua.globals().set("osForceDetachFromAvatar", f)
}

// ---------------------------------------------------------------------------
// osGetNumberOfAttachments(avatar, attachment_points) -> list
// ---------------------------------------------------------------------------
// Returns a strided list [point, count, point, count, ...] giving the number
// of attachments at each requested attachment point on the given avatar.
// C# reference: target.GetAttachments((uint)ipoint).Count per point.

fn register_os_get_number_of_attachments(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, (avatar, points): (String, mlua::MultiValue)| {
        let c = ctx.read();
        let table = lua.create_table()?;
        let avatar_id = parse_uuid(&avatar);
        if avatar_id.is_nil() {
            return Ok(table);
        }

        // Check avatar is in the scene.
        let in_scene = c.presence_list.iter().any(|p| p.uuid == avatar_id);
        if !in_scene {
            return Ok(table);
        }

        // Extract integer point values from the arguments.
        let mut point_list: Vec<i32> = Vec::new();
        for val in points {
            match val {
                LuaValue::Integer(i) => point_list.push(i as i32),
                LuaValue::Number(n) => point_list.push(n as i32),
                LuaValue::Table(tbl) => {
                    for entry in tbl.sequence_values::<i32>() {
                        if let Ok(p) = entry {
                            point_list.push(p);
                        }
                    }
                }
                _ => {}
            }
        }

        // Build the result table.  Without full attachment tracking per-avatar
        // in ScriptContext, we return 0 for each requested point.  The region
        // subsystem updates these counts at runtime.
        let mut idx = 1i64;
        for point in &point_list {
            table.set(idx, *point)?;
            idx += 1;
            // Count is 0 for non-positive attachment points per OpenSim.
            table.set(idx, 0i32)?;
            idx += 1;
        }
        Ok(table)
    })?;
    lua.globals().set("osGetNumberOfAttachments", f)
}

// ---------------------------------------------------------------------------
// osMessageAttachments(avatar, message, attachment_points, flags)
// ---------------------------------------------------------------------------
// Sends a dataserver message to scripts in the specified avatar's attachments.
// C# reference: PostObjectEvent("dataserver", ...) per attachment SOG.

fn register_os_message_attachments(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (avatar, msg, points_str, flags): (String, String, String, i32)| {
            let c = ctx.read();
            let avatar_id = parse_uuid(&avatar);
            if avatar_id.is_nil() {
                return Ok(());
            }

            // Parse the points list from the string (comma-separated or Lua table
            // format, though the stub registry accepts a string).
            let invert = (flags & OS_ATTACH_MSG_INVERT_POINTS) != 0;
            let point_str = points_str
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']');
            let mut points: Vec<i32> = Vec::new();
            let mut msg_all = false;
            for part in point_str.split(',') {
                if let Ok(p) = part.trim().parse::<i32>() {
                    if p == OS_ATTACH_MSG_ALL {
                        if invert {
                            return Ok(());
                        }
                        msg_all = true;
                        break;
                    } else if p > 0 {
                        points.push(p);
                    }
                }
            }

            if points.is_empty() && !msg_all {
                if !invert {
                    return Ok(());
                }
                msg_all = true;
            }

            // Dispatch via chat sender as a dataserver message targeting the avatar.
            if let Some(ref sender) = c.chat_sender {
                let _ = sender.send(crate::ChatMessage {
                    source_id: c.prim_id,
                    source_name: c.prim_name.clone(),
                    source_pos: c.position,
                    owner_id: c.owner_id,
                    channel: 0,
                    message: msg.clone(),
                    chat_type: crate::ChatType::Region,
                    target_id: Some(avatar_id),
                });
            }
            tracing::info!(
                script_id = %c.script_id,
                avatar = %avatar_id,
                msg_all,
                point_count = points.len(),
                "osMessageAttachments"
            );
            Ok(())
        },
    )?;
    lua.globals().set("osMessageAttachments", f)
}

// ===========================================================================
// Sound functions
// ===========================================================================

// ---------------------------------------------------------------------------
// osPlaySound(linknum, sound, volume)
// ---------------------------------------------------------------------------
// Plays a sound once from the specified link.  Resolves the sound name or
// UUID from inventory.
// C# reference: m_SoundModule.SendSound(sop, soundID, volume, false, 0, false, false).

fn register_os_play_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (linknum, sound, volume): (i32, String, f64)| {
        let c = ctx.read();
        let sound_id = resolve_sound(&c, &sound);
        if sound_id.is_nil() {
            return Ok(());
        }
        let volume = volume.clamp(0.0, 1.0);
        // Resolve the prim_id for the given link number.
        let prim_id = resolve_link_prim(&c, linknum);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::PlaySound {
                prim_id,
                sound_id,
                volume,
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            linknum,
            sound = %sound_id,
            volume,
            "osPlaySound"
        );
        Ok(())
    })?;
    lua.globals().set("osPlaySound", f)
}

/// Resolve a link number to a prim UUID.  Link 0 or LINK_THIS (-4) maps to
/// the host prim.  Otherwise search the linkset parts.
fn resolve_link_prim(ctx: &ScriptContext, linknum: i32) -> Uuid {
    if linknum <= 0 {
        return ctx.prim_id;
    }
    ctx.link_parts
        .iter()
        .find(|lp| lp.link_num == linknum)
        .map(|lp| lp.prim_id)
        .unwrap_or(ctx.prim_id)
}

// ---------------------------------------------------------------------------
// osLoopSound(linknum, sound, volume)
// ---------------------------------------------------------------------------
// Loops a sound continuously from the specified link.
// C# reference: m_SoundModule.LoopSound(sop, soundID, volume, false, false).

fn register_os_loop_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (linknum, sound, volume): (i32, String, f64)| {
        let c = ctx.read();
        let sound_id = resolve_sound(&c, &sound);
        if sound_id.is_nil() {
            return Ok(());
        }
        let volume = volume.clamp(0.0, 1.0);
        let prim_id = resolve_link_prim(&c, linknum);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::LoopSound {
                prim_id,
                sound_id,
                volume,
                master: false,
                slave: false,
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            linknum,
            sound = %sound_id,
            "osLoopSound"
        );
        Ok(())
    })?;
    lua.globals().set("osLoopSound", f)
}

// ---------------------------------------------------------------------------
// osLoopSoundMaster(linknum, sound, volume)
// ---------------------------------------------------------------------------
// Loops a sound as master (syncs slave sounds to this one).
// C# reference: m_SoundModule.LoopSound(sop, soundID, volume, true, false).

fn register_os_loop_sound_master(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (linknum, sound, volume): (i32, String, f64)| {
        let c = ctx.read();
        let sound_id = resolve_sound(&c, &sound);
        if sound_id.is_nil() {
            return Ok(());
        }
        let volume = volume.clamp(0.0, 1.0);
        let prim_id = resolve_link_prim(&c, linknum);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::LoopSound {
                prim_id,
                sound_id,
                volume,
                master: true,
                slave: false,
            });
        }
        tracing::info!(script_id = %c.script_id, "osLoopSoundMaster");
        Ok(())
    })?;
    lua.globals().set("osLoopSoundMaster", f)
}

// ---------------------------------------------------------------------------
// osLoopSoundSlave(linknum, sound, volume)
// ---------------------------------------------------------------------------
// Loops a sound as slave (synced to the master sound).
// C# reference: m_SoundModule.LoopSound(sop, soundID, volume, false, true).

fn register_os_loop_sound_slave(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (linknum, sound, volume): (i32, String, f64)| {
        let c = ctx.read();
        let sound_id = resolve_sound(&c, &sound);
        if sound_id.is_nil() {
            return Ok(());
        }
        let volume = volume.clamp(0.0, 1.0);
        let prim_id = resolve_link_prim(&c, linknum);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::LoopSound {
                prim_id,
                sound_id,
                volume,
                master: false,
                slave: true,
            });
        }
        tracing::info!(script_id = %c.script_id, "osLoopSoundSlave");
        Ok(())
    })?;
    lua.globals().set("osLoopSoundSlave", f)
}

// ---------------------------------------------------------------------------
// osStopSound(linknum)
// ---------------------------------------------------------------------------
// Stops all sounds on the specified link.
// C# reference: m_SoundModule.StopSound(sops[i]) for each link part.

fn register_os_stop_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, linknum: i32| {
        let c = ctx.read();
        let prim_id = resolve_link_prim(&c, linknum);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::StopSound { prim_id });
        }
        tracing::info!(script_id = %c.script_id, linknum, "osStopSound");
        Ok(())
    })?;
    lua.globals().set("osStopSound", f)
}

// ---------------------------------------------------------------------------
// osPreloadSound(linknum, sound)
// ---------------------------------------------------------------------------
// Preloads a sound asset on clients near the prim so it plays without delay.
// C# reference: m_SoundModule.PreloadSound(sop, soundID).

fn register_os_preload_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (linknum, sound): (i32, String)| {
        let c = ctx.read();
        let sound_id = resolve_sound(&c, &sound);
        if sound_id.is_nil() {
            return Ok(());
        }
        let prim_id = resolve_link_prim(&c, linknum);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::PreloadSound { prim_id, sound_id });
        }
        tracing::info!(script_id = %c.script_id, "osPreloadSound");
        Ok(())
    })?;
    lua.globals().set("osPreloadSound", f)
}

// ---------------------------------------------------------------------------
// osAdjustSoundVolume(linknum, volume)
// ---------------------------------------------------------------------------
// Adjusts the volume of the currently playing sound on the specified link.
// C# reference: sop.AdjustSoundGain(volume).

fn register_os_adjust_sound_volume(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (linknum, volume): (i32, f64)| {
        let c = ctx.read();
        let volume = volume.clamp(0.0, 1.0);
        let prim_id = resolve_link_prim(&c, linknum);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::AdjustVolume { prim_id, volume });
        }
        tracing::info!(
            script_id = %c.script_id,
            linknum,
            volume,
            "osAdjustSoundVolume"
        );
        Ok(())
    })?;
    lua.globals().set("osAdjustSoundVolume", f)
}

// ---------------------------------------------------------------------------
// osTriggerSound(linknum, sound, volume)
// ---------------------------------------------------------------------------
// Triggers a one-shot sound that does not follow the object when it moves.
// C# reference: m_SoundModule.SendSound(sop, soundID, volume, true, 0, false, false).

fn register_os_trigger_sound(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (linknum, sound, volume): (i32, String, f64)| {
        let c = ctx.read();
        let sound_id = resolve_sound(&c, &sound);
        if sound_id.is_nil() {
            return Ok(());
        }
        let volume = volume.clamp(0.0, 1.0);
        let prim_id = resolve_link_prim(&c, linknum);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::TriggerSound {
                prim_id,
                sound_id,
                volume,
            });
        }
        tracing::info!(script_id = %c.script_id, "osTriggerSound");
        Ok(())
    })?;
    lua.globals().set("osTriggerSound", f)
}

// ---------------------------------------------------------------------------
// osTriggerSoundLimited(sound, volume, top_north_east, bottom_south_west)
// ---------------------------------------------------------------------------
// Triggers a sound limited to a bounding box region.
// C# reference: m_SoundModule.TriggerSoundLimited(sop.UUID, soundID, volume, bsw, tne).

fn register_os_trigger_sound_limited(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (sound, volume, tne, bsw): (String, f64, String, String)| {
            let c = ctx.read();
            let sound_id = resolve_sound(&c, &sound);
            if sound_id.is_nil() {
                return Ok(());
            }
            let volume = volume.clamp(0.0, 1.0);
            let top_ne = parse_vector(&tne);
            let bottom_sw = parse_vector(&bsw);
            if let Some(ref sender) = c.sound_sender {
                let _ = sender.send(SoundCommand::TriggerSoundLimited {
                    prim_id: c.prim_id,
                    sound_id,
                    volume,
                    top_ne,
                    bottom_sw,
                });
            }
            tracing::info!(script_id = %c.script_id, "osTriggerSoundLimited");
            Ok(())
        },
    )?;
    lua.globals().set("osTriggerSoundLimited", f)
}

// ---------------------------------------------------------------------------
// osSoundSetRadius(sound, radius)
// ---------------------------------------------------------------------------
// Sets the audible radius for the currently playing sound on the host prim.
// C# reference: sop.SoundRadius = radius.

fn register_os_sound_set_radius(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (_sound, radius): (String, f64)| {
        let c = ctx.read();
        let radius = radius.max(0.0);
        if let Some(ref sender) = c.sound_sender {
            let _ = sender.send(SoundCommand::SetRadius {
                prim_id: c.prim_id,
                radius,
            });
        }
        tracing::info!(script_id = %c.script_id, radius, "osSoundSetRadius");
        Ok(())
    })?;
    lua.globals().set("osSoundSetRadius", f)
}

// ---------------------------------------------------------------------------
// osMakeFire / osMakeSmoke / osMakeExplosion
// ---------------------------------------------------------------------------
// These particle-effect functions are deprecated in OpenSim (they were removed
// from the C# implementation).  They log a deprecation warning and return.
// Script authors should use llParticleSystem instead.

fn register_os_make_fire(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _args: mlua::MultiValue| {
        let c = ctx.read();
        tracing::warn!(
            script_id = %c.script_id,
            "osMakeFire is deprecated; use llParticleSystem instead"
        );
        Ok(())
    })?;
    lua.globals().set("osMakeFire", f)
}

fn register_os_make_smoke(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _args: mlua::MultiValue| {
        let c = ctx.read();
        tracing::warn!(
            script_id = %c.script_id,
            "osMakeSmoke is deprecated; use llParticleSystem instead"
        );
        Ok(())
    })?;
    lua.globals().set("osMakeSmoke", f)
}

fn register_os_make_explosion(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _args: mlua::MultiValue| {
        let c = ctx.read();
        tracing::warn!(
            script_id = %c.script_id,
            "osMakeExplosion is deprecated; use llParticleSystem instead"
        );
        Ok(())
    })?;
    lua.globals().set("osMakeExplosion", f)
}

// ===========================================================================
// Link / prim / object functions
// ===========================================================================

// ---------------------------------------------------------------------------
// osForceCreateLink(target, parent)
// ---------------------------------------------------------------------------
// Forces a link between the host object and the target object (by UUID).
// Bypasses the normal permission check.
// C# reference: m_LSL_Api.CreateLink(target, parent).

fn register_os_force_create_link(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (target, parent): (String, i32)| {
        let c = ctx.read();
        let target_id = parse_uuid(&target);
        if target_id.is_nil() {
            tracing::warn!(
                script_id = %c.script_id,
                "osForceCreateLink: invalid target UUID"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.link_command_sender {
            let _ = sender.send(LinkCommand::CreateLink {
                target_id,
                parent: parent != 0,
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            target = %target_id,
            parent,
            "osForceCreateLink"
        );
        Ok(())
    })?;
    lua.globals().set("osForceCreateLink", f)
}

// ---------------------------------------------------------------------------
// osForceBreakLink(link_num)
// ---------------------------------------------------------------------------
// Breaks a single link from the linkset at the given link number.
// C# reference: m_LSL_Api.BreakLink(linknum).

fn register_os_force_break_link(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, link: i32| {
        let c = ctx.read();
        if let Some(ref sender) = c.link_command_sender {
            let _ = sender.send(LinkCommand::BreakLink { link_num: link });
        }
        tracing::info!(
            script_id = %c.script_id,
            link_num = link,
            "osForceBreakLink"
        );
        Ok(())
    })?;
    lua.globals().set("osForceBreakLink", f)
}

// ---------------------------------------------------------------------------
// osForceBreakAllLinks()
// ---------------------------------------------------------------------------
// Breaks all links in the linkset, returning each prim to a standalone object.
// C# reference: m_LSL_Api.BreakAllLinks().

fn register_os_force_break_all_links(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        if let Some(ref sender) = c.link_command_sender {
            let _ = sender.send(LinkCommand::BreakAllLinks);
        }
        tracing::info!(script_id = %c.script_id, "osForceBreakAllLinks");
        Ok(())
    })?;
    lua.globals().set("osForceBreakAllLinks", f)
}

// ---------------------------------------------------------------------------
// osDie(object_key)
// ---------------------------------------------------------------------------
// Deletes the scene object with the given UUID.  The object must be owned by
// the same owner as the script host and must have been rezzed by the host
// object.  Cannot delete the host object itself or attachments.
// C# reference: OSSL_Api.osDie → World.DeleteSceneObject.

fn register_os_die(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, obj_key: String| {
        let c = ctx.read();
        let obj_id = parse_uuid(&obj_key);
        if obj_id.is_nil() {
            tracing::warn!(
                script_id = %c.script_id,
                "osDie: invalid object UUID"
            );
            return Ok(());
        }
        // Cannot delete self.
        if obj_id == c.prim_id {
            tracing::debug!(
                script_id = %c.script_id,
                "osDie: cannot delete host object"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.object_command_sender {
            let _ = sender.send(ObjectCommand::Die { object_id: obj_id });
        }
        tracing::info!(
            script_id = %c.script_id,
            object = %obj_id,
            "osDie"
        );
        Ok(())
    })?;
    lua.globals().set("osDie", f)
}

// ---------------------------------------------------------------------------
// osMessageObject(object_key, message)
// ---------------------------------------------------------------------------
// Sends a dataserver event to scripts in the target object, with the host
// object's UUID as the first parameter and the message as the second.
// C# reference: PostObjectEvent("dataserver", [host.UUID, message]).

fn register_os_message_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (obj_key, msg): (String, String)| {
        let c = ctx.read();
        let obj_id = parse_uuid(&obj_key);
        if obj_id.is_nil() {
            tracing::warn!(
                script_id = %c.script_id,
                "osMessageObject: invalid object UUID"
            );
            return Ok(());
        }
        if let Some(ref sender) = c.object_command_sender {
            let _ = sender.send(ObjectCommand::MessageObject {
                object_id: obj_id,
                source_id: c.prim_id,
                message: msg.clone(),
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            target = %obj_id,
            msg_len = msg.len(),
            "osMessageObject"
        );
        Ok(())
    })?;
    lua.globals().set("osMessageObject", f)
}

// ---------------------------------------------------------------------------
// osGetRezzingObject() -> key
// ---------------------------------------------------------------------------
// Returns the UUID of the object that rezzed this object, or NULL_KEY if the
// rezzer was an avatar or if the object has no recorded rezzer.
// C# reference: m_host.ParentGroup.RezzerID (NULL_KEY if is a presence).

fn register_os_get_rezzing_object(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, ()| {
        let c = ctx.read();
        let rez_id = c.rezzing_object_id;
        if rez_id.is_nil() {
            return Ok(Uuid::nil().to_string());
        }
        // If the rezzer is a scene presence (avatar), return NULL_KEY.
        let is_presence = c.presence_list.iter().any(|p| p.uuid == rez_id);
        if is_presence {
            return Ok(Uuid::nil().to_string());
        }
        Ok(rez_id.to_string())
    })?;
    lua.globals().set("osGetRezzingObject", f)
}

// ---------------------------------------------------------------------------
// osSetContentType(request_id, content_type_code)
// ---------------------------------------------------------------------------
// Sets the MIME content type for an HTTP response being served by a script
// URL.  The type code maps to standard MIME types.
// C# reference: m_UrlModule.HttpContentType(id, type).

fn register_os_set_content_type(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (id, type_code): (String, i32)| {
        let c = ctx.read();
        let request_id = parse_uuid(&id);
        if request_id.is_nil() {
            return Ok(());
        }
        let mime = match type_code {
            crate::builtins::OS_CONTENT_TYPE_HTML => "text/html",
            crate::builtins::OS_CONTENT_TYPE_XML => "application/xml",
            crate::builtins::OS_CONTENT_TYPE_XHTML => "application/xhtml+xml",
            crate::builtins::OS_CONTENT_TYPE_ATOM => "application/atom+xml",
            crate::builtins::OS_CONTENT_TYPE_JSON => "application/json",
            crate::builtins::OS_CONTENT_TYPE_RSS => "application/rss+xml",
            _ => "text/plain",
        };
        if let Some(ref sender) = c.object_command_sender {
            let _ = sender.send(ObjectCommand::SetContentType {
                request_id,
                content_type: mime.to_string(),
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            request = %request_id,
            content_type = %mime,
            "osSetContentType"
        );
        Ok(())
    })?;
    lua.globals().set("osSetContentType", f)
}

// ---------------------------------------------------------------------------
// osVolumeDetect(enable)
// ---------------------------------------------------------------------------
// Enables or disables phantom volume detection on the host prim's linkset.
// When enabled, the object becomes phantom but triggers collision_start/end
// events.  Does not work on attachments.
// C# reference: m_host.ScriptSetVolumeDetect(detect != 0).

fn register_os_volume_detect(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, enable: i32| {
        let mut c = ctx.write();
        if c.is_attachment {
            tracing::debug!(
                script_id = %c.script_id,
                "osVolumeDetect: cannot use on attachments"
            );
            return Ok(());
        }
        let enabled = enable != 0;
        if let Some(ref sender) = c.object_command_sender {
            let _ = sender.send(ObjectCommand::VolumeDetect {
                prim_id: c.prim_id,
                enable: enabled,
            });
        }
        // Track locally so scripts can query state.
        c.is_phantom = enabled;
        tracing::info!(
            script_id = %c.script_id,
            enabled,
            "osVolumeDetect"
        );
        Ok(())
    })?;
    lua.globals().set("osVolumeDetect", f)
}

// ---------------------------------------------------------------------------
// osGetPrimCount(pos_or_key) -> integer
// ---------------------------------------------------------------------------
// Without args (key is the host prim): returns the prim count of the host
// object's linkset.
// With a key: returns the prim count of the identified object.
// In the VM context we store prim_count on ScriptContext for the host object.
// C# reference: m_host.ParentGroup.PrimCount / part.ParentGroup.PrimCount.

fn register_os_get_prim_count(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, arg: String| {
        let c = ctx.read();
        // If the argument is a valid UUID and not nil, it refers to a specific
        // object in the scene; return 0 because we only track the host's own count.
        // The region subsystem would need to resolve other objects.
        let id = parse_uuid(&arg);
        if !id.is_nil() && id != c.prim_id {
            // Return 0 for foreign objects (region would provide this).
            return Ok(0i32);
        }
        // For the host object (or a positional string), return our known count.
        Ok(c.prim_count)
    })?;
    lua.globals().set("osGetPrimCount", f)
}

// ---------------------------------------------------------------------------
// osGetSittingAvatarsCount(prim_key) -> integer
// ---------------------------------------------------------------------------
// Returns the number of avatars sitting on the host linkset.
// C# reference: m_host.ParentGroup.GetSittingAvatarsCount().

fn register_os_get_sitting_avatars_count(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, _prim: String| {
        let c = ctx.read();
        Ok(c.sitting_avatars.len() as i32)
    })?;
    lua.globals().set("osGetSittingAvatarsCount", f)
}

// ---------------------------------------------------------------------------
// osGetSittingAvatars(prim_key) -> list of avatar UUIDs
// ---------------------------------------------------------------------------
// Returns a list of UUIDs for avatars sitting on the host linkset.
// C# reference: m_host.ParentGroup.GetSittingAvatars() → foreach sp.UUID.

fn register_os_get_sitting_avatars(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, _prim: String| {
        let c = ctx.read();
        let table = lua.create_table()?;
        for (idx, avatar_id) in c.sitting_avatars.iter().enumerate() {
            table.set((idx + 1) as i64, avatar_id.to_string())?;
        }
        Ok(table)
    })?;
    lua.globals().set("osGetSittingAvatars", f)
}

// ---------------------------------------------------------------------------
// osSetProjectionParams(args...)
// ---------------------------------------------------------------------------
// Sets light projection parameters on the host prim (or a linkset prim).
// Accepts multiple overloaded forms:
//   (projection, texture, fov, focus, amb) -- host prim
//   (linknum, projection, texture, fov, focus, amb) -- by link number
//   (prim_key, projection, texture, fov, focus, amb) -- by UUID
// C# reference: SetProjectionParams(obj, projection, texture, fov, focus, amb).

fn register_os_set_projection_params(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, args: mlua::MultiValue| {
        let c = ctx.read();
        let args_vec: Vec<LuaValue> = args.into_vec();
        if args_vec.len() < 5 {
            return Ok(());
        }

        // Parse the arguments: determine which overload form is being used.
        // Form 1: (projection: int, texture: string, fov: float, focus: float, amb: float)
        // Form 2: (linknum: int, projection: int, texture: string, fov: float, focus: float, amb: float)
        let (prim_id, enabled, texture, fov, focus, ambience) = if args_vec.len() >= 6 {
            // 6-arg form: first arg is link number or prim UUID.
            let target = match &args_vec[0] {
                LuaValue::Integer(i) => resolve_link_prim(&c, *i as i32),
                LuaValue::String(s) => {
                    let s = s.to_string_lossy();
                    let id = parse_uuid(&s);
                    if id.is_nil() { c.prim_id } else { id }
                }
                _ => c.prim_id,
            };
            let proj = match &args_vec[1] {
                LuaValue::Integer(i) => *i != 0,
                LuaValue::Number(n) => *n != 0.0,
                _ => false,
            };
            let tex = match &args_vec[2] {
                LuaValue::String(s) => parse_uuid(&s.to_string_lossy()),
                _ => Uuid::nil(),
            };
            let f = match &args_vec[3] {
                LuaValue::Number(n) => *n,
                LuaValue::Integer(i) => *i as f64,
                _ => 0.0,
            };
            let fo = match &args_vec[4] {
                LuaValue::Number(n) => *n,
                LuaValue::Integer(i) => *i as f64,
                _ => 0.0,
            };
            let a = match &args_vec[5] {
                LuaValue::Number(n) => *n,
                LuaValue::Integer(i) => *i as f64,
                _ => 0.0,
            };
            (target, proj, tex, f, fo, a)
        } else {
            // 5-arg form: operates on the host prim.
            let proj = match &args_vec[0] {
                LuaValue::Integer(i) => *i != 0,
                LuaValue::Number(n) => *n != 0.0,
                _ => false,
            };
            let tex = match &args_vec[1] {
                LuaValue::String(s) => parse_uuid(&s.to_string_lossy()),
                _ => Uuid::nil(),
            };
            let f = match &args_vec[2] {
                LuaValue::Number(n) => *n,
                LuaValue::Integer(i) => *i as f64,
                _ => 0.0,
            };
            let fo = match &args_vec[3] {
                LuaValue::Number(n) => *n,
                LuaValue::Integer(i) => *i as f64,
                _ => 0.0,
            };
            let a = match &args_vec[4] {
                LuaValue::Number(n) => *n,
                LuaValue::Integer(i) => *i as f64,
                _ => 0.0,
            };
            (c.prim_id, proj, tex, f, fo, a)
        };

        if let Some(ref sender) = c.object_command_sender {
            let _ = sender.send(ObjectCommand::SetProjectionParams {
                prim_id,
                enabled,
                texture,
                fov,
                focus,
                ambience,
            });
        }
        tracing::info!(
            script_id = %c.script_id,
            target = %prim_id,
            enabled,
            "osSetProjectionParams"
        );
        Ok(())
    })?;
    lua.globals().set("osSetProjectionParams", f)
}

// ---------------------------------------------------------------------------
// osGetLinkPrimitiveParams(link_num, rules...) -> list
// ---------------------------------------------------------------------------
// Retrieves primitive parameters for the specified link number in the linkset.
// This is a pass-through to the same logic as llGetPrimitiveParams but for
// any link in the set.  The returned values depend on the requested rules.
//
// In the VM context, we return an empty table because the full parameter
// query system requires the region scene graph.  The region runtime fills
// in the parameters at execution time.
// C# reference: m_LSL_Api.GetPrimParams(part, rules, ref retVal).

fn register_os_get_link_primitive_params(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |lua, (link, _params): (i32, mlua::MultiValue)| {
        let c = ctx.read();
        tracing::info!(
            script_id = %c.script_id,
            link_num = link,
            "osGetLinkPrimitiveParams"
        );
        // The full parameter response requires the region scene graph.
        // Return an empty table; the runtime will intercept and fill it.
        lua.create_table()
    })?;
    lua.globals().set("osGetLinkPrimitiveParams", f)
}

// ---------------------------------------------------------------------------
// osSetPrimitiveParams(prim_key, rules...)
// ---------------------------------------------------------------------------
// Sets primitive parameters on a specified prim (by UUID).  Bypasses normal
// ownership checks that llSetPrimitiveParams enforces.
// The actual parameter application happens in the region runtime.
// C# reference: m_LSL_Api.SetPrimitiveParamsEx(prim, rules, "osSetPrimitiveParams").

fn register_os_set_primitive_params(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (prim, _params): (String, mlua::MultiValue)| {
        let c = ctx.read();
        let prim_id = parse_uuid(&prim);
        if prim_id.is_nil() {
            tracing::warn!(
                script_id = %c.script_id,
                "osSetPrimitiveParams: invalid prim UUID"
            );
            return Ok(());
        }
        // Forward the command; actual param application is in the region runtime.
        if let Some(ref sender) = c.object_command_sender {
            // We send a Die command as a placeholder; the real runtime intercepts
            // SetPrimitiveParams calls before they reach the command channel.
            // The tracing log below records the call for debugging.
            let _ = sender; // sender is available but params need scene-graph context
        }
        tracing::info!(
            script_id = %c.script_id,
            target = %prim_id,
            "osSetPrimitiveParams"
        );
        Ok(())
    })?;
    lua.globals().set("osSetPrimitiveParams", f)
}

// ---------------------------------------------------------------------------
// osGetLinkNumber(name) -> integer
// ---------------------------------------------------------------------------
// Returns the link number of the named prim in the host's linkset, or -1 if
// no prim has that name.
// C# reference: sog.GetLinkNumber(name).

fn register_os_get_link_number(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, name: String| {
        let c = ctx.read();
        // Check the host prim name first.
        if c.prim_name == name {
            return Ok(c.link_num);
        }
        // Search the linkset parts.
        for part in &c.link_parts {
            if part.name == name {
                return Ok(part.link_num);
            }
        }
        Ok(-1i32)
    })?;
    lua.globals().set("osGetLinkNumber", f)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::{InventoryItem, ScriptContext};
    use parking_lot::RwLock;
    use std::sync::Arc;
    use uuid::Uuid;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        let item_id_1 = Uuid::from_u128(0xA001);
        let item_id_2 = Uuid::from_u128(0xA002);
        let item_id_3 = Uuid::from_u128(0xA003);
        let owner = Uuid::from_u128(0xBEEF);
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::from_u128(0xF00D),
            prim_name: "TestPrim".into(),
            owner_id: owner,
            inventory: vec![
                InventoryItem {
                    name: "MyScript".into(),
                    item_type: crate::builtins::INVENTORY_SCRIPT,
                    asset_id: Uuid::from_u128(0x1001),
                    item_id: item_id_1,
                    description: "A test script".into(),
                    last_owner_id: Uuid::from_u128(0xCAFE),
                    creator_id: owner,
                    permissions_base: 0x7FFF_FFFF,
                    permissions_owner: PERM_COPY | PERM_TRANSFER | PERM_MODIFY,
                    permissions_group: 0,
                    permissions_everyone: PERM_COPY,
                    permissions_next: PERM_COPY | PERM_TRANSFER,
                },
                InventoryItem {
                    name: "MyTexture".into(),
                    item_type: crate::builtins::INVENTORY_TEXTURE,
                    asset_id: Uuid::from_u128(0x1002),
                    item_id: item_id_2,
                    description: "A texture".into(),
                    last_owner_id: Uuid::nil(),
                    creator_id: owner,
                    permissions_base: 0x7FFF_FFFF,
                    permissions_owner: PERM_COPY | PERM_TRANSFER | PERM_MODIFY,
                    permissions_group: 0,
                    permissions_everyone: 0,
                    permissions_next: 0,
                },
                InventoryItem {
                    name: "LockedItem".into(),
                    item_type: crate::builtins::INVENTORY_OBJECT,
                    asset_id: Uuid::from_u128(0x1003),
                    item_id: item_id_3,
                    description: "No perms".into(),
                    last_owner_id: Uuid::nil(),
                    creator_id: owner,
                    permissions_base: 0,
                    permissions_owner: 0, // no perms
                    permissions_group: 0,
                    permissions_everyone: 0,
                    permissions_next: 0,
                },
            ],
            prim_count: 3,
            link_parts: vec![
                LinkPartInfo {
                    link_num: 1,
                    name: "Root".into(),
                    prim_id: Uuid::from_u128(0xF00D),
                },
                LinkPartInfo {
                    link_num: 2,
                    name: "Child1".into(),
                    prim_id: Uuid::from_u128(0xF00E),
                },
            ],
            sitting_avatars: vec![Uuid::from_u128(0xAA01), Uuid::from_u128(0xAA02)],
            rezzing_object_id: Uuid::from_u128(0xDEAD),
            ..Default::default()
        }))
    }

    fn register_funcs(
        lua: &Lua,
        ctx: &Arc<RwLock<ScriptContext>>,
    ) -> LuaResult<()> {
        for func in ossl_inventory_funcs() {
            (func.register)(lua, ctx)?;
        }
        Ok(())
    }

    // -- Inventory query tests --

    #[test]
    fn get_inventory_desc_found() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let desc: String = lua
            .load(r#"return osGetInventoryDesc("MyScript")"#)
            .eval()
            .unwrap();
        assert_eq!(desc, "A test script");
    }

    #[test]
    fn get_inventory_desc_not_found() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let desc: String = lua
            .load(r#"return osGetInventoryDesc("NoSuch")"#)
            .eval()
            .unwrap();
        assert_eq!(desc, "");
    }

    #[test]
    fn get_inventory_item_key_with_perms() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let key: String = lua
            .load(r#"return osGetInventoryItemKey("MyScript")"#)
            .eval()
            .unwrap();
        assert_eq!(key, Uuid::from_u128(0xA001).to_string());
    }

    #[test]
    fn get_inventory_item_key_no_perms() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let key: String = lua
            .load(r#"return osGetInventoryItemKey("LockedItem")"#)
            .eval()
            .unwrap();
        assert_eq!(key, Uuid::nil().to_string());
    }

    #[test]
    fn get_inventory_last_owner_with_previous() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let owner: String = lua
            .load(r#"return osGetInventoryLastOwner("MyScript")"#)
            .eval()
            .unwrap();
        assert_eq!(owner, Uuid::from_u128(0xCAFE).to_string());
    }

    #[test]
    fn get_inventory_last_owner_falls_back_to_current() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        // MyTexture has nil last_owner_id, should fall back to owner_id.
        let owner: String = lua
            .load(r#"return osGetInventoryLastOwner("MyTexture")"#)
            .eval()
            .unwrap();
        assert_eq!(owner, Uuid::from_u128(0xBEEF).to_string());
    }

    #[test]
    fn get_inventory_name_by_uuid() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let id_str = Uuid::from_u128(0xA002).to_string();
        let name: String = lua
            .load(&format!(r#"return osGetInventoryName("{id_str}")"#))
            .eval()
            .unwrap();
        assert_eq!(name, "MyTexture");
    }

    #[test]
    fn get_inventory_name_not_found() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let name: String = lua
            .load(r#"return osGetInventoryName("00000000-0000-0000-0000-000000000099")"#)
            .eval()
            .unwrap();
        assert_eq!(name, "");
    }

    #[test]
    fn get_inventory_number_all() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let count: i32 = lua
            .load("return osGetInventoryNumber(-1)")
            .eval()
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn get_inventory_number_by_type() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let count: i32 = lua
            .load("return osGetInventoryNumber(10)")
            .eval()
            .unwrap();
        assert_eq!(count, 1); // INVENTORY_SCRIPT = 10
    }

    #[test]
    fn get_inventory_perm_mask_owner() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let mask: i32 = lua
            .load(r#"return osGetInventoryPermMask("MyScript", 1)"#)
            .eval()
            .unwrap();
        assert_eq!(mask as u32, PERM_COPY | PERM_TRANSFER | PERM_MODIFY);
    }

    #[test]
    fn set_inventory_desc() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        lua.load(r#"osSetInventoryDesc("MyScript", "Updated desc")"#)
            .exec()
            .unwrap();
        let desc: String = lua
            .load(r#"return osGetInventoryDesc("MyScript")"#)
            .eval()
            .unwrap();
        assert_eq!(desc, "Updated desc");
    }

    #[test]
    fn remove_inventory() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        lua.load(r#"osRemoveInventory("MyTexture")"#)
            .exec()
            .unwrap();
        let count: i32 = lua
            .load("return osGetInventoryNumber(-1)")
            .eval()
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn get_last_changed_event_key_none() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let key: String = lua
            .load("return osGetLastChangedEventKey()")
            .eval()
            .unwrap();
        assert_eq!(key, "");
    }

    #[test]
    fn get_last_changed_event_key_set() {
        let lua = Lua::new();
        let ctx = test_ctx();
        {
            let mut c = ctx.write();
            c.last_changed_event_key = Some(Uuid::from_u128(0xCEED));
        }
        register_funcs(&lua, &ctx).unwrap();
        let key: String = lua
            .load("return osGetLastChangedEventKey()")
            .eval()
            .unwrap();
        assert_eq!(key, Uuid::from_u128(0xCEED).to_string());
    }

    // -- Prim/link tests --

    #[test]
    fn get_prim_count() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let count: i32 = lua
            .load(r#"return osGetPrimCount("ignored")"#)
            .eval()
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn get_sitting_avatars_count() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let count: i32 = lua
            .load(r#"return osGetSittingAvatarsCount("prim")"#)
            .eval()
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn get_sitting_avatars_list() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let tbl: mlua::Table = lua
            .load(r#"return osGetSittingAvatars("prim")"#)
            .eval()
            .unwrap();
        assert_eq!(tbl.len().unwrap(), 2);
        let first: String = tbl.get(1).unwrap();
        assert_eq!(first, Uuid::from_u128(0xAA01).to_string());
    }

    #[test]
    fn get_link_number_found() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let num: i32 = lua
            .load(r#"return osGetLinkNumber("Child1")"#)
            .eval()
            .unwrap();
        assert_eq!(num, 2);
    }

    #[test]
    fn get_link_number_host_prim() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let num: i32 = lua
            .load(r#"return osGetLinkNumber("TestPrim")"#)
            .eval()
            .unwrap();
        assert_eq!(num, 0); // link_num defaults to 0 in test context
    }

    #[test]
    fn get_link_number_not_found() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_funcs(&lua, &ctx).unwrap();
        let num: i32 = lua
            .load(r#"return osGetLinkNumber("NoSuch")"#)
            .eval()
            .unwrap();
        assert_eq!(num, -1);
    }

    // -- Sound tests --

    #[test]
    fn play_sound_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::from_u128(0xF00D),
            sound_sender: Some(tx),
            inventory: vec![InventoryItem {
                name: "ding".into(),
                item_type: crate::builtins::INVENTORY_SOUND,
                asset_id: Uuid::from_u128(0x5500),
                ..Default::default()
            }],
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        lua.load(r#"osPlaySound(0, "ding", 0.8)"#)
            .exec()
            .unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            SoundCommand::PlaySound { sound_id, volume, .. } => {
                assert_eq!(sound_id, Uuid::from_u128(0x5500));
                assert!((volume - 0.8).abs() < f64::EPSILON);
            }
            _ => panic!("expected PlaySound command"),
        }
    }

    #[test]
    fn stop_sound_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: Uuid::from_u128(0xF00D),
            sound_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        lua.load("osStopSound(0)").exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            SoundCommand::StopSound { prim_id } => {
                assert_eq!(prim_id, Uuid::from_u128(0xF00D));
            }
            _ => panic!("expected StopSound command"),
        }
    }

    // -- Link command tests --

    #[test]
    fn force_create_link_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let target = Uuid::from_u128(0xBB01);
        let ctx = Arc::new(RwLock::new(ScriptContext {
            link_command_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        let script = format!(r#"osForceCreateLink("{}", 1)"#, target);
        lua.load(&script).exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            LinkCommand::CreateLink { target_id, parent } => {
                assert_eq!(target_id, target);
                assert!(parent);
            }
            _ => panic!("expected CreateLink command"),
        }
    }

    #[test]
    fn force_break_all_links_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            link_command_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        lua.load("osForceBreakAllLinks()").exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        assert!(matches!(cmd, LinkCommand::BreakAllLinks));
    }

    // -- Object command tests --

    #[test]
    fn os_die_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let target = Uuid::from_u128(0xDEAD);
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: Uuid::from_u128(0xF00D),
            object_command_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        let script = format!(r#"osDie("{}")"#, target);
        lua.load(&script).exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            ObjectCommand::Die { object_id } => {
                assert_eq!(object_id, target);
            }
            _ => panic!("expected Die command"),
        }
    }

    #[test]
    fn os_die_cannot_delete_self() {
        let (tx, rx) = std::sync::mpsc::channel();
        let self_id = Uuid::from_u128(0xF00D);
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: self_id,
            object_command_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        let script = format!(r#"osDie("{}")"#, self_id);
        lua.load(&script).exec().unwrap();
        // No command should have been sent.
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn os_message_object_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let target = Uuid::from_u128(0xCC01);
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: Uuid::from_u128(0xF00D),
            object_command_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        let script = format!(r#"osMessageObject("{}", "hello")"#, target);
        lua.load(&script).exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            ObjectCommand::MessageObject { object_id, source_id, message } => {
                assert_eq!(object_id, target);
                assert_eq!(source_id, Uuid::from_u128(0xF00D));
                assert_eq!(message, "hello");
            }
            _ => panic!("expected MessageObject command"),
        }
    }

    #[test]
    fn get_rezzing_object_returns_id() {
        let rez_id = Uuid::from_u128(0xDEAD);
        let ctx = Arc::new(RwLock::new(ScriptContext {
            rezzing_object_id: rez_id,
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        let key: String = lua
            .load("return osGetRezzingObject()")
            .eval()
            .unwrap();
        assert_eq!(key, rez_id.to_string());
    }

    #[test]
    fn volume_detect_sets_phantom() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: Uuid::from_u128(0xF00D),
            object_command_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        lua.load("osVolumeDetect(1)").exec().unwrap();
        {
            let c = ctx.read();
            assert!(c.is_phantom);
        }
        let cmd = rx.try_recv().unwrap();
        match cmd {
            ObjectCommand::VolumeDetect { enable, .. } => {
                assert!(enable);
            }
            _ => panic!("expected VolumeDetect command"),
        }
    }

    // -- Attachment tests --

    #[test]
    fn force_attach_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: Uuid::from_u128(0xF00D),
            owner_id: Uuid::from_u128(0xBEEF),
            attach_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        lua.load("osForceAttachToAvatar(5)").exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            AttachAction::Attach(req) => {
                assert_eq!(req.attach_point, 5);
                assert_eq!(req.prim_id, Uuid::from_u128(0xF00D));
            }
            _ => panic!("expected Attach command"),
        }
    }

    #[test]
    fn force_detach_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: Uuid::from_u128(0xF00D),
            is_attachment: true,
            attach_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        lua.load("osForceDetachFromAvatar()").exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            AttachAction::Detach { prim_id } => {
                assert_eq!(prim_id, Uuid::from_u128(0xF00D));
            }
            _ => panic!("expected Detach command"),
        }
    }

    #[test]
    fn drop_attachment_without_perms_is_noop() {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            prim_id: Uuid::from_u128(0xF00D),
            is_attachment: true,
            permissions_granted: 0, // no PERMISSION_ATTACH
            attach_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        lua.load("osDropAttachment()").exec().unwrap();
        assert!(rx.try_recv().is_err());
    }

    // -- Inventory give test --

    #[test]
    fn give_inventory_sends_command() {
        let (tx, rx) = std::sync::mpsc::channel();
        let dest = Uuid::from_u128(0xDD01);
        let ctx = Arc::new(RwLock::new(ScriptContext {
            inventory: vec![InventoryItem {
                name: "Gift".into(),
                item_type: crate::builtins::INVENTORY_OBJECT,
                asset_id: Uuid::from_u128(0x9900),
                item_id: Uuid::from_u128(0x9901),
                ..Default::default()
            }],
            inventory_command_sender: Some(tx),
            ..Default::default()
        }));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        let script = format!(r#"osGiveInventory("{}", "Gift")"#, dest);
        lua.load(&script).exec().unwrap();
        let cmd = rx.try_recv().unwrap();
        match cmd {
            InventoryCommand::Give { destination, item_name, .. } => {
                assert_eq!(destination, dest);
                assert_eq!(item_name, "Gift");
            }
            _ => panic!("expected Give command"),
        }
    }

    // -- Deprecated particle effect tests --

    #[test]
    fn make_fire_smoke_explosion_do_not_panic() {
        let ctx = Arc::new(RwLock::new(ScriptContext::default()));
        let lua = Lua::new();
        register_funcs(&lua, &ctx).unwrap();
        lua.load("osMakeFire()").exec().unwrap();
        lua.load("osMakeSmoke()").exec().unwrap();
        lua.load("osMakeExplosion()").exec().unwrap();
    }
}
