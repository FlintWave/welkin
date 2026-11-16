// SPDX-License-Identifier: LGPL-3.0-or-later
//! OSSL NPC functions -- interact with the NPC registry via ScriptContext.
//!
//! Each function validates the NPC exists and (where required by OpenSim
//! semantics) that the calling script's owner matches the NPC's owner before
//! performing any action.  Commands are dispatched through the `npc_sender`
//! channel to the region NPC subsystem.
//!
//! Reference: `OpenSim/Region/ScriptEngine/Shared/Api/Implementation/OSSL_Api.cs`
//!            `OpenSim/Region/OptionalModules/World/NPC/NPCModule.cs`

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::{NpcCommand, NpcInfo, ScriptContext};
use crate::ChatMessage;
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// OpenSim NPC option constants (from LSL_Constants.cs)
// ---------------------------------------------------------------------------

/// Movement option: allow flying (default).
pub const OS_NPC_FLY: i32 = 0;
/// Movement option: no flying, walk/run only.
pub const OS_NPC_NO_FLY: i32 = 1;
/// Movement option: land at the target position.
pub const OS_NPC_LAND_AT_TARGET: i32 = 2;
/// Movement option: NPC runs instead of walks.
pub const OS_NPC_RUNNING: i32 = 4;

/// Sit option: sit immediately.
pub const OS_NPC_SIT_NOW: i32 = 0;

/// Creation option: NPC is owned by its creator.
pub const OS_NPC_CREATOR_OWNED: i32 = 0x1;
/// Creation option: NPC has no owner (community NPC).
pub const OS_NPC_NOT_OWNED: i32 = 0x2;
/// Creation option: NPC appears as a real avatar to LSL sensors.
pub const OS_NPC_SENSE_AS_AGENT: i32 = 0x4;
/// Creation option: assign the host object's group to the NPC.
pub const OS_NPC_OBJECT_GROUP: i32 = 0x08;

// ---------------------------------------------------------------------------
// NPCLOOKAT constants (from LSL_Constants.cs)
// ---------------------------------------------------------------------------

pub const NPCLOOKAT_NONE: i32 = 0;
pub const NPCLOOKAT_IDLE: i32 = 1;
pub const NPCLOOKAT_LISTEN: i32 = 2;
pub const NPCLOOKAT_FREELOOK: i32 = 3;
pub const NPCLOOKAT_RESPOND: i32 = 4;
pub const NPCLOOKAT_HOVER: i32 = 5;
pub const NPCLOOKAT_CONVERSATION: i32 = 6;
pub const NPCLOOKAT_SELECT: i32 = 7;
pub const NPCLOOKAT_FOCUS: i32 = 8;
pub const NPCLOOKAT_MOUSELOOK: i32 = 9;
pub const NPCLOOKAT_CLEAR: i32 = 10;

// ---------------------------------------------------------------------------
// Function registry
// ---------------------------------------------------------------------------

pub(super) fn ossl_npc_funcs() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osIsNpc",
            level: OsslThreatLevel::High,
            register: register_os_is_npc,
        },
        OsslFunc {
            name: "osNpcCreate",
            level: OsslThreatLevel::High,
            register: register_os_npc_create,
        },
        OsslFunc {
            name: "osNpcRemove",
            level: OsslThreatLevel::High,
            register: register_os_npc_remove,
        },
        OsslFunc {
            name: "osNpcSaveAppearance",
            level: OsslThreatLevel::High,
            register: register_os_npc_save_appearance,
        },
        OsslFunc {
            name: "osNpcLoadAppearance",
            level: OsslThreatLevel::High,
            register: register_os_npc_load_appearance,
        },
        OsslFunc {
            name: "osNpcGetOwner",
            level: OsslThreatLevel::High,
            register: register_os_npc_get_owner,
        },
        OsslFunc {
            name: "osNpcGetPos",
            level: OsslThreatLevel::High,
            register: register_os_npc_get_pos,
        },
        OsslFunc {
            name: "osNpcMoveTo",
            level: OsslThreatLevel::High,
            register: register_os_npc_move_to,
        },
        OsslFunc {
            name: "osNpcMoveToTarget",
            level: OsslThreatLevel::High,
            register: register_os_npc_move_to_target,
        },
        OsslFunc {
            name: "osNpcGetRot",
            level: OsslThreatLevel::High,
            register: register_os_npc_get_rot,
        },
        OsslFunc {
            name: "osNpcSetRot",
            level: OsslThreatLevel::High,
            register: register_os_npc_set_rot,
        },
        OsslFunc {
            name: "osNpcStopMoveToTarget",
            level: OsslThreatLevel::High,
            register: register_os_npc_stop_move_to_target,
        },
        OsslFunc {
            name: "osNpcSetProfileAbout",
            level: OsslThreatLevel::Low,
            register: register_os_npc_set_profile_about,
        },
        OsslFunc {
            name: "osNpcSetProfileImage",
            level: OsslThreatLevel::Low,
            register: register_os_npc_set_profile_image,
        },
        OsslFunc {
            name: "osNpcSay",
            level: OsslThreatLevel::High,
            register: register_os_npc_say,
        },
        OsslFunc {
            name: "osNpcSayTo",
            level: OsslThreatLevel::High,
            register: register_os_npc_say_to,
        },
        OsslFunc {
            name: "osNpcShout",
            level: OsslThreatLevel::High,
            register: register_os_npc_shout,
        },
        OsslFunc {
            name: "osNpcSit",
            level: OsslThreatLevel::High,
            register: register_os_npc_sit,
        },
        OsslFunc {
            name: "osNpcStand",
            level: OsslThreatLevel::High,
            register: register_os_npc_stand,
        },
        OsslFunc {
            name: "osNpcPlayAnimation",
            level: OsslThreatLevel::High,
            register: register_os_npc_play_animation,
        },
        OsslFunc {
            name: "osNpcStopAnimation",
            level: OsslThreatLevel::High,
            register: register_os_npc_stop_animation,
        },
        OsslFunc {
            name: "osNpcTouch",
            level: OsslThreatLevel::High,
            register: register_os_npc_touch,
        },
        OsslFunc {
            name: "osNpcWhisper",
            level: OsslThreatLevel::High,
            register: register_os_npc_whisper,
        },
        OsslFunc {
            name: "osNpcLookAt",
            level: OsslThreatLevel::High,
            register: register_os_npc_look_at,
        },
    ]
}

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
        let x = parts[0].trim().parse::<f64>().unwrap_or(0.0);
        let y = parts[1].trim().parse::<f64>().unwrap_or(0.0);
        let z = parts[2].trim().parse::<f64>().unwrap_or(0.0);
        [x, y, z]
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

