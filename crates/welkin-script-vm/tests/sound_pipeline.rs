// SPDX-License-Identifier: LGPL-3.0-or-later

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use welkin_script_vm::builtins::{register_all, ScriptContext, SoundCommand};

fn run(src: &str) -> (String, SoundCommand) {
    let luau = welkin_script_vm::compile_lsl(src).expect("LSL compile");
    let bytecode = welkin_script_vm::compile_source(&luau).expect("luau compile");
    let lua = welkin_script_vm::new_vm().unwrap();
    let (tx, rx) = mpsc::channel();
    let ctx = Arc::new(parking_lot::RwLock::new(ScriptContext {
        sound_sender: Some(tx),
        ..Default::default()
    }));
    register_all(&lua, &ctx).unwrap();
    lua.load(&bytecode).exec().unwrap();
    let prim_id = ctx.read().prim_id.to_string();
    let dispatch: mlua::Function = lua.globals().get("__dispatch").unwrap();
    dispatch.call::<()>(("state_entry",)).unwrap();
    let cmd = rx.recv().unwrap();
    (prim_id, cmd)
}

#[test]
fn ll_play_sound() {
    let src = r#"default { state_entry() { llPlaySound("00000000-0000-0000-0000-000000000042", 1.0); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::PlaySound { prim_id: p, sound_id, volume } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
            assert_eq!(volume, 1.0);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_loop_sound() {
    let src = r#"default { state_entry() { llLoopSound("00000000-0000-0000-0000-000000000042", 1.0); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::LoopSound { prim_id: p, sound_id, volume, master, slave } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
            assert_eq!(volume, 1.0);
            assert!(!master);
            assert!(!slave);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_loop_sound_master() {
    let src = r#"default { state_entry() { llLoopSoundMaster("00000000-0000-0000-0000-000000000042", 1.0); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::LoopSound { prim_id: p, sound_id, volume, master, slave } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
            assert_eq!(volume, 1.0);
            assert!(master);
            assert!(!slave);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_loop_sound_slave() {
    let src = r#"default { state_entry() { llLoopSoundSlave("00000000-0000-0000-0000-000000000042", 1.0); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::LoopSound { prim_id: p, sound_id, volume, master, slave } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
            assert_eq!(volume, 1.0);
            assert!(!master);
            assert!(slave);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_play_sound_slave() {
    let src = r#"default { state_entry() { llPlaySoundSlave("00000000-0000-0000-0000-000000000042", 1.0); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::PlaySound { prim_id: p, sound_id, volume } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
            assert_eq!(volume, 1.0);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_stop_sound() {
    let src = r#"default { state_entry() { llStopSound(); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::StopSound { prim_id: p } => {
            assert_eq!(p.to_string(), prim_id);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_preload_sound() {
    let src = r#"default { state_entry() { llPreloadSound("00000000-0000-0000-0000-000000000042"); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::PreloadSound { prim_id: p, sound_id } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_adjust_sound_volume() {
    let src = r#"default { state_entry() { llAdjustSoundVolume(0.5); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::AdjustVolume { prim_id: p, volume } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(volume, 0.5);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_trigger_sound() {
    let src = r#"default { state_entry() { llTriggerSound("00000000-0000-0000-0000-000000000042", 0.5); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::TriggerSound { prim_id: p, sound_id, volume } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
            assert_eq!(volume, 0.5);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_trigger_sound_limited() {
    let src = r#"default { state_entry() { llTriggerSoundLimited("00000000-0000-0000-0000-000000000042", 1.0, <1.0, 2.0, 3.0>, <4.0, 5.0, 6.0>); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::TriggerSoundLimited { prim_id: p, sound_id, volume, top_ne, bottom_sw } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
            assert_eq!(volume, 1.0);
            assert_eq!(top_ne, [1.0, 2.0, 3.0]);
            assert_eq!(bottom_sw, [4.0, 5.0, 6.0]);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_set_sound_radius() {
    let src = r#"default { state_entry() { llSetSoundRadius(2.5); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::SetRadius { prim_id: p, radius } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(radius, 2.5);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_collision_sound() {
    let src = r#"default { state_entry() { llCollisionSound("00000000-0000-0000-0000-000000000042", 0.5); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::CollisionSound { prim_id: p, sound_id, volume } => {
            assert_eq!(p.to_string(), prim_id);
            assert_eq!(sound_id.to_string(), "00000000-0000-0000-0000-000000000042");
            assert_eq!(volume, 0.5);
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_collision_sound_empty_string_disables() {
    let src = r#"default { state_entry() { llCollisionSound("", 1.0); } }"#;
    let (_prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::CollisionSound { sound_id, .. } => {
            assert_eq!(sound_id.to_string(), "ffffffff-ffff-ffff-ffff-ffffffffffff");
        }
        _ => panic!("unexpected sound command"),
    }
}

#[test]
fn ll_set_sound_queueing() {
    let src = r#"default { state_entry() { llSetSoundQueueing(1); } }"#;
    let (prim_id, cmd) = run(src);
    match cmd {
        SoundCommand::SetQueueing { prim_id: p, enabled } => {
            assert_eq!(p.to_string(), prim_id);
            assert!(enabled);
        }
        _ => panic!("unexpected sound command"),
    }
}
