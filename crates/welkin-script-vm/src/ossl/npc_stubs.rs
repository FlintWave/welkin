// SPDX-License-Identifier: GPL-3.0-or-later
//! Real NPC OSSL functions — interact with NPC registry via ScriptContext.

use super::{OsslFunc, OsslThreatLevel};
use crate::builtins::{NpcCommand, ScriptContext};
use crate::ChatMessage;
use mlua::prelude::*;
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn ossl_npc_funcs() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osIsNpc",
            level: OsslThreatLevel::High,
            register: register_os_is_npc,
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
            level: OsslThreatLevel::High,
            register: register_os_npc_set_profile_about,
        },
        OsslFunc {
            name: "osNpcSetProfileImage",
            level: OsslThreatLevel::High,
            register: register_os_npc_set_profile_image,
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

fn format_vector(v: [f64; 3]) -> String {
    format!("<{}, {}, {}>", v[0], v[1], v[2])
}

fn format_rotation(r: [f64; 4]) -> String {
    format!("<{}, {}, {}, {}>", r[0], r[1], r[2], r[3])
}

fn send_npc_cmd(ctx: &ScriptContext, cmd: NpcCommand) {
    if let Some(ref sender) = ctx.npc_sender {
        let _ = sender.send(cmd);
    }
}

fn send_npc_chat(ctx: &ScriptContext, npc_id: Uuid, channel: i32, message: &str, chat_type: crate::ChatType) {
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

// -- osIsNpc(key) -> 1 if NPC, 0 otherwise --
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

// -- osNpcSaveAppearance(npc, notecard) -> notecard key --
fn register_os_npc_save_appearance(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, notecard): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !c.npc_registry.contains_key(&npc_id) {
            return Ok(Uuid::nil().to_string());
        }
        let nc_id = Uuid::new_v4();
        tracing::info!(npc = %npc_id, notecard = %notecard, nc_id = %nc_id, "osNpcSaveAppearance");
        Ok(nc_id.to_string())
    })?;
    lua.globals().set("osNpcSaveAppearance", f)
}

// -- osNpcLoadAppearance(npc, notecard) -> void --
fn register_os_npc_load_appearance(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, notecard): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        if !c.npc_registry.contains_key(&npc_id) {
            return Ok(());
        }
        tracing::info!(npc = %npc_id, notecard = %notecard, "osNpcLoadAppearance");
        Ok(())
    })?;
    lua.globals().set("osNpcLoadAppearance", f)
}

// -- osNpcGetOwner(npc) -> owner key --
fn register_os_npc_get_owner(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        let owner = c.npc_registry.get(&npc_id)
            .map(|n| n.owner_id)
            .unwrap_or(Uuid::nil());
        Ok(owner.to_string())
    })?;
    lua.globals().set("osNpcGetOwner", f)
}

// -- osNpcGetPos(npc) -> vector string --
fn register_os_npc_get_pos(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        let pos = c.npc_registry.get(&npc_id)
            .map(|n| n.position)
            .unwrap_or([0.0; 3]);
        Ok(format_vector(pos))
    })?;
    lua.globals().set("osNpcGetPos", f)
}

// -- osNpcMoveToTarget(npc, target, options) -> void --
fn register_os_npc_move_to_target(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, target, options): (String, String, i32)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        let target_pos = parse_vector(&target);
        send_npc_cmd(&c, NpcCommand::MoveTo { npc_id, target: target_pos, options });
        Ok(())
    })?;
    lua.globals().set("osNpcMoveToTarget", f)
}

// -- osNpcGetRot(npc) -> rotation string --
fn register_os_npc_get_rot(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        let rot = c.npc_registry.get(&npc_id)
            .map(|n| n.rotation)
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        Ok(format_rotation(rot))
    })?;
    lua.globals().set("osNpcGetRot", f)
}

// -- osNpcSetRot(npc, rot) -> void --
fn register_os_npc_set_rot(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, rot): (String, String)| {
        let mut c = ctx.write();
        let npc_id = parse_uuid(&npc);
        let rot_str = rot.trim().trim_start_matches('<').trim_end_matches('>');
        let parts: Vec<&str> = rot_str.split(',').collect();
        let rotation = if parts.len() >= 4 {
            [
                parts[0].trim().parse::<f64>().unwrap_or(0.0),
                parts[1].trim().parse::<f64>().unwrap_or(0.0),
                parts[2].trim().parse::<f64>().unwrap_or(0.0),
                parts[3].trim().parse::<f64>().unwrap_or(1.0),
            ]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        if let Some(npc_info) = c.npc_registry.get_mut(&npc_id) {
            npc_info.rotation = rotation;
        }
        send_npc_cmd(&c, NpcCommand::SetRot { npc_id, rotation });
        Ok(())
    })?;
    lua.globals().set("osNpcSetRot", f)
}

// -- osNpcStopMoveToTarget(npc) -> void --
fn register_os_npc_stop_move_to_target(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        send_npc_cmd(&c, NpcCommand::StopMove { npc_id });
        Ok(())
    })?;
    lua.globals().set("osNpcStopMoveToTarget", f)
}