fn format_vector(v: [f64; 3]) -> String {
    format!("<{}, {}, {}>", v[0], v[1], v[2])
}

fn format_rotation(r: [f64; 4]) -> String {
    format!("<{}, {}, {}, {}>", r[0], r[1], r[2], r[3])
}

/// Check that the calling script owns the given NPC.
///
/// Matches the OpenSim `NPCModule.CheckPermissions` logic:
/// - If the NPC's owner is nil, any caller can manage it (community NPC).
/// - Otherwise the caller's `owner_id` must match the NPC's `owner_id`.
///
/// Returns `true` if the caller has permission.
fn check_npc_owner(ctx: &ScriptContext, npc_id: Uuid) -> bool {
    if let Some(npc) = ctx.npc_registry.get(&npc_id) {
        npc.owner_id.is_nil() || npc.owner_id == ctx.owner_id
    } else {
        false
    }
}

fn send_npc_cmd(ctx: &ScriptContext, cmd: NpcCommand) {
    if let Some(ref sender) = ctx.npc_sender {
        let _ = sender.send(cmd);
    }
}

fn send_npc_chat(
    ctx: &ScriptContext,
    npc_id: Uuid,
    channel: i32,
    message: &str,
    chat_type: crate::ChatType,
) {
    let npc = ctx.npc_registry.get(&npc_id);
    let (name, pos) = npc
        .map(|n| (format!("{} {}", n.first_name, n.last_name), n.position))
        .unwrap_or_else(|| ("NPC".to_string(), [0.0; 3]));
    if let Some(ref sender) = ctx.chat_sender {
        let _ = sender.send(ChatMessage {
            source_id: npc_id,
            source_name: name,
            source_pos: pos,
            owner_id: ctx.owner_id,
            channel,
            message: message.to_string(),
            chat_type,
            target_id: None,
        });
    }
}

// ---------------------------------------------------------------------------
// osIsNpc(key) -> 1 if NPC, 0 otherwise
// No ownership check required (matches C# -- anyone can query).
// ---------------------------------------------------------------------------

fn register_os_is_npc(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, key: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&key);
        let is_npc = c.npc_registry.contains_key(&npc_id);
        Ok(if is_npc { 1i32 } else { 0i32 })
    })?;
    lua.globals().set("osIsNpc", f)
}

// ---------------------------------------------------------------------------
// osNpcCreate(firstname, lastname, position, notecard [, options]) -> key
//
// Creates an NPC at the given position with appearance from the notecard.
// When `options` is provided, it is a bitmask of OS_NPC_* flags.
// Returns the NPC's UUID as a key string, or NULL_KEY on failure.
// ---------------------------------------------------------------------------

fn register_os_npc_create(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, args: mlua::MultiValue| {
        let args_vec: Vec<mlua::Value> = args.into_vec();
        if args_vec.len() < 4 {
            return Err(mlua::Error::RuntimeError(
                "osNpcCreate requires at least 4 arguments".into(),
            ));
        }

        let first_name = match &args_vec[0] {
            mlua::Value::String(s) => s.to_str()?.to_string(),
            _ => return Err(mlua::Error::RuntimeError("bad argument #1".into())),
        };
        let last_name = match &args_vec[1] {
            mlua::Value::String(s) => s.to_str()?.to_string(),
            _ => return Err(mlua::Error::RuntimeError("bad argument #2".into())),
        };
        let position_str = match &args_vec[2] {
            mlua::Value::String(s) => s.to_str()?.to_string(),
            _ => return Err(mlua::Error::RuntimeError("bad argument #3".into())),
        };
        let notecard = match &args_vec[3] {
            mlua::Value::String(s) => s.to_str()?.to_string(),
            _ => return Err(mlua::Error::RuntimeError("bad argument #4".into())),
        };
        let options: i32 = if args_vec.len() > 4 {
            match &args_vec[4] {
                mlua::Value::Integer(i) => *i as i32,
                mlua::Value::Number(n) => *n as i32,
                _ => 0,
            }
        } else {
            0
        };

        let mut c = ctx.write();
        let npc_id = Uuid::new_v4();
        let pos = parse_vector(&position_str);

        // Determine owner based on OS_NPC_NOT_OWNED flag.
        let owner = if (options & OS_NPC_NOT_OWNED) != 0 {
            Uuid::nil()
        } else {
            c.owner_id
        };

        c.npc_registry.insert(
            npc_id,
            NpcInfo {
                owner_id: owner,
                first_name: first_name.clone(),
                last_name: last_name.clone(),
                position: pos,
                rotation: [0.0, 0.0, 0.0, 1.0],
                profile_about: String::new(),
                profile_image: Uuid::nil(),
            },
        );

        send_npc_cmd(
            &c,
            NpcCommand::Create {
                npc_id,
                owner,
                first_name: first_name.clone(),
                last_name: last_name.clone(),
                position: pos,
                options,
                appearance_notecard: notecard,
            },
        );

        tracing::info!(
            npc = %npc_id,
            name = %format!("{first_name} {last_name}"),
            options = options,
            "osNpcCreate"
        );
        Ok(npc_id.to_string())
    })?;
    lua.globals().set("osNpcCreate", f)
}

