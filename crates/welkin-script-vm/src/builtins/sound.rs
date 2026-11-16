// SPDX-License-Identifier: LGPL-3.0-or-later

use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use super::{ScriptContext, SoundCommand};

fn resolve_sound(ctx: &ScriptContext, sound: &str) -> Uuid {
    if let Ok(uuid) = Uuid::parse_str(sound) {
        if !uuid.is_nil() {
            return uuid;
        }
    }
    if let Some(item) = ctx
        .inventory
        .iter()
        .find(|item| item.name == sound && item.item_type == crate::builtins::INVENTORY_SOUND)
    {
        return item.asset_id;
    }
    Uuid::nil()
}

fn vec3(t: &mlua::Table) -> mlua::Result<[f64; 3]> {
    let x = t
        .get::<f64>("x")
        .or_else(|_| t.get::<f64>(1))
        .unwrap_or(0.0);
    let y = t
        .get::<f64>("y")
        .or_else(|_| t.get::<f64>(2))
        .unwrap_or(0.0);
    let z = t
        .get::<f64>("z")
        .or_else(|_| t.get::<f64>(3))
        .unwrap_or(0.0);
    Ok([x, y, z])
}

pub fn register_sound(
    lua: &mlua::Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llPlaySound",
            lua.create_function(move |_lua, (sound, volume): (Option<String>, f64)| {
                let ctx = ctx.read();
                let sound_id = resolve_sound(&ctx, &sound.unwrap_or_default());
                if sound_id.is_nil() {
                    return Ok(());
                }
                let volume = volume.clamp(0.0, 1.0);
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::PlaySound {
                        prim_id: ctx.prim_id,
                        sound_id,
                        volume,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llLoopSound",
            lua.create_function(move |_lua, (sound, volume): (Option<String>, f64)| {
                let ctx = ctx.read();
                let sound_id = resolve_sound(&ctx, &sound.unwrap_or_default());
                if sound_id.is_nil() {
                    return Ok(());
                }
                let volume = volume.clamp(0.0, 1.0);
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::LoopSound {
                        prim_id: ctx.prim_id,
                        sound_id,
                        volume,
                        master: false,
                        slave: false,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llLoopSoundMaster",
            lua.create_function(move |_lua, (sound, volume): (Option<String>, f64)| {
                let ctx = ctx.read();
                let sound_id = resolve_sound(&ctx, &sound.unwrap_or_default());
                if sound_id.is_nil() {
                    return Ok(());
                }
                let volume = volume.clamp(0.0, 1.0);
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::LoopSound {
                        prim_id: ctx.prim_id,
                        sound_id,
                        volume,
                        master: true,
                        slave: false,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llLoopSoundSlave",
            lua.create_function(move |_lua, (sound, volume): (Option<String>, f64)| {
                let ctx = ctx.read();
                let sound_id = resolve_sound(&ctx, &sound.unwrap_or_default());
                if sound_id.is_nil() {
                    return Ok(());
                }
                let volume = volume.clamp(0.0, 1.0);
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::LoopSound {
                        prim_id: ctx.prim_id,
                        sound_id,
                        volume,
                        master: false,
                        slave: true,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llPlaySoundSlave",
            lua.create_function(move |_lua, (sound, volume): (Option<String>, f64)| {
                let ctx = ctx.read();
                let sound_id = resolve_sound(&ctx, &sound.unwrap_or_default());
                if sound_id.is_nil() {
                    return Ok(());
                }
                let volume = volume.clamp(0.0, 1.0);
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::PlaySound {
                        prim_id: ctx.prim_id,
                        sound_id,
                        volume,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llStopSound",
            lua.create_function(move |_lua, ()| {
                let ctx = ctx.read();
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::StopSound {
                        prim_id: ctx.prim_id,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llPreloadSound",
            lua.create_function(move |_lua, (sound,): (Option<String>,)| {
                let ctx = ctx.read();
                let sound_id = resolve_sound(&ctx, &sound.unwrap_or_default());
                if sound_id.is_nil() {
                    return Ok(());
                }
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::PreloadSound {
                        prim_id: ctx.prim_id,
                        sound_id,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llTriggerSound",
            lua.create_function(move |_lua, (sound, volume): (Option<String>, f64)| {
                let ctx = ctx.read();
                let sound_id = resolve_sound(&ctx, &sound.unwrap_or_default());
                if sound_id.is_nil() {
                    return Ok(());
                }
                let volume = volume.clamp(0.0, 1.0);
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::TriggerSound {
                        prim_id: ctx.prim_id,
                        sound_id,
                        volume,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llTriggerSoundLimited",
            lua.create_function(
                move |_lua,
                      (sound, volume, top_ne, bottom_sw): (
                          Option<String>,
                          f64,
                          mlua::Table,
                          mlua::Table,
                      )| {
                    let ctx = ctx.read();
                    let sound_id = resolve_sound(&ctx, &sound.unwrap_or_default());
                    if sound_id.is_nil() {
                        return Ok(());
                    }
                    let volume = volume.clamp(0.0, 1.0);
                    let top_ne = vec3(&top_ne)?;
                    let bottom_sw = vec3(&bottom_sw)?;
                    if let Some(sender) = &ctx.sound_sender {
                        let _ = sender.send(SoundCommand::TriggerSoundLimited {
                            prim_id: ctx.prim_id,
                            sound_id,
                            volume,
                            top_ne,
                            bottom_sw,
                        });
                    }
                    Ok(())
                },
            )?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llAdjustSoundVolume",
            lua.create_function(move |_lua, (volume,): (f64,)| {
                let ctx = ctx.read();
                let volume = volume.clamp(0.0, 1.0);
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::AdjustVolume {
                        prim_id: ctx.prim_id,
                        volume,
                    });
                }
                Ok(())
            })?,
        )?;
    }

    {
        let ctx = Arc::clone(ctx);
        lua.globals().set(
            "llSetSoundRadius",
            lua.create_function(move |_lua, (radius,): (f64,)| {
                let ctx = ctx.read();
                let radius = radius.max(0.0);
                if let Some(sender) = &ctx.sound_sender {
                    let _ = sender.send(SoundCommand::SetRadius {
                        prim_id: ctx.prim_id,
                        radius,
                    });
                }
                Ok(())
            })?,
        )?;
    }

        {
            let ctx = Arc::clone(ctx);
            lua.globals().set(
                "llCollisionSound",
                lua.create_function(move |_lua, (sound, volume): (Option<String>, f64)| {
                    let ctx = ctx.read();
                    let sound_id = match sound {
                        Some(sound) => resolve_sound(&ctx, &sound),
                        None => Uuid::nil(),
                    };
                    let volume = volume.clamp(0.0, 1.0);
                    if let Some(sender) = &ctx.sound_sender {
                        let _ = sender.send(SoundCommand::CollisionSound {
                            prim_id: ctx.prim_id,
                            sound_id,
                            volume,
                        });
                    }
                    Ok(())
                })?,
            )?;
        }
        {
            let ctx = Arc::clone(ctx);
            lua.globals().set(
                "llSetSoundQueueing",
                lua.create_function(move |_lua, (enabled,): (i64,)| {
                    let ctx = ctx.read();
                    if let Some(sender) = &ctx.sound_sender {
                        let _ = sender.send(SoundCommand::SetQueueing {
                            prim_id: ctx.prim_id,
                            enabled: enabled != 0,
                        });
                    }
                    Ok(())
                })?,
            )?;
        }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn play_sound_sends_command() {
        let lua = mlua::Lua::new();
        let (tx, rx) = mpsc::channel();
        let prim_id = Uuid::new_v4();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            sound_sender: Some(tx),
            prim_id,
            ..Default::default()
        }));
        register_sound(&lua, &ctx).unwrap();
        lua.load(r#"llPlaySound("00000000-0000-0000-0000-000000000001", 0.5)"#)
            .exec()
            .unwrap();
        match rx.try_recv().unwrap() {
            SoundCommand::PlaySound {
                sound_id,
                volume,
                ..
            } => {
                assert_eq!(
                    sound_id,
                    Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
                );
                assert_eq!(volume, 0.5);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn nil_sound_sends_no_command() {
        let lua = mlua::Lua::new();
        let (tx, rx) = mpsc::channel();
        let prim_id = Uuid::new_v4();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            sound_sender: Some(tx),
            prim_id,
            ..Default::default()
        }));
        register_sound(&lua, &ctx).unwrap();
        lua.load(r#"llPlaySound("00000000-0000-0000-0000-000000000000", 0.5)"#)
            .exec()
            .unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn loop_sound_master_sets_master() {
        let lua = mlua::Lua::new();
        let (tx, rx) = mpsc::channel();
        let prim_id = Uuid::new_v4();
        let ctx = Arc::new(RwLock::new(ScriptContext {
            sound_sender: Some(tx),
            prim_id,
            ..Default::default()
        }));
        register_sound(&lua, &ctx).unwrap();
        lua.load(r#"llLoopSoundMaster("00000000-0000-0000-0000-000000000001", 1.0)"#)
            .exec()
            .unwrap();
        match rx.try_recv().unwrap() {
            SoundCommand::LoopSound { master, .. } => assert!(master),
            other => panic!("unexpected command: {:?}", other),
        }
    }
}
