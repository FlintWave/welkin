// SPDX-License-Identifier: LGPL-3.0-or-later

use super::ScriptContext;
use std::sync::Arc;
use parking_lot::RwLock;

fn notify_unsupported(ctx: &ScriptContext, name: &str) {
    if let Some(sender) = &ctx.chat_sender {
        let msg = crate::ChatMessage {
            source_id: ctx.prim_id,
            source_name: ctx.prim_name.clone(),
            source_pos: ctx.position,
            owner_id: ctx.owner_id,
            channel: 0,
            message: format!("{name} is not supported on this region"),
            chat_type: crate::ChatType::Owner,
            target_id: Some(ctx.owner_id),
        };
        let _ = sender.send(msg);
    }
}

fn table_vec(t: Option<&mlua::Table>) -> [f64; 3] {
    if let Some(t) = t {
        [
            t.get::<f64>("x").unwrap_or(0.0),
            t.get::<f64>("y").unwrap_or(0.0),
            t.get::<f64>("z").unwrap_or(0.0),
        ]
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn table_rot(t: Option<&mlua::Table>) -> [f64; 4] {
    if let Some(t) = t {
        [
            t.get::<f64>("x").unwrap_or(0.0),
            t.get::<f64>("y").unwrap_or(0.0),
            t.get::<f64>("z").unwrap_or(0.0),
            t.get::<f64>("w").unwrap_or(1.0),
        ]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

fn rotate_vector(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let (qx, qy, qz, qw) = (q[0], q[1], q[2], q[3]);
    let tx = 2.0 * (qy * v[2] - qz * v[1]);
    let ty = 2.0 * (qz * v[0] - qx * v[2]);
    let tz = 2.0 * (qx * v[1] - qy * v[0]);
    [
        v[0] + qw * tx + (qy * tz - qz * ty),
        v[1] + qw * ty + (qz * tx - qx * tz),
        v[2] + qw * tz + (qx * ty - qy * tx),
    ]
}

pub fn register_extra_g2(
    lua: &mlua::Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    // llSetText
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetText",
            lua.create_function(move |_lua, (text, color, _alpha): (Option<String>, Option<mlua::Table>, Option<f64>)| {
                let text = text.unwrap_or_default();
                let v = table_vec(color.as_ref());
                let color = [clamp01(v[0]), clamp01(v[1]), clamp01(v[2])];
                let mut c = ctx.write();
                c.hover_text = text;
                c.hover_text_color = color;
                Ok(())
            })?,
        )?;
    }

    // llSetClickAction
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetClickAction",
            lua.create_function(move |_lua, action: i64| {
                let mut c = ctx.write();
                c.click_action = action as i32;
                Ok(())
            })?,
        )?;
    }

    // llSetSitText
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetSitText",
            lua.create_function(move |_lua, text: Option<String>| {
                let mut c = ctx.write();
                c.sit_text = text.unwrap_or_default();
                Ok(())
            })?,
        )?;
    }

    // llSetLocalRot
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetLocalRot",
            lua.create_function(move |_lua, rot: Option<mlua::Table>| {
                let r = table_rot(rot.as_ref());
                let mut c = ctx.write();
                c.rotation = r;
                Ok(())
            })?,
        )?;
    }

    // llSetLinkColor
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetLinkColor",
            lua.create_function(move |_lua, (link_number, color, _face): (i64, Option<mlua::Table>, i64)| {
                let v = table_vec(color.as_ref());
                let color = [clamp01(v[0]), clamp01(v[1]), clamp01(v[2])];
                let mut c = ctx.write();
                c.link_colors.insert(link_number as i32, color);
                Ok(())
            })?,
        )?;
    }

    // llSetRemoteScriptAccessPin
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetRemoteScriptAccessPin",
            lua.create_function(move |_lua, pin: i64| {
                let mut c = ctx.write();
                c.remote_script_pin = pin as i32;
                Ok(())
            })?,
        )?;
    }

    // llCollisionFilter
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llCollisionFilter",
            lua.create_function(move |_lua, (name, id, accept): (Option<String>, Option<String>, i64)| {
                let name = name.unwrap_or_default();
                let id_str = id.unwrap_or_default();
                let mut owner_id = uuid::Uuid::nil();
                let parsed = uuid::Uuid::parse_str(id_str.trim()).ok();
                let filter_id = if let Some(u) = parsed {
                    if !u.is_nil() {
                        owner_id = u;
                        u.to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                let _ = owner_id;
                let mut c = ctx.write();
                c.collision_filter_accept = accept != 0;
                c.collision_filter_name = name.to_lowercase();
                c.collision_filter_id = filter_id;
                Ok(())
            })?,
        )?;
    }

    // llPassCollisions
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llPassCollisions",
            lua.create_function(move |_lua, pass: i64| {
                let mut c = ctx.write();
                c.pass_collisions = if pass == 1 { 1 } else { 0 };
                Ok(())
            })?,
        )?;
    }

    // llVolumeDetect
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llVolumeDetect",
            lua.create_function(move |_lua, detect: i64| {
                let mut c = ctx.write();
                c.volume_detect = detect != 0;
                Ok(())
            })?,
        )?;
    }

    // llSetCameraAtOffset
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetCameraAtOffset",
            lua.create_function(move |_lua, offset: Option<mlua::Table>| {
                let v = table_vec(offset.as_ref());
                let mut c = ctx.write();
                c.camera_at_offset = v;
                Ok(())
            })?,
        )?;
    }

    // llSetCameraEyeOffset
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetCameraEyeOffset",
            lua.create_function(move |_lua, offset: Option<mlua::Table>| {
                let v = table_vec(offset.as_ref());
                let mut c = ctx.write();
                c.camera_eye_offset = v;
                Ok(())
            })?,
        )?;
    }

    // llSetCameraParams
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetCameraParams",
            lua.create_function(move |_lua, rules: Option<mlua::Table>| {
                let mut params = Vec::new();
                if let Some(t) = rules.as_ref() {
                    let n = t.len().unwrap_or(0);
                    for i in 1..=n {
                        if let Ok(v) = t.get::<mlua::Value>(i) {
                            match v {
                                mlua::Value::Integer(x) => params.push(x as f64),
                                mlua::Value::Number(x) => params.push(x),
                                mlua::Value::Table(tt) => {
                                    params.push(tt.get::<f64>("x").unwrap_or(0.0));
                                    params.push(tt.get::<f64>("y").unwrap_or(0.0));
                                    params.push(tt.get::<f64>("z").unwrap_or(0.0));
                                }
                                _ => {}
                            }
                        }
                    }
                }
                let mut c = ctx.write();
                c.camera_params = params;
                Ok(())
            })?,
        )?;
    }

    // llTargetOmega
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llTargetOmega",
            lua.create_function(move |_lua, (axis, spinrate, gain): (Option<mlua::Table>, f64, f64)| {
                let a = table_vec(axis.as_ref());
                let omega = [a[0] * spinrate, a[1] * spinrate, a[2] * spinrate, gain];
                let mut c = ctx.write();
                c.target_omega = omega;
                Ok(())
            })?,
        )?;
    }

    // llSetVelocity
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetVelocity",
            lua.create_function(move |_lua, (vel, local): (Option<mlua::Table>, i64)| {
                let v = table_vec(vel.as_ref());
                let mut c = ctx.write();
                let world = if local != 0 {
                    rotate_vector(c.rotation, v)
                } else {
                    v
                };
                c.velocity = world;
                Ok(())
            })?,
        )?;
    }

    // llSetRegionPos
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetRegionPos",
            lua.create_function(move |_lua, pos: Option<mlua::Table>| {
                let p = table_vec(pos.as_ref());
                let mut c = ctx.write();
                let size = c.region_size;
                let ok = p[0] >= 0.0
                    && p[0] < size[0] as f64
                    && p[1] >= 0.0
                    && p[1] < size[1] as f64
                    && p[2] >= 0.0;
                if ok {
                    c.position = p;
                    Ok(1i64)
                } else {
                    Ok(0i64)
                }
            })?,
        )?;
    }

    // llStopLookAt
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llStopLookAt",
            lua.create_function(move |_lua, ()| {
                let mut c = ctx.write();
                c.look_at_target = None;
                Ok(())
            })?,
        )?;
    }

    // llRotLookAt (not supported with available context fields)
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llRotLookAt",
            lua.create_function(move |_lua, (_target, _strength, _damping): (Option<mlua::Table>, f64, f64)| {
                let c = ctx.read();
                notify_unsupported(&c, "llRotLookAt");
                Ok(())
            })?,
        )?;
    }

    // llSleep
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSleep",
            lua.create_function(move |_lua, seconds: f64| {
                let mut c = ctx.write();
                c.sleep_requested += seconds.max(0.0);
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
                let mut c = ctx.write();
                c.reset_requested = true;
                Ok(())
            })?,
        )?;
    }

    // llKey2Name
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llKey2Name",
            lua.create_function(move |_lua, key: Option<String>| {
                let key = key.unwrap_or_default();
                let parsed = uuid::Uuid::parse_str(key.trim()).ok();
                let c = ctx.read();
                if let Some(u) = parsed {
                    for p in &c.presence_list {
                        if p.uuid == u {
                            return Ok(format!("{} {}", p.first_name, p.last_name).trim().to_string());
                        }
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
            lua.create_function(move |_lua, key: Option<String>| {
                let key = key.unwrap_or_default();
                let parsed = uuid::Uuid::parse_str(key.trim()).unwrap_or(uuid::Uuid::nil());
                let c = ctx.read();
                if c.owner_id == parsed {
                    Ok(1i64)
                } else {
                    Ok(0i64)
                }
            })?,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;
    use std::sync::Arc;
    use parking_lot::RwLock;

    fn new_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext::default()))
    }

    #[test]
    fn text_sets_fields() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetText("hi",{x=2,y=-1,z=0.5},0.25)"#)
            .exec()
            .unwrap();
        let c = ctx.read();
        assert_eq!(c.hover_text, "hi");
        assert_eq!(c.hover_text_color, [1.0, 0.0, 0.5]);
    }

    #[test]
    fn click_action_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetClickAction(7)"#).exec().unwrap();
        assert_eq!(ctx.read().click_action, 7);
    }

    #[test]
    fn sit_text_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetSitText("sit here")"#).exec().unwrap();
        assert_eq!(ctx.read().sit_text, "sit here");
    }

    #[test]
    fn local_rot_sets_rotation() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetLocalRot({x=1,y=2,z=3,w=4})"#)
            .exec()
            .unwrap();
        assert_eq!(ctx.read().rotation, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn link_color_sets_map() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetLinkColor(3,{x=2,y=0.5,z=-1},0)"#)
            .exec()
            .unwrap();
        assert_eq!(ctx.read().link_colors.get(&3), Some(&[1.0, 0.5, 0.0]));
    }

    #[test]
    fn remote_pin_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetRemoteScriptAccessPin(42)"#).exec().unwrap();
        assert_eq!(ctx.read().remote_script_pin, 42);
    }

    #[test]
    fn collision_filter_sets_fields() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        let id = uuid::Uuid::nil().to_string();
        lua.load(format!(
            r#"llCollisionFilter("Foo", "{}", 1)"#,
            id
        ))
        .exec()
        .unwrap();
        let c = ctx.read();
        assert!(c.collision_filter_accept);
        assert_eq!(c.collision_filter_name, "foo");
    }

    #[test]
    fn pass_collisions_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llPassCollisions(1)"#).exec().unwrap();
        assert_eq!(ctx.read().pass_collisions, 1);
    }

    #[test]
    fn volume_detect_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llVolumeDetect(1)"#).exec().unwrap();
        assert!(ctx.read().volume_detect);
    }

    #[test]
    fn camera_at_offset_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetCameraAtOffset({x=1,y=2,z=3})"#)
            .exec()
            .unwrap();
        assert_eq!(ctx.read().camera_at_offset, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn camera_eye_offset_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetCameraEyeOffset({x=4,y=5,z=6})"#)
            .exec()
            .unwrap();
        assert_eq!(ctx.read().camera_eye_offset, [4.0, 5.0, 6.0]);
    }

    #[test]
    fn camera_params_stores_list() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetCameraParams({1,2,3})"#).exec().unwrap();
        assert_eq!(ctx.read().camera_params, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn target_omega_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llTargetOmega({x=1,y=0,z=0}, 2.0, 0.5)"#)
            .exec()
            .unwrap();
        assert_eq!(ctx.read().target_omega, [2.0, 0.0, 0.0, 0.5]);
    }

    #[test]
    fn set_velocity_sets_field() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSetVelocity({x=1,y=2,z=3}, 0)"#).exec().unwrap();
        assert_eq!(ctx.read().velocity, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn set_region_pos_returns_success() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        let v: i64 = lua.load(r#"llSetRegionPos({x=10,y=20,z=30})"#).eval().unwrap();
        assert_eq!(v, 1);
        assert_eq!(ctx.read().position, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn stop_look_at_clears_target() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llStopLookAt()"#).exec().unwrap();
        assert!(ctx.read().look_at_target.is_none());
    }

    #[test]
    fn rot_look_at_does_not_panic() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llRotLookAt({x=0,y=0,z=0,w=1},1.0,1.0)"#)
            .exec()
            .unwrap();
    }

    #[test]
    fn sleep_accumulates() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llSleep(1.5); llSleep(-1)"#).exec().unwrap();
        assert_eq!(ctx.read().sleep_requested, 1.5);
    }

    #[test]
    fn reset_script_sets_flag() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        lua.load(r#"llResetScript()"#).exec().unwrap();
        assert!(ctx.read().reset_requested);
    }

    #[test]
    fn key2name_returns_empty_for_unknown() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        let s: String = lua
            .load(format!(r#"llKey2Name("{}")"#, uuid::Uuid::nil()))
            .eval()
            .unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn same_group_owner_matches() {
        let lua = Lua::new();
        let ctx = new_ctx();
        register_extra_g2(&lua, &ctx).unwrap();
        let id = uuid::Uuid::nil().to_string();
        let v: i64 = lua.load(format!(r#"llSameGroup("{}")"#, id)).eval().unwrap();
        assert_eq!(v, 1);
    }
}