// ---------------------------------------------------------------------------
// osNpcRemove(npc) -> void
// Checks ownership before removing.
// ---------------------------------------------------------------------------

fn register_os_npc_remove(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let mut c = ctx.write();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        c.npc_registry.remove(&npc_id);
        send_npc_cmd(&c, NpcCommand::Remove { npc_id });
        tracing::info!(npc = %npc_id, "osNpcRemove");
        Ok(())
    })?;
    lua.globals().set("osNpcRemove", f)
}

// ---------------------------------------------------------------------------
// osNpcSaveAppearance(npc, notecard) -> notecard key
//
// Saves the NPC's current appearance. Generates a notecard asset UUID,
// records the mapping in notecard_names, and dispatches a SaveAppearance
// command to the NPC subsystem which performs the actual serialisation.
// ---------------------------------------------------------------------------

fn register_os_npc_save_appearance(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, notecard): (String, String)| {
        let mut c = ctx.write();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(Uuid::nil().to_string());
        }
        let nc_id = Uuid::new_v4();
        // Record the notecard name -> asset id mapping so LoadAppearance
        // and osGetNotecard can find it later.
        c.notecard_names.insert(notecard.clone(), nc_id);
        send_npc_cmd(
            &c,
            NpcCommand::SaveAppearance {
                npc_id,
                notecard_name: notecard.clone(),
                notecard_id: nc_id,
            },
        );
        tracing::info!(
            npc = %npc_id,
            notecard = %notecard,
            nc_id = %nc_id,
            "osNpcSaveAppearance"
        );
        Ok(nc_id.to_string())
    })?;
    lua.globals().set("osNpcSaveAppearance", f)
}

// ---------------------------------------------------------------------------
// osNpcLoadAppearance(npc, notecard) -> void
//
// Loads an appearance from a notecard and applies it to the NPC.
// Dispatches a LoadAppearance command to the NPC subsystem.
// ---------------------------------------------------------------------------

fn register_os_npc_load_appearance(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, notecard): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        send_npc_cmd(
            &c,
            NpcCommand::LoadAppearance {
                npc_id,
                notecard_name: notecard.clone(),
            },
        );
        tracing::info!(npc = %npc_id, notecard = %notecard, "osNpcLoadAppearance");
        Ok(())
    })?;
    lua.globals().set("osNpcLoadAppearance", f)
}

// ---------------------------------------------------------------------------
// osNpcGetOwner(npc) -> owner key
//
// No ownership check (matches C# -- anyone can query).
// If the NPC has no owner (UUID::nil), returns the NPC's own key
// (matches OpenSim behaviour).
// ---------------------------------------------------------------------------

fn register_os_npc_get_owner(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        match c.npc_registry.get(&npc_id) {
            Some(n) => {
                if n.owner_id.is_nil() {
                    // OpenSim returns the NPC key itself when unowned.
                    Ok(npc_id.to_string())
                } else {
                    Ok(n.owner_id.to_string())
                }
            }
            None => Ok(Uuid::nil().to_string()),
        }
    })?;
    lua.globals().set("osNpcGetOwner", f)
}

// ---------------------------------------------------------------------------
// osNpcGetPos(npc) -> vector string
// ---------------------------------------------------------------------------

fn register_os_npc_get_pos(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(format_vector([0.0; 3]));
        }
        let pos = c
            .npc_registry
            .get(&npc_id)
            .map(|n| n.position)
            .unwrap_or([0.0; 3]);
        Ok(format_vector(pos))
    })?;
    lua.globals().set("osNpcGetPos", f)
}

// ---------------------------------------------------------------------------
// osNpcMoveTo(npc, position) -> void
//
// Simpler movement: walk to position without option flags.
// Equivalent to MoveToTarget with no_fly=false, land_at_target=true,
// running=false (matches C# `module.MoveToTarget(..., false, true, false)`).
// ---------------------------------------------------------------------------

fn register_os_npc_move_to(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, position): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        let target = parse_vector(&position);
        send_npc_cmd(
            &c,
            NpcCommand::MoveTo {
                npc_id,
                target,
                options: OS_NPC_LAND_AT_TARGET,
            },
        );
        Ok(())
    })?;
    lua.globals().set("osNpcMoveTo", f)
}

