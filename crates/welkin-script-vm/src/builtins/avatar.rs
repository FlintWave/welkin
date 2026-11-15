// SPDX-License-Identifier: GPL-3.0-or-later
//! Avatar ll* functions (permissions, animations, sit targets, etc.).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;

use super::{ScriptContext, AttachAction, AttachRequest};

pub const DATA_ONLINE: i32 = 1;
pub const DATA_NAME: i32 = 2;
pub const DATA_BORN: i32 = 3;
pub const DATA_RATING: i32 = 4;
pub const DATA_SIM_POS: i32 = 5;
pub const DATA_SIM_STATUS: i32 = 6;
pub const DATA_SIM_RATING: i32 = 7;
pub const DATA_PAYINFO: i32 = 8;

pub fn register_avatar(lua: &Lua, ctx: &Arc<RwLock<ScriptContext>>) -> Result<(), crate::VmError> {
    // PERMISSION_* constants
    lua.globals().set("PERMISSION_DEBIT", 0x2i64)?;
    lua.globals().set("PERMISSION_TRIGGER_ANIMATION", 0x10i64)?;
    lua.globals().set("PERMISSION_ATTACH", 0x20i64)?;
    lua.globals().set("PERMISSION_CHANGE_LINKS", 0x80i64)?;
    lua.globals().set("PERMISSION_TRACK_CAMERA", 0x400i64)?;
    lua.globals().set("PERMISSION_CONTROL_CAMERA", 0x800i64)?;
    lua.globals().set("PERMISSION_TELEPORT", 0x1000i64)?;
    lua.globals().set("PERMISSION_TAKE_CONTROLS", 0x4i64)?;

    // DATA_* constants
    lua.globals().set("DATA_ONLINE", DATA_ONLINE as i64)?;
    lua.globals().set("DATA_NAME", DATA_NAME as i64)?;
    lua.globals().set("DATA_BORN", DATA_BORN as i64)?;
    lua.globals().set("DATA_RATING", DATA_RATING as i64)?;
    lua.globals().set("DATA_SIM_POS", DATA_SIM_POS as i64)?;
    lua.globals().set("DATA_SIM_STATUS", DATA_SIM_STATUS as i64)?;
    lua.globals().set("DATA_SIM_RATING", DATA_SIM_RATING as i64)?;
    lua.globals().set("DATA_PAYINFO", DATA_PAYINFO as i64)?;

    // llRequestPermissions(agent_id, permissions)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (_agent_id, perms): (String, i64)| {
            let mut ctx = ctx.write();
            ctx.permissions_granted = perms as u32;
            tracing::debug!(perms, "llRequestPermissions: auto-granting");
            Ok(())
        })?;
        lua.globals().set("llRequestPermissions", f)?;
    }

    // llGetPermissions() -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.permissions_granted as i64)
        })?;
        lua.globals().set("llGetPermissions", f)?;
    }

    // llGetPermissionsKey() -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            if ctx.permissions_granted != 0 {
                Ok(ctx.owner_id.to_string())
            } else {
                Ok(uuid::Uuid::nil().to_string())
            }
        })?;
        lua.globals().set("llGetPermissionsKey", f)?;
    }

    // llStartAnimation(anim_name)
    {
        let f = lua.create_function(|_, anim: String| {
            tracing::debug!(%anim, "llStartAnimation");
            Ok(())
        })?;
        lua.globals().set("llStartAnimation", f)?;
    }

    // llStopAnimation(anim_name)
    {
        let f = lua.create_function(|_, anim: String| {
            tracing::debug!(%anim, "llStopAnimation");
            Ok(())
        })?;
        lua.globals().set("llStopAnimation", f)?;
    }

    // llSitTarget(offset, rot)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (offset, rot): (LuaTable, LuaTable)| {
            let pos = [
                offset.get::<f64>("x").or_else(|_| offset.get(1))?,
                offset.get::<f64>("y").or_else(|_| offset.get(2))?,
                offset.get::<f64>("z").or_else(|_| offset.get(3))?,
            ];
            let r = [
                rot.get::<f64>("x").or_else(|_| rot.get(1))?,
                rot.get::<f64>("y").or_else(|_| rot.get(2))?,
                rot.get::<f64>("z").or_else(|_| rot.get(3))?,
                rot.get::<f64>("w").or_else(|_| rot.get(4))?,
            ];
            let mut ctx = ctx.write();
            ctx.sit_target_pos = pos;
            ctx.sit_target_rot = r;
            Ok(())
        })?;
        lua.globals().set("llSitTarget", f)?;
    }

    // llAvatarOnSitTarget() -> string (UUID of sitting avatar, or NULL_KEY)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            Ok(ctx.sitting_avatar.unwrap_or(uuid::Uuid::nil()).to_string())
        })?;
        lua.globals().set("llAvatarOnSitTarget", f)?;
    }

    // llUnSit(agent_id)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, _agent_id: String| {
            let mut ctx = ctx.write();
            ctx.sitting_avatar = None;
            Ok(())
        })?;
        lua.globals().set("llUnSit", f)?;
    }

    // llGetAgentSize(agent_id) -> vector
    {
        let f = lua.create_function(|lua, _agent_id: String| {
            let t = lua.create_table()?;
            t.set("x", 0.45)?;
            t.set("y", 0.6)?;
            t.set("z", 1.8)?;
            Ok(t)
        })?;
        lua.globals().set("llGetAgentSize", f)?;
    }

    // llGetAgentInfo(agent_id) -> integer
    {
        let f = lua.create_function(|_, _agent_id: String| Ok(0i64))?;
        lua.globals().set("llGetAgentInfo", f)?;
    }

    // llTeleportAgent(agent_id, landmark, pos, look_at)
    {
        let f = lua.create_function(
            |_, (_agent_id, _landmark, _pos, _look_at): (String, String, LuaTable, LuaTable)| {
                tracing::debug!("llTeleportAgent");
                Ok(())
            },
        )?;
        lua.globals().set("llTeleportAgent", f)?;
    }

    // llTeleportAgentHome(agent_id)
    {
        let f = lua.create_function(|_, _agent_id: String| {
            tracing::debug!("llTeleportAgentHome");
            Ok(())
        })?;
        lua.globals().set("llTeleportAgentHome", f)?;
    }

    // ATTACH_* constants
    lua.globals().set("ATTACH_CHEST", 1i64)?;
    lua.globals().set("ATTACH_HEAD", 2i64)?;
    lua.globals().set("ATTACH_LSHOULDER", 3i64)?;
    lua.globals().set("ATTACH_RSHOULDER", 4i64)?;
    lua.globals().set("ATTACH_LHAND", 5i64)?;
    lua.globals().set("ATTACH_RHAND", 6i64)?;
    lua.globals().set("ATTACH_LFOOT", 7i64)?;
    lua.globals().set("ATTACH_RFOOT", 8i64)?;
    lua.globals().set("ATTACH_BACK", 9i64)?;
    lua.globals().set("ATTACH_PELVIS", 10i64)?;
    lua.globals().set("ATTACH_MOUTH", 11i64)?;
    lua.globals().set("ATTACH_CHIN", 12i64)?;
    lua.globals().set("ATTACH_LEAR", 13i64)?;
    lua.globals().set("ATTACH_REAR", 14i64)?;
    lua.globals().set("ATTACH_LEYE", 15i64)?;
    lua.globals().set("ATTACH_REYE", 16i64)?;
    lua.globals().set("ATTACH_NOSE", 17i64)?;
    lua.globals().set("ATTACH_RUARM", 18i64)?;
    lua.globals().set("ATTACH_RLARM", 19i64)?;
    lua.globals().set("ATTACH_LUARM", 20i64)?;
    lua.globals().set("ATTACH_LLARM", 21i64)?;
    lua.globals().set("ATTACH_RHIP", 22i64)?;
    lua.globals().set("ATTACH_RULEG", 23i64)?;
    lua.globals().set("ATTACH_RLLEG", 24i64)?;
    lua.globals().set("ATTACH_LHIP", 25i64)?;
    lua.globals().set("ATTACH_LULEG", 26i64)?;
    lua.globals().set("ATTACH_LLLEG", 27i64)?;
    lua.globals().set("ATTACH_BELLY", 28i64)?;
    lua.globals().set("ATTACH_RPEC", 29i64)?;
    lua.globals().set("ATTACH_LPEC", 30i64)?;
    lua.globals().set("ATTACH_HUD_CENTER_2", 31i64)?;
    lua.globals().set("ATTACH_HUD_TOP_RIGHT", 32i64)?;
    lua.globals().set("ATTACH_HUD_TOP_CENTER", 33i64)?;
    lua.globals().set("ATTACH_HUD_TOP_LEFT", 34i64)?;
    lua.globals().set("ATTACH_HUD_CENTER_1", 35i64)?;
    lua.globals().set("ATTACH_HUD_BOTTOM_LEFT", 36i64)?;
    lua.globals().set("ATTACH_HUD_BOTTOM", 37i64)?;
    lua.globals().set("ATTACH_HUD_BOTTOM_RIGHT", 38i64)?;

    // llAttachToAvatar(attach_point)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, attach_point: i64| {
            let ctx = ctx.read();
            if ctx.permissions_granted & 0x20 == 0 {
                return Err(mlua::Error::runtime("llAttachToAvatar: PERMISSION_ATTACH not granted"));
            }
            if let Some(ref tx) = ctx.attach_sender {
                let req = AttachRequest {
                    script_id: ctx.script_id,
                    prim_id: ctx.prim_id,
                    owner_id: ctx.owner_id,
                    attach_point: attach_point as i32,
                    is_temp: false,
                };
                let _ = tx.send(AttachAction::Attach(req));
            }
            Ok(())
        })?;
        lua.globals().set("llAttachToAvatar", f)?;
    }

    // llAttachToAvatarTemp(attach_point)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, attach_point: i64| {
            let ctx = ctx.read();
            if ctx.permissions_granted & 0x20 == 0 {
                return Err(mlua::Error::runtime("llAttachToAvatarTemp: PERMISSION_ATTACH not granted"));
            }
            if let Some(ref tx) = ctx.attach_sender {
                let req = AttachRequest {
                    script_id: ctx.script_id,
                    prim_id: ctx.prim_id,
                    owner_id: ctx.owner_id,
                    attach_point: attach_point as i32,
                    is_temp: true,
                };
                let _ = tx.send(AttachAction::Attach(req));
            }
            Ok(())
        })?;
        lua.globals().set("llAttachToAvatarTemp", f)?;
    }

    // llDetachFromAvatar()
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, ()| {
            let ctx = ctx.read();
            if ctx.permissions_granted & 0x20 == 0 {
                return Err(mlua::Error::runtime("llDetachFromAvatar: PERMISSION_ATTACH not granted"));
            }
            if let Some(ref tx) = ctx.attach_sender {
                let _ = tx.send(AttachAction::Detach { prim_id: ctx.prim_id });
            }
            Ok(())
        })?;
        lua.globals().set("llDetachFromAvatar", f)?;
    }

    // llRequestAgentData(agent_id, data_type) -> key (request id)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (agent_id, data_type): (String, i64)| {
            let ctx = ctx.read();
            let request_id = uuid::Uuid::new_v4().to_string();
            let data = match data_type as i32 {
                DATA_ONLINE => "1".to_string(),
                DATA_NAME => "Unknown User".to_string(),
                DATA_BORN => "2006-01-01".to_string(),
                DATA_RATING => "0,0,0,0,0".to_string(),
                DATA_SIM_POS => format!("<{}, {}, {}>", ctx.position[0], ctx.position[1], ctx.position[2]),
                DATA_SIM_STATUS => "up".to_string(),
                DATA_SIM_RATING => "PG".to_string(),
                DATA_PAYINFO => "0".to_string(),
                _ => String::new(),
            };
            if let Some(ref tx) = ctx.event_sender {
                let event = crate::ScriptEvent::Dataserver {
                    request_id: request_id.clone(),
                    data,
                };
                let _ = tx.send((ctx.prim_id, event));
            }
            tracing::debug!(%agent_id, data_type, %request_id, "llRequestAgentData");
            Ok(request_id)
        })?;
        lua.globals().set("llRequestAgentData", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            owner_id: Uuid::from_u128(0xabcd),
            ..Default::default()
        }))
    }

    #[test]
    fn request_permissions_auto_grants() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        lua.load(r#"llRequestPermissions("00000000-0000-0000-0000-000000000000", PERMISSION_TRIGGER_ANIMATION)"#)
            .exec()
            .unwrap();
        assert_eq!(ctx.read().permissions_granted, 0x10);
    }

    #[test]
    fn get_permissions_returns_granted() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        lua.load(r#"llRequestPermissions("00000000-0000-0000-0000-000000000000", PERMISSION_DEBIT)"#)
            .exec()
            .unwrap();
        let perms: i64 = lua.load("return llGetPermissions()").eval().unwrap();
        assert_eq!(perms, 0x2);
    }

    #[test]
    fn sit_target_updates_context() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        lua.load("llSitTarget({x=0, y=0, z=0.5}, {x=0, y=0, z=0, w=1})")
            .exec()
            .unwrap();
        let pos = ctx.read().sit_target_pos;
        assert!((pos[2] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn avatar_on_sit_target_nil_when_nobody() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        let key: String = lua.load("return llAvatarOnSitTarget()").eval().unwrap();
        assert_eq!(key, Uuid::nil().to_string());
    }

    #[test]
    fn unsit_clears_sitting_avatar() {
        let lua = Lua::new();
        let ctx = test_ctx();
        ctx.write().sitting_avatar = Some(Uuid::from_u128(0x1234));
        register_avatar(&lua, &ctx).unwrap();
        lua.load(r#"llUnSit("00000000-0000-0000-0000-000000001234")"#)
            .exec()
            .unwrap();
        assert!(ctx.read().sitting_avatar.is_none());
    }

    #[test]
    fn get_agent_size_returns_defaults() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_avatar(&lua, &ctx).unwrap();
        let z: f64 = lua
            .load(r#"return llGetAgentSize("00000000-0000-0000-0000-000000000000").z"#)
            .eval()
            .unwrap();
        assert!((z - 1.8).abs() < f64::EPSILON);
    }

    fn ctx_with_events() -> (
        Arc<RwLock<ScriptContext>>,
        std::sync::mpsc::Receiver<(Uuid, crate::ScriptEvent)>,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            owner_id: Uuid::from_u128(0xabcd),
            event_sender: Some(tx),
            ..Default::default()
        }));
        (ctx, rx)
    }

    #[test]
    fn request_agent_data_online() {
        let (ctx, rx) = ctx_with_events();
        let lua = Lua::new();
        register_avatar(&lua, &ctx).unwrap();

        let key: String = lua
            .load(r#"return llRequestAgentData("00000000-0000-0000-0000-000000001234", DATA_ONLINE)"#)
            .eval()
            .unwrap();
        assert!(!key.is_empty());

        let (_prim_id, event) = rx.try_recv().unwrap();
        match event {
            crate::ScriptEvent::Dataserver { request_id, data } => {
                assert_eq!(request_id, key);
                assert_eq!(data, "1");
            }
            _ => panic!("expected Dataserver event"),
        }
    }

    fn ctx_with_attach() -> (
        Arc<RwLock<ScriptContext>>,
        std::sync::mpsc::Receiver<super::super::AttachAction>,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            owner_id: Uuid::from_u128(0xabcd),
            permissions_granted: 0x20, // PERMISSION_ATTACH
            attach_sender: Some(tx),
            ..Default::default()
        }));
        (ctx, rx)
    }

    #[test]
    fn attach_to_avatar_sends_request() {
        let (ctx, rx) = ctx_with_attach();
        let lua = Lua::new();
        register_avatar(&lua, &ctx).unwrap();
        lua.load("llAttachToAvatar(ATTACH_CHEST)").exec().unwrap();
        let action = rx.try_recv().unwrap();
        match action {
            super::super::AttachAction::Attach(req) => {
                assert_eq!(req.attach_point, 1);
                assert!(!req.is_temp);
            }
            _ => panic!("expected Attach"),
        }
    }

    #[test]
    fn attach_denied_without_permission() {
        let ctx = Arc::new(RwLock::new(ScriptContext {
            permissions_granted: 0,
            ..Default::default()
        }));
        let lua = Lua::new();
        register_avatar(&lua, &ctx).unwrap();
        let result = lua.load("llAttachToAvatar(1)").exec();
        assert!(result.is_err());
    }

    #[test]
    fn detach_sends_action() {
        let (ctx, rx) = ctx_with_attach();
        let lua = Lua::new();
        register_avatar(&lua, &ctx).unwrap();
        lua.load("llDetachFromAvatar()").exec().unwrap();
        let action = rx.try_recv().unwrap();
        match action {
            super::super::AttachAction::Detach { .. } => {}
            _ => panic!("expected Detach"),
        }
    }

    #[test]
    fn attach_temp_sets_is_temp() {
        let (ctx, rx) = ctx_with_attach();
        let lua = Lua::new();
        register_avatar(&lua, &ctx).unwrap();
        lua.load("llAttachToAvatarTemp(ATTACH_HEAD)").exec().unwrap();
        let action = rx.try_recv().unwrap();
        match action {
            super::super::AttachAction::Attach(req) => {
                assert_eq!(req.attach_point, 2);
                assert!(req.is_temp);
            }
            _ => panic!("expected Attach"),
        }
    }

    #[test]
    fn request_agent_data_name() {
        let (ctx, rx) = ctx_with_events();
        let lua = Lua::new();
        register_avatar(&lua, &ctx).unwrap();

        lua.load(r#"llRequestAgentData("00000000-0000-0000-0000-000000001234", DATA_NAME)"#)
            .exec()
            .unwrap();

        let (_prim_id, event) = rx.try_recv().unwrap();
        match event {
            crate::ScriptEvent::Dataserver { data, .. } => {
                assert_eq!(data, "Unknown User");
            }
            _ => panic!("expected Dataserver event"),
        }
    }
}