// -- osNpcSetProfileAbout(npc, text) -> void --
fn register_os_npc_set_profile_about(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, text): (String, String)| {
        let mut c = ctx.write();
        let npc_id = parse_uuid(&npc);
        if let Some(npc_info) = c.npc_registry.get_mut(&npc_id) {
            npc_info.profile_about = text;
        }
        Ok(())
    })?;
    lua.globals().set("osNpcSetProfileAbout", f)
}

// -- osNpcSetProfileImage(npc, texture) -> void --
fn register_os_npc_set_profile_image(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, texture): (String, String)| {
        let mut c = ctx.write();
        let npc_id = parse_uuid(&npc);
        let tex_id = parse_uuid(&texture);
        if let Some(npc_info) = c.npc_registry.get_mut(&npc_id) {
            npc_info.profile_image = tex_id;
        }
        Ok(())
    })?;
    lua.globals().set("osNpcSetProfileImage", f)
}

// -- osNpcSayTo(npc, target, channel, msg) -> void --
fn register_os_npc_say_to(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(
        move |_, (npc, target, channel, msg): (String, String, i32, String)| {
            let c = ctx.read();
            let npc_id = parse_uuid(&npc);
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

// -- osNpcShout(npc, channel, msg) -> void --
fn register_os_npc_shout(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, channel, msg): (String, i32, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        send_npc_chat(&c, npc_id, channel, &msg, crate::ChatType::Shout);
        Ok(())
    })?;
    lua.globals().set("osNpcShout", f)
}

// -- osNpcSit(npc, target, options) -> void --
fn register_os_npc_sit(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, target, options): (String, String, i32)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        let target_id = parse_uuid(&target);
        send_npc_cmd(&c, NpcCommand::Sit { npc_id, target: target_id, options });
        Ok(())
    })?;
    lua.globals().set("osNpcSit", f)
}

// -- osNpcStand(npc) -> void --
fn register_os_npc_stand(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, npc: String| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        send_npc_cmd(&c, NpcCommand::Stand { npc_id });
        Ok(())
    })?;
    lua.globals().set("osNpcStand", f)
}

// -- osNpcPlayAnimation(npc, anim) -> void --
fn register_os_npc_play_animation(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, anim): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        send_npc_cmd(&c, NpcCommand::PlayAnimation { npc_id, animation: anim });
        Ok(())
    })?;
    lua.globals().set("osNpcPlayAnimation", f)
}

// -- osNpcStopAnimation(npc, anim) -> void --
fn register_os_npc_stop_animation(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, anim): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        send_npc_cmd(&c, NpcCommand::StopAnimation { npc_id, animation: anim });
        Ok(())
    })?;
    lua.globals().set("osNpcStopAnimation", f)
}

// -- osNpcTouch(npc, object, link) -> void --
fn register_os_npc_touch(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, object, link): (String, String, i32)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        let target = parse_uuid(&object);
        send_npc_cmd(&c, NpcCommand::Touch { npc_id, target, link_num: link });
        Ok(())
    })?;
    lua.globals().set("osNpcTouch", f)
}

// -- osNpcWhisper(npc, channel, msg) -> void --
fn register_os_npc_whisper(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, channel, msg): (String, i32, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        send_npc_chat(&c, npc_id, channel, &msg, crate::ChatType::Whisper);
        Ok(())
    })?;
    lua.globals().set("osNpcWhisper", f)
}

// -- osNpcLookAt(npc, target) -> void --
fn register_os_npc_look_at(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let ctx = ctx.clone();
    let f = lua.create_function(move |_, (npc, target): (String, String)| {
        let c = ctx.read();
        let npc_id = parse_uuid(&npc);
        let target_pos = parse_vector(&target);
        send_npc_cmd(&c, NpcCommand::LookAt { npc_id, target: target_pos });
        Ok(())
    })?;
    lua.globals().set("osNpcLookAt", f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::NpcInfo;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        let mut ctx = ScriptContext::default();
        ctx.script_id = Uuid::new_v4();
        ctx.owner_id = Uuid::new_v4();
        let npc_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        ctx.npc_registry.insert(npc_id, NpcInfo {
            owner_id: ctx.owner_id,
            first_name: "Test".into(),
            last_name: "NPC".into(),
            position: [128.0, 128.0, 25.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            profile_about: String::new(),
            profile_image: Uuid::nil(),
        });
        Arc::new(RwLock::new(ctx))
    }

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

    #[test]
    fn os_npc_set_rot() {
        let lua = mlua::Lua::new();
        let ctx = test_ctx();
        register_os_npc_set_rot(&lua, &ctx).unwrap();
        lua.load(r#"osNpcSetRot("12345678-1234-1234-1234-123456789abc", "<0, 0, 0.707, 0.707>")"#)
            .exec()
            .unwrap();
        let c = ctx.read();
        let npc_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let rot = c.npc_registry.get(&npc_id).unwrap().rotation;
        assert!((rot[2] - 0.707).abs() < 0.001);
        assert!((rot[3] - 0.707).abs() < 0.001);
    }
}