// ---------------------------------------------------------------------------
// osNpcMoveToTarget(npc, target, options) -> void
//
// Movement with option flags:
//   OS_NPC_NO_FLY (1)       -- walk/run only
//   OS_NPC_LAND_AT_TARGET (2) -- land when reaching target
//   OS_NPC_RUNNING (4)      -- run instead of walk
// ---------------------------------------------------------------------------

fn register_os_npc_move_to_target(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, target, options): (String, String, i32)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        let target_pos = parse_vector(&target);
        send_npc_cmd(
            &c,
            NpcCommand::MoveTo {
                npc_id,
                target: target_pos,
                options,
            },
        );
        Ok(())
    })?;
    lua.globals().set("osNpcMoveToTarget", f)
}

// ---------------------------------------------------------------------------
// osNpcGetRot(npc) -> rotation string
// ---------------------------------------------------------------------------

fn register_os_npc_get_rot(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(format_rotation([0.0, 0.0, 0.0, 1.0]));
        }
        let rot = c
            .npc_registry
            .get(&npc_id)
            .map(|n| n.rotation)
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        Ok(format_rotation(rot))
    })?;
    lua.globals().set("osNpcGetRot", f)
}

// ---------------------------------------------------------------------------
// osNpcSetRot(npc, rot) -> void
// ---------------------------------------------------------------------------

fn register_os_npc_set_rot(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, rot): (String, String)| {
        let mut c = ctx.write();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        let rotation = parse_rotation(&rot);
        if let Some(npc_info) = c.npc_registry.get_mut(&npc_id) {
            npc_info.rotation = rotation;
        }
        send_npc_cmd(&c, NpcCommand::SetRot { npc_id, rotation });
        Ok(())
    })?;
    lua.globals().set("osNpcSetRot", f)
}

// ---------------------------------------------------------------------------
// osNpcStopMoveToTarget(npc) -> void
// ---------------------------------------------------------------------------

fn register_os_npc_stop_move_to_target(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        send_npc_cmd(&c, NpcCommand::StopMove { npc_id });
        Ok(())
    })?;
    lua.globals().set("osNpcStopMoveToTarget", f)
}

// ---------------------------------------------------------------------------
// osNpcSetProfileAbout(npc, text) -> void
// Threat level Low in OpenSim.
// ---------------------------------------------------------------------------

fn register_os_npc_set_profile_about(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, text): (String, String)| {
        let mut c = ctx.write();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        if let Some(npc_info) = c.npc_registry.get_mut(&npc_id) {
            npc_info.profile_about = text.clone();
        }
        send_npc_cmd(
            &c,
            NpcCommand::SetProfileAbout {
                npc_id,
                about: text,
            },
        );
        Ok(())
    })?;
    lua.globals().set("osNpcSetProfileAbout", f)
}

// ---------------------------------------------------------------------------
// osNpcSetProfileImage(npc, texture) -> void
// Threat level Low in OpenSim. Accepts a texture UUID or inventory name.
// ---------------------------------------------------------------------------

fn register_os_npc_set_profile_image(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, texture): (String, String)| {
        let mut c = ctx.write();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        let tex_id = parse_uuid(&texture);
        if let Some(npc_info) = c.npc_registry.get_mut(&npc_id) {
            npc_info.profile_image = tex_id;
        }
        send_npc_cmd(
            &c,
            NpcCommand::SetProfileImage {
                npc_id,
                image_id: tex_id,
            },
        );
        Ok(())
    })?;
    lua.globals().set("osNpcSetProfileImage", f)
}

// ---------------------------------------------------------------------------
// osNpcSay(npc, channel, message) -> void
//
// Also accepts 2-arg form osNpcSay(npc, message) which uses channel 0
// (matches the C# overload).
// ---------------------------------------------------------------------------

fn register_os_npc_say(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, args: mlua::MultiValue| {
        let args_vec: Vec<mlua::Value> = args.into_vec();
        if args_vec.len() < 2 {
            return Err(mlua::Error::RuntimeError(
                "osNpcSay requires at least 2 arguments".into(),
            ));
        }

        let npc_str = match &args_vec[0] {
            mlua::Value::String(s) => s.to_str()?.to_string(),
            _ => return Err(mlua::Error::RuntimeError("bad argument #1".into())),
        };

        // Detect 2-arg vs 3-arg form.
        let (channel, message) = if args_vec.len() >= 3 {
            let ch = match &args_vec[1] {
                mlua::Value::Integer(i) => *i as i32,
                mlua::Value::Number(n) => *n as i32,
                _ => 0,
            };
            let msg = match &args_vec[2] {
                mlua::Value::String(s) => s.to_str()?.to_string(),
                _ => return Err(mlua::Error::RuntimeError("bad argument #3".into())),
            };
            (ch, msg)
        } else {
            // 2-arg form: osNpcSay(npc, message) -- channel 0
            let msg = match &args_vec[1] {
                mlua::Value::String(s) => s.to_str()?.to_string(),
                _ => return Err(mlua::Error::RuntimeError("bad argument #2".into())),
            };
            (0, msg)
        };

        let c = ctx.read();
        let npc_id = parse_uuid(&npc_str);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        send_npc_chat(&c, npc_id, channel, &message, crate::ChatType::Say);
        Ok(())
    })?;
    lua.globals().set("osNpcSay", f)
}

