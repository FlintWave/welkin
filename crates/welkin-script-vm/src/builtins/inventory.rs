// SPDX-License-Identifier: LGPL-3.0-or-later
use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::RwLock;
use uuid::Uuid;

use super::{
    ScriptContext, INVENTORY_ALL, INVENTORY_ANIMATION, INVENTORY_BODYPART, INVENTORY_CLOTHING,
    INVENTORY_GESTURE, INVENTORY_LANDMARK, INVENTORY_NONE, INVENTORY_NOTECARD, INVENTORY_OBJECT,
    INVENTORY_SCRIPT, INVENTORY_SOUND, INVENTORY_TEXTURE,
};

pub fn register_inventory(
    lua: &Lua,
    ctx: &Arc<RwLock<ScriptContext>>,
) -> Result<(), crate::VmError> {
    lua.globals().set("INVENTORY_TEXTURE", INVENTORY_TEXTURE)?;
    lua.globals().set("INVENTORY_SOUND", INVENTORY_SOUND)?;
    lua.globals().set("INVENTORY_LANDMARK", INVENTORY_LANDMARK)?;
    lua.globals().set("INVENTORY_CLOTHING", INVENTORY_CLOTHING)?;
    lua.globals().set("INVENTORY_OBJECT", INVENTORY_OBJECT)?;
    lua.globals().set("INVENTORY_NOTECARD", INVENTORY_NOTECARD)?;
    lua.globals().set("INVENTORY_SCRIPT", INVENTORY_SCRIPT)?;
    lua.globals().set("INVENTORY_BODYPART", INVENTORY_BODYPART)?;
    lua.globals().set("INVENTORY_ANIMATION", INVENTORY_ANIMATION)?;
    lua.globals().set("INVENTORY_GESTURE", INVENTORY_GESTURE)?;
    lua.globals().set("INVENTORY_ALL", INVENTORY_ALL)?;
    lua.globals().set("INVENTORY_NONE", INVENTORY_NONE)?;

    // llGetInventoryNumber(type) -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, inv_type: i32| {
            let ctx = ctx.read();
            if inv_type == INVENTORY_ALL {
                Ok(ctx.inventory.len() as i64)
            } else {
                Ok(ctx
                    .inventory
                    .iter()
                    .filter(|i| i.item_type == inv_type)
                    .count() as i64)
            }
        })?;
        lua.globals().set("llGetInventoryNumber", f)?;
    }

    // llGetInventoryName(type, index) -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (inv_type, index): (i32, i64)| {
            let ctx = ctx.read();
            let matching: Vec<&str> = if inv_type == INVENTORY_ALL {
                ctx.inventory.iter().map(|i| i.name.as_str()).collect()
            } else {
                ctx.inventory
                    .iter()
                    .filter(|i| i.item_type == inv_type)
                    .map(|i| i.name.as_str())
                    .collect()
            };
            let idx = index as usize;
            Ok(matching.get(idx).copied().unwrap_or("").to_string())
        })?;
        lua.globals().set("llGetInventoryName", f)?;
    }

    // llGetInventoryType(name) -> integer
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, name: String| {
            let ctx = ctx.read();
            Ok(ctx
                .inventory
                .iter()
                .find(|i| i.name == name)
                .map_or(INVENTORY_NONE as i64, |i| i.item_type as i64))
        })?;
        lua.globals().set("llGetInventoryType", f)?;
    }

    // llGetInventoryKey(name) -> string
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, name: String| {
            let ctx = ctx.read();
            Ok(ctx
                .inventory
                .iter()
                .find(|i| i.name == name)
                .map_or_else(
                    || uuid::Uuid::nil().to_string(),
                    |i| i.asset_id.to_string(),
                ))
        })?;
        lua.globals().set("llGetInventoryKey", f)?;
    }

    // llRemoveInventory(name)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, name: String| {
            let mut ctx = ctx.write();
            let before = ctx.inventory.len();
            ctx.inventory.retain(|i| i.name != name);
            if ctx.inventory.len() < before {
                tracing::debug!(%name, "llRemoveInventory: removed item");
            } else {
                tracing::debug!(%name, "llRemoveInventory: item not found");
            }
            Ok(())
        })?;
        lua.globals().set("llRemoveInventory", f)?;
    }

    // llAllowInventoryDrop(allow)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, allow: i64| {
            let mut ctx = ctx.write();
            ctx.allow_inventory_drop = allow != 0;
            tracing::debug!(allow = ctx.allow_inventory_drop, "llAllowInventoryDrop");
            Ok(())
        })?;
        lua.globals().set("llAllowInventoryDrop", f)?;
    }

    // llGiveInventory(destination, name)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (destination, name): (String, String)| {
            let ctx = ctx.read();
            if let Some(item) = ctx.inventory.iter().find(|i| i.name == name) {
                tracing::debug!(
                    %destination,
                    item_name = %item.name,
                    item_type = item.item_type,
                    asset_id = %item.asset_id,
                    "llGiveInventory: giving item"
                );
            } else {
                tracing::warn!(%name, "llGiveInventory: item not found in inventory");
            }
            Ok(())
        })?;
        lua.globals().set("llGiveInventory", f)?;
    }

    // llGiveInventoryList(destination, folder, items)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(
            move |_, (destination, folder, items): (String, String, mlua::Table)| {
                let ctx = ctx.read();
                let mut found = 0i32;
                let mut missing = 0i32;
                for entry in items.sequence_values::<String>() {
                    if let Ok(name) = entry {
                        if ctx.inventory.iter().any(|i| i.name == name) {
                            found += 1;
                        } else {
                            missing += 1;
                            tracing::warn!(%name, "llGiveInventoryList: item not found");
                        }
                    }
                }
                tracing::debug!(
                    %destination,
                    %folder,
                    found,
                    missing,
                    "llGiveInventoryList: giving items"
                );
                Ok(())
            },
        )?;
        lua.globals().set("llGiveInventoryList", f)?;
    }

    // llGetNotecardLine(notecard_name, line) -> key (request id)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, (name, line): (String, i64)| {
            let ctx = ctx.read();
            let asset_id = match Uuid::parse_str(&name) {
                Ok(u) => u,
                Err(_) => match ctx
                    .inventory
                    .iter()
                    .find(|i| i.name == name && i.item_type == INVENTORY_NOTECARD)
                {
                    Some(item) => item.asset_id,
                    None => Uuid::nil(),
                },
            };
            if asset_id.is_nil() {
                return Ok(Uuid::nil().to_string());
            }
            let request_id = Uuid::new_v4().to_string();
            let data = if let Some(content) = ctx.notecard_data.get(&asset_id) {
                if line < 0 {
                    String::new()
                } else {
                    let lines: Vec<&str> = content.lines().collect();
                    let idx = line as usize;
                    if idx < lines.len() {
                        lines[idx].to_string()
                    } else {
                        "\n\n\n".to_string()
                    }
                }
            } else {
                String::new()
            };
            if let Some(ref tx) = ctx.event_sender {
                    let event = crate::ScriptEvent::Dataserver {
                        request_id: request_id.clone(),
                        data,
                    };
                    let _ = tx.send((ctx.prim_id, event));
                }
            Ok(request_id)
        })?;
        lua.globals().set("llGetNotecardLine", f)?;
    }

    // llGetNumberOfNotecardLines(notecard_name) -> key (request id)
    {
        let ctx = Arc::clone(ctx);
        let f = lua.create_function(move |_, name: String| {
            let ctx = ctx.read();
            let asset_id = match Uuid::parse_str(&name) {
                Ok(u) => u,
                Err(_) => match ctx
                    .inventory
                    .iter()
                    .find(|i| i.name == name && i.item_type == INVENTORY_NOTECARD)
                {
                    Some(item) => item.asset_id,
                    None => Uuid::nil(),
                },
            };
            if asset_id.is_nil() {
                return Ok(Uuid::nil().to_string());
            }
            let request_id = Uuid::new_v4().to_string();
            let count = if let Some(content) = ctx.notecard_data.get(&asset_id) {
                content.lines().count()
            } else {
                0
            };
            if let Some(ref tx) = ctx.event_sender {
                    let event = crate::ScriptEvent::Dataserver {
                        request_id: request_id.clone(),
                        data: count.to_string(),
                    };
                    let _ = tx.send((ctx.prim_id, event));
                }
            Ok(request_id)
        })?;
        lua.globals().set("llGetNumberOfNotecardLines", f)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::InventoryItem;
    use uuid::Uuid;

    fn test_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            inventory: vec![
                InventoryItem {
                    name: "MyScript".into(),
                    item_type: INVENTORY_SCRIPT,
                    asset_id: Uuid::nil(),
                    ..Default::default()
                },
                InventoryItem {
                    name: "MyTexture".into(),
                    item_type: INVENTORY_TEXTURE,
                    asset_id: Uuid::from_u128(0xabcd),
                    ..Default::default()
                },
                InventoryItem {
                    name: "MyNotecard".into(),
                    item_type: INVENTORY_NOTECARD,
                    asset_id: Uuid::from_u128(0x1234),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }))
    }

    #[test]
    fn get_inventory_number_all() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_inventory(&lua, &ctx).unwrap();
        let n: i64 = lua
            .load("return llGetInventoryNumber(INVENTORY_ALL)")
            .eval()
            .unwrap();
        assert_eq!(n, 3);
    }

    #[test]
    fn get_inventory_number_by_type() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_inventory(&lua, &ctx).unwrap();
        let n: i64 = lua
            .load("return llGetInventoryNumber(INVENTORY_SCRIPT)")
            .eval()
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn get_inventory_name_valid() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_inventory(&lua, &ctx).unwrap();
        let name: String = lua
            .load("return llGetInventoryName(INVENTORY_SCRIPT, 0)")
            .eval()
            .unwrap();
        assert_eq!(name, "MyScript");
    }

    #[test]
    fn get_inventory_name_out_of_range() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_inventory(&lua, &ctx).unwrap();
        let name: String = lua
            .load("return llGetInventoryName(INVENTORY_SCRIPT, 1)")
            .eval()
            .unwrap();
        assert_eq!(name, "");
    }

    #[test]
    fn get_inventory_type_found() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_inventory(&lua, &ctx).unwrap();
        let t: i64 = lua
            .load(r#"return llGetInventoryType("MyTexture")"#)
            .eval()
            .unwrap();
        assert_eq!(t, INVENTORY_TEXTURE as i64);
    }

    #[test]
    fn get_inventory_type_not_found() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_inventory(&lua, &ctx).unwrap();
        let t: i64 = lua
            .load(r#"return llGetInventoryType("NonExistent")"#)
            .eval()
            .unwrap();
        assert_eq!(t, INVENTORY_NONE as i64);
    }

    #[test]
    fn get_inventory_key() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_inventory(&lua, &ctx).unwrap();
        let key: String = lua
            .load(r#"return llGetInventoryKey("MyTexture")"#)
            .eval()
            .unwrap();
        assert_eq!(key, Uuid::from_u128(0xabcd).to_string());
    }

    #[test]
    fn remove_inventory() {
        let lua = Lua::new();
        let ctx = test_ctx();
        register_inventory(&lua, &ctx).unwrap();
        lua.load(r#"llRemoveInventory("MyTexture")"#)
            .exec()
            .unwrap();
        let n: i64 = lua
            .load("return llGetInventoryNumber(INVENTORY_ALL)")
            .eval()
            .unwrap();
        assert_eq!(n, 2);
    }

    fn ctx_with_notecard() -> (
        Arc<RwLock<ScriptContext>>,
        std::sync::mpsc::Receiver<(Uuid, crate::ScriptEvent)>,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        let notecard_id = Uuid::from_u128(0x1234);
        let mut notecard_data = std::collections::HashMap::new();
        notecard_data.insert(notecard_id, "Line zero\nLine one\nLine two".to_string());
        let ctx = Arc::new(RwLock::new(ScriptContext {
            inventory: vec![
                InventoryItem {
                    name: "MyNotecard".into(),
                    item_type: INVENTORY_NOTECARD,
                    asset_id: notecard_id,
                    ..Default::default()
                },
            ],
            event_sender: Some(tx),
            notecard_data,
            ..Default::default()
        }));
        (ctx, rx)
    }

    #[test]
    fn get_notecard_line_returns_content() {
        let (ctx, rx) = ctx_with_notecard();
        let lua = Lua::new();
        register_inventory(&lua, &ctx).unwrap();

        let key: String = lua
            .load(r#"return llGetNotecardLine("MyNotecard", 1)"#)
            .eval()
            .unwrap();
        assert!(!key.is_empty());

        let (_prim_id, event) = rx.try_recv().unwrap();
        match event {
            crate::ScriptEvent::Dataserver { request_id, data } => {
                assert_eq!(request_id, key);
                assert_eq!(data, "Line one");
            }
            _ => panic!("expected Dataserver event"),
        }
    }

    #[test]
    fn get_notecard_line_eof_past_end() {
        let (ctx, rx) = ctx_with_notecard();
        let lua = Lua::new();
        register_inventory(&lua, &ctx).unwrap();

        lua.load(r#"llGetNotecardLine("MyNotecard", 99)"#)
            .exec()
            .unwrap();

        let (_prim_id, event) = rx.try_recv().unwrap();
        match event {
            crate::ScriptEvent::Dataserver { data, .. } => {
                assert_eq!(data, "\n\n\n");
            }
            _ => panic!("expected Dataserver event"),
        }
    }

    #[test]
    fn get_number_of_notecard_lines() {
        let (ctx, rx) = ctx_with_notecard();
        let lua = Lua::new();
        register_inventory(&lua, &ctx).unwrap();

        let key: String = lua
            .load(r#"return llGetNumberOfNotecardLines("MyNotecard")"#)
            .eval()
            .unwrap();
        assert!(!key.is_empty());

        let (_prim_id, event) = rx.try_recv().unwrap();
        match event {
            crate::ScriptEvent::Dataserver { request_id, data } => {
                assert_eq!(request_id, key);
                assert_eq!(data, "3");
            }
            _ => panic!("expected Dataserver event"),
        }
    }
}