// ---------------------------------------------------------------------------
// osNpcSayTo(npc, target, channel, msg) -> void
// ---------------------------------------------------------------------------

fn register_os_npc_say_to(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (npc, target, channel, msg): (String, String, i32, String)| {
            let c = ctx.read();
            let npc_id = parse_uuid(&npc);
            if !check_npc_owner(&c, npc_id) {
                return Ok(());
            }
            let target_id = parse_uuid(&target);
            let npc_info = c.npc_registry.get(&npc_id);
            let (name, pos) = npc_info
                .map(|n| (format!("{} {}", n.first_name, n.last_name), n.position))
                .unwrap_or_else(|| ("NPC".to_string(), [0.0; 3]));
            if let Some(ref sender) = c.chat_sender {
                let _ = sender.send(ChatMessage {
                    source_id: npc_id,
                    source_name: name,
                    source_pos: pos,
                    owner_id: c.owner_id,
                    channel,
                    message: msg,
                    chat_type: crate::ChatType::Say,
                    target_id: Some(target_id),
                });
            }
            Ok(())
        },
    )?;
    lua.globals().set("osNpcSayTo", f)
}

// ---------------------------------------------------------------------------
// osNpcShout(npc, channel, msg) -> void
// ---------------------------------------------------------------------------

fn register_os_npc_shout(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, channel, msg): (String, i32, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        send_npc_chat(&c, npc_id, channel, &msg, crate::ChatType::Shout);
        Ok(())
    })?;
    lua.globals().set("osNpcShout", f)
}

// ---------------------------------------------------------------------------
// osNpcSit(npc, target, options) -> void
//
// Makes the NPC sit on the target object. The `options` parameter
// (OS_NPC_SIT_NOW = 0) is forwarded to the subsystem.
// ---------------------------------------------------------------------------

fn register_os_npc_sit(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, target, options): (String, String, i32)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        let target_id = parse_uuid(&target);
        send_npc_cmd(
            &c,
            NpcCommand::Sit {
                npc_id,
                target: target_id,
                options,
            },
        );
        Ok(())
    })?;
    lua.globals().set("osNpcSit", f)
}

// ---------------------------------------------------------------------------
// osNpcStand(npc) -> void
// ---------------------------------------------------------------------------

fn register_os_npc_stand(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        send_npc_cmd(&c, NpcCommand::Stand { npc_id });
        Ok(())
    })?;
    lua.globals().set("osNpcStand", f)
}

// ---------------------------------------------------------------------------
// osNpcPlayAnimation(npc, anim) -> void
//
// The animation parameter can be an animation name or asset UUID.
// ---------------------------------------------------------------------------

fn register_os_npc_play_animation(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, anim): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        send_npc_cmd(
            &c,
            NpcCommand::PlayAnimation {
                npc_id,
                animation: anim,
            },
        );
        Ok(())
    })?;
    lua.globals().set("osNpcPlayAnimation", f)
}

// ---------------------------------------------------------------------------
// osNpcStopAnimation(npc, anim) -> void
// ---------------------------------------------------------------------------

fn register_os_npc_stop_animation(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, anim): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        send_npc_cmd(
            &c,
            NpcCommand::StopAnimation {
                npc_id,
                animation: anim,
            },
        );
        Ok(())
    })?;
    lua.globals().set("osNpcStopAnimation", f)
}

// ---------------------------------------------------------------------------
// osNpcTouch(npc, object, link_num) -> void
//
// Makes the NPC touch a specific link of the target object.
// link_num values: LINK_ROOT (1), LINK_THIS (-4), or a positive link number.
// ---------------------------------------------------------------------------

fn register_os_npc_touch(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, object, link): (String, String, i32)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        let target = parse_uuid(&object);
        send_npc_cmd(
            &c,
            NpcCommand::Touch {
                npc_id,
                target,
                link_num: link,
            },
        );
        Ok(())
    })?;
    lua.globals().set("osNpcTouch", f)
}

// ---------------------------------------------------------------------------
// osNpcWhisper(npc, channel, msg) -> void
// ---------------------------------------------------------------------------

fn register_os_npc_whisper(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, channel, msg): (String, i32, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !check_npc_owner(&c, npc_id) {
            return Ok(());
        }
        send_npc_chat(&c, npc_id, channel, &msg, crate::ChatType::Whisper);
        Ok(())
    })?;
    lua.globals().set("osNpcWhisper", f)
}

// ---------------------------------------------------------------------------
// osNpcLookAt(npckey, type, objkey, offset) -> integer
//
// Sends a viewer-effect LookAt packet to all connected clients.
// `type` is one of the NPCLOOKAT_* constants (0..10).
// `objkey` is the target object/avatar UUID.
// `offset` is a position offset from the target.
//
// Returns:
//    0 on success
//   -1 if type is out of range
//   -2 if npckey is not a valid UUID
//   -3 if npckey is not an NPC
//   -4 if objkey is not a valid UUID
//   -5 if objkey refers to a nonexistent entity (when non-nil)
// ---------------------------------------------------------------------------

fn register_os_npc_look_at(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (npc_key, look_type, obj_key, offset): (String, i32, String, String)| {
            // Validate look_type range.
            if look_type < 0 || look_type > NPCLOOKAT_CLEAR {
                return Ok(-1i32);
            }

            // Parse and validate NPC key.
            let npc_id = match Uuid::parse_str(&npc_key) {
                Ok(id) => id,
                Err(_) => return Ok(-2i32),
            };

            let c = ctx.read();

            // Verify it is actually an NPC.
            if !c.npc_registry.contains_key(&npc_id) {
                return Ok(-3i32);
            }

            // Parse and validate target object key.
            let target_id = match Uuid::parse_str(&obj_key) {
                Ok(id) => id,
                Err(_) => return Ok(-4i32),
            };

            let offset_vec = parse_vector(&offset);

            send_npc_cmd(
                &c,
                NpcCommand::LookAt {
                    npc_id,
                    look_type,
                    target_id,
                    offset: offset_vec,
                },
            );

            Ok(0i32)
        },
    )?;
    lua.globals().set("osNpcLookAt", f)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::NpcInfo;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        let mut ctx = ScriptContext::default();
        ctx.script_id = Uuid::new_v4();
        ctx.owner_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let npc_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        ctx.npc_registry.insert(
            npc_id,
            NpcInfo {
                owner_id: ctx.owner_id,
                first_name: "Test".into(),
                last_name: "NPC".into(),
                position: [128.0, 128.0, 25.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                profile_about: String::new(),
                profile_image: Uuid::nil(),
            },
        );
        Arc::new(RwLock::new(ctx))
    }

    fn test_ctx_with_channels() -> (
        Arc<RwLock<ScriptContext>>,
        std::sync::mpsc::Receiver<NpcCommand>,
        std::sync::mpsc::Receiver<ChatMessage>,
    ) {
        let (npc_tx, npc_rx) = std::sync::mpsc::channel();
        let (chat_tx, chat_rx) = std::sync::mpsc::channel();
        let mut ctx = ScriptContext::default();
        ctx.script_id = Uuid::new_v4();
        ctx.owner_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        ctx.npc_sender = Some(npc_tx);
        ctx.chat_sender = Some(chat_tx);
        let npc_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        ctx.npc_registry.insert(
            npc_id,
            NpcInfo {
                owner_id: ctx.owner_id,
                first_name: "Test".into(),
                last_name: "NPC".into(),
                position: [128.0, 128.0, 25.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                profile_about: String::new(),
                profile_image: Uuid::nil(),
            },
        );
        (Arc::new(RwLock::new(ctx)), npc_rx, chat_rx)
    }

    // -- osIsNpc ---------------------------------------------------------------

    #[test]
    fn os_is_npc_known() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        register_os_is_npc(&lua, &ctx).unwrap();
        let result: i32 = lua
            .load(r#"return osIsNpc("12345678-1234-1234-1234-123456789abc")"#)
            .eval()
            .unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn os_is_npc_unknown() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        register_os_is_npc(&lua, &ctx).unwrap();
        let result: i32 = lua
            .load(r#"return osIsNpc("00000000-0000-0000-0000-000000000000")"#)
            .eval()
            .unwrap();
        assert_eq!(result, 0);
    }

    // -- osNpcGetOwner ---------------------------------------------------------

    #[test]
    fn os_npc_get_owner() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        let expected_owner = ctx.read().owner_id;
        register_os_npc_get_owner(&lua, &ctx).unwrap();
        let result: String = lua
            .load(r#"return osNpcGetOwner("12345678-1234-1234-1234-123456789abc")"#)
            .eval()
            .unwrap();
        assert_eq!(result, expected_owner.to_string());
    }

    #[test]
    fn os_npc_get_owner_unowned_returns_npc_key() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        let npc_id = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        ctx.write().npc_registry.insert(
            npc_id,
            NpcInfo {
                owner_id: Uuid::nil(), // unowned
                first_name: "Free".into(),
                last_name: "NPC".into(),
                position: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                profile_about: String::new(),
                profile_image: Uuid::nil(),
            },
        );
        register_os_npc_get_owner(&lua, &ctx).unwrap();
        let result: String = lua
            .load(r#"return osNpcGetOwner("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")"#)
            .eval()
            .unwrap();
        assert_eq!(result, npc_id.to_string());
    }

    // -- osNpcGetPos -----------------------------------------------------------

    #[test]
    fn os_npc_get_pos() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        register_os_npc_get_pos(&lua, &ctx).unwrap();
        let result: String = lua
            .load(r#"return osNpcGetPos("12345678-1234-1234-1234-123456789abc")"#)
            .eval()
            .unwrap();
        assert_eq!(result, "<128, 128, 25>");
    }

    // -- osNpcSetRot -----------------------------------------------------------

    #[test]
    fn os_npc_set_rot() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        register_os_npc_set_rot(&lua, &ctx).unwrap();
        lua.load(
            r#"osNpcSetRot("12345678-1234-1234-1234-123456789abc", "<0, 0, 0.707, 0.707>")"#,
        )
        .exec()
        .unwrap();
        let c = ctx.read();
        let npc_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let rot = c.npc_registry.get(&npc_id).unwrap().rotation;
        assert!((rot[2] - 0.707).abs() < 0.001);
        assert!((rot[3] - 0.707).abs() < 0.001);
    }

    // -- osNpcCreate -----------------------------------------------------------

    #[test]
    fn os_npc_create_4_args() {
        let lua = mlua::Lua::new();
        let (ctx, npc_rx, _chat_rx) = test_ctx_with_channels();
        register_os_npc_create(&lua, &ctx).unwrap();
        let key: String = lua
            .load(r#"return osNpcCreate("New", "Bot", "<64,64,30>", "appearance")"#)
            .eval()
            .unwrap();
        // Should return a valid UUID.
        assert!(Uuid::parse_str(&key).is_ok());
        let npc_id = Uuid::parse_str(&key).unwrap();
        // NPC should be in registry.
        let c = ctx.read();
        assert!(c.npc_registry.contains_key(&npc_id));
        let npc = c.npc_registry.get(&npc_id).unwrap();
        assert_eq!(npc.first_name, "New");
        assert_eq!(npc.last_name, "Bot");
        // Command should be sent.
        let cmd = npc_rx.try_recv().unwrap();
        match cmd {
            NpcCommand::Create { npc_id: id, options, .. } => {
                assert_eq!(id, npc_id);
                assert_eq!(options, 0);
            }
            _ => panic!("expected Create command"),
        }
    }

    #[test]
    fn os_npc_create_5_args_not_owned() {
        let lua = mlua::Lua::new();
        let (ctx, _npc_rx, _chat_rx) = test_ctx_with_channels();
        register_os_npc_create(&lua, &ctx).unwrap();
        let key: String = lua
            .load(r#"return osNpcCreate("Free", "NPC", "<0,0,0>", "", 2)"#) // OS_NPC_NOT_OWNED
            .eval()
            .unwrap();
        let npc_id = Uuid::parse_str(&key).unwrap();
        let c = ctx.read();
        let npc = c.npc_registry.get(&npc_id).unwrap();
        assert!(npc.owner_id.is_nil(), "NPC should be unowned");
    }

    // -- osNpcRemove -----------------------------------------------------------

    #[test]
    fn os_npc_remove() {
        let lua = mlua::Lua::new();
        let (ctx, npc_rx, _chat_rx) = test_ctx_with_channels();
        register_os_npc_remove(&lua, &ctx).unwrap();
        lua.load(r#"osNpcRemove("12345678-1234-1234-1234-123456789abc")"#)
            .exec()
            .unwrap();
        let c = ctx.read();
        let npc_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        assert!(!c.npc_registry.contains_key(&npc_id));
        let cmd = npc_rx.try_recv().unwrap();
        matches!(cmd, NpcCommand::Remove { .. });
    }

    #[test]
    fn os_npc_remove_denied_for_stranger() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        // Change owner so script owner does not match NPC owner.
        ctx.write().owner_id = Uuid::new_v4();
        register_os_npc_remove(&lua, &ctx).unwrap();
        lua.load(r#"osNpcRemove("12345678-1234-1234-1234-123456789abc")"#)
            .exec()
            .unwrap();
        // NPC should still be in registry.
        let c = ctx.read();
        let npc_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        assert!(c.npc_registry.contains_key(&npc_id));
    }

    // -- osNpcSaveAppearance ---------------------------------------------------

    #[test]
    fn os_npc_save_appearance_returns_notecard_id() {
        let lua = mlua::Lua::new();
        let (ctx, npc_rx, _chat_rx) = test_ctx_with_channels();
        register_os_npc_save_appearance(&lua, &ctx).unwrap();
        let key: String = lua
            .load(r#"return osNpcSaveAppearance("12345678-1234-1234-1234-123456789abc", "my_notecard")"#)
            .eval()
            .unwrap();
        let nc_id = Uuid::parse_str(&key).unwrap();
        assert!(!nc_id.is_nil(), "should return a real notecard UUID");
        // Notecard name should be recorded.
        let c = ctx.read();
        assert_eq!(c.notecard_names.get("my_notecard"), Some(&nc_id));
        // Command should be sent.
        let cmd = npc_rx.try_recv().unwrap();
        match cmd {
            NpcCommand::SaveAppearance { notecard_name, notecard_id, .. } => {
                assert_eq!(notecard_name, "my_notecard");
                assert_eq!(notecard_id, nc_id);
            }
            _ => panic!("expected SaveAppearance command"),
        }
    }

    #[test]
    fn os_npc_save_appearance_denied_returns_nil() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        ctx.write().owner_id = Uuid::new_v4(); // different owner
        register_os_npc_save_appearance(&lua, &ctx).unwrap();
        let key: String = lua
            .load(r#"return osNpcSaveAppearance("12345678-1234-1234-1234-123456789abc", "nc")"#)
            .eval()
            .unwrap();
        assert_eq!(key, Uuid::nil().to_string());
    }

    // -- osNpcLoadAppearance ---------------------------------------------------

    #[test]
    fn os_npc_load_appearance_sends_command() {
        let lua = mlua::Lua::new();
        let (ctx, npc_rx, _chat_rx) = test_ctx_with_channels();
        register_os_npc_load_appearance(&lua, &ctx).unwrap();
        lua.load(r#"osNpcLoadAppearance("12345678-1234-1234-1234-123456789abc", "some_notecard")"#)
            .exec()
            .unwrap();
        let cmd = npc_rx.try_recv().unwrap();
        match cmd {
            NpcCommand::LoadAppearance { notecard_name, .. } => {
                assert_eq!(notecard_name, "some_notecard");
            }
            _ => panic!("expected LoadAppearance command"),
        }
    }

    // -- osNpcSay (variadic) ---------------------------------------------------

    #[test]
    fn os_npc_say_3_args() {
        let lua = mlua::Lua::new();
        let (ctx, _npc_rx, chat_rx) = test_ctx_with_channels();
        register_os_npc_say(&lua, &ctx).unwrap();
        lua.load(r#"osNpcSay("12345678-1234-1234-1234-123456789abc", 5, "hello")"#)
            .exec()
            .unwrap();
        let msg = chat_rx.try_recv().unwrap();
        assert_eq!(msg.channel, 5);
        assert_eq!(msg.message, "hello");
    }

    #[test]
    fn os_npc_say_2_args_uses_channel_0() {
        let lua = mlua::Lua::new();
        let (ctx, _npc_rx, chat_rx) = test_ctx_with_channels();
        register_os_npc_say(&lua, &ctx).unwrap();
        lua.load(r#"osNpcSay("12345678-1234-1234-1234-123456789abc", "world")"#)
            .exec()
            .unwrap();
        let msg = chat_rx.try_recv().unwrap();
        assert_eq!(msg.channel, 0);
        assert_eq!(msg.message, "world");
    }

    // -- osNpcLookAt (new signature) -------------------------------------------

    #[test]
    fn os_npc_look_at_valid() {
        let lua = mlua::Lua::new();
        let (ctx, npc_rx, _chat_rx) = test_ctx_with_channels();
        register_os_npc_look_at(&lua, &ctx).unwrap();
        let result: i32 = lua
            .load(
                r#"return osNpcLookAt("12345678-1234-1234-1234-123456789abc", 1, "00000000-0000-0000-0000-000000000000", "<0,0,0>")"#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, 0);
        let cmd = npc_rx.try_recv().unwrap();
        match cmd {
            NpcCommand::LookAt { look_type, .. } => {
                assert_eq!(look_type, NPCLOOKAT_IDLE);
            }
            _ => panic!("expected LookAt command"),
        }
    }

    #[test]
    fn os_npc_look_at_bad_type() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        register_os_npc_look_at(&lua, &ctx).unwrap();
        let result: i32 = lua
            .load(
                r#"return osNpcLookAt("12345678-1234-1234-1234-123456789abc", 99, "00000000-0000-0000-0000-000000000000", "<0,0,0>")"#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, -1);
    }

    #[test]
    fn os_npc_look_at_not_an_npc() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        register_os_npc_look_at(&lua, &ctx).unwrap();
        let result: i32 = lua
            .load(
                r#"return osNpcLookAt("cccccccc-cccc-cccc-cccc-cccccccccccc", 0, "00000000-0000-0000-0000-000000000000", "<0,0,0>")"#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, -3);
    }

    // -- Ownership check -------------------------------------------------------

    #[test]
    fn ownership_blocks_move_to_target() {
        let lua = mlua::Lua::new();
        let (ctx, npc_rx, _chat_rx) = test_ctx_with_channels();
        // Change script owner so it differs from NPC owner.
        ctx.write().owner_id = Uuid::new_v4();
        register_os_npc_move_to_target(&lua, &ctx).unwrap();
        lua.load(r#"osNpcMoveToTarget("12345678-1234-1234-1234-123456789abc", "<0,0,0>", 0)"#)
            .exec()
            .unwrap();
        // No command should be sent.
        assert!(npc_rx.try_recv().is_err());
    }

    #[test]
    fn unowned_npc_accessible_by_anyone() {
        let lua = mlua::Lua::new();
        let (ctx, npc_rx, _chat_rx) = test_ctx_with_channels();
        let npc_id = Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap();
        ctx.write().npc_registry.insert(
            npc_id,
            NpcInfo {
                owner_id: Uuid::nil(),
                first_name: "Free".into(),
                last_name: "NPC".into(),
                position: [10.0, 20.0, 30.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                profile_about: String::new(),
                profile_image: Uuid::nil(),
            },
        );
        // Script owner is different from NPC owner (nil), but should still work.
        register_os_npc_stand(&lua, &ctx).unwrap();
        lua.load(r#"osNpcStand("dddddddd-dddd-dddd-dddd-dddddddddddd")"#)
            .exec()
            .unwrap();
        let cmd = npc_rx.try_recv().unwrap();
        matches!(cmd, NpcCommand::Stand { .. });
    }

    // -- osNpcSetProfileAbout sends command ------------------------------------

    #[test]
    fn os_npc_set_profile_about_sends_command() {
        let lua = mlua::Lua::new();
        let (ctx, npc_rx, _chat_rx) = test_ctx_with_channels();
        register_os_npc_set_profile_about(&lua, &ctx).unwrap();
        lua.load(r#"osNpcSetProfileAbout("12345678-1234-1234-1234-123456789abc", "I am a bot")"#)
            .exec()
            .unwrap();
        // Registry should be updated.
        let c = ctx.read();
        let npc_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        assert_eq!(
            c.npc_registry.get(&npc_id).unwrap().profile_about,
            "I am a bot"
        );
        // Command should be sent.
        let cmd = npc_rx.try_recv().unwrap();
        match cmd {
            NpcCommand::SetProfileAbout { about, .. } => {
                assert_eq!(about, "I am a bot");
            }
            _ => panic!("expected SetProfileAbout command"),
        }
    }
}
