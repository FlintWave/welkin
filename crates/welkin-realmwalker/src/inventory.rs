// SPDX-License-Identifier: LGPL-3.0-or-later
//! Inventory: folders, items, fetching, CRUD, offers.

use uuid::Uuid;
use welkin_msg::msgs::*;
use welkin_msg::{IncomingPacket, Msg};

use crate::error::RealmWalkerError;
use crate::events::{Event, EventBus};
use crate::network::Network;

/// Inventory system folder types (from OpenSim/OpenMetaverse).
pub mod folder_type {
    pub const ROOT: i8 = -1;
    pub const TEXTURE: i8 = 0;
    pub const SOUND: i8 = 1;
    pub const CALLING_CARD: i8 = 2;
    pub const LANDMARK: i8 = 3;
    pub const CLOTHING: i8 = 5;
    pub const OBJECT: i8 = 6;
    pub const NOTECARD: i8 = 7;
    pub const LSL_TEXT: i8 = 10;
    pub const LSL_BYTECODE: i8 = 11;
    pub const SNAPSHOT: i8 = 15;
    pub const TASK_INVENTORY: i8 = 47;
    pub const TRASH: i8 = 14;
    pub const LOST_AND_FOUND: i8 = 16;
}

/// Asset types.
pub mod asset_type {
    pub const TEXTURE: i8 = 0;
    pub const SOUND: i8 = 1;
    pub const CALLING_CARD: i8 = 2;
    pub const LANDMARK: i8 = 3;
    pub const CLOTHING: i8 = 5;
    pub const OBJECT: i8 = 6;
    pub const NOTECARD: i8 = 7;
    pub const LSL_TEXT: i8 = 10;
    pub const LSL_BYTECODE: i8 = 11;
    pub const FOLDER: i8 = 8;
}

#[derive(Debug, Clone)]
pub struct InventoryFolder {
    pub folder_id: Uuid,
    pub parent_id: Uuid,
    pub folder_type: i8,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub item_id: Uuid,
    pub folder_id: Uuid,
    pub creator_id: Uuid,
    pub owner_id: Uuid,
    pub group_id: Uuid,
    pub base_mask: u32,
    pub owner_mask: u32,
    pub group_mask: u32,
    pub everyone_mask: u32,
    pub next_owner_mask: u32,
    pub group_owned: bool,
    pub asset_id: Uuid,
    pub asset_type: i8,
    pub inv_type: i8,
    pub flags: u32,
    pub sale_type: u8,
    pub sale_price: i32,
    pub name: String,
    pub description: String,
    pub creation_date: i32,
}

impl InventoryItem {
    pub fn from_item_data(d: &inventory_descendents::ItemData) -> Self {
        Self {
            item_id: d.item_id,
            folder_id: d.folder_id,
            creator_id: d.creator_id,
            owner_id: d.owner_id,
            group_id: d.group_id,
            base_mask: d.base_mask,
            owner_mask: d.owner_mask,
            group_mask: d.group_mask,
            everyone_mask: d.everyone_mask,
            next_owner_mask: d.next_owner_mask,
            group_owned: d.group_owned,
            asset_id: d.asset_id,
            asset_type: d.type_,
            inv_type: d.inv_type,
            flags: d.flags,
            sale_type: d.sale_type,
            sale_price: d.sale_price,
            name: String::from_utf8_lossy(&d.name).into_owned(),
            description: String::from_utf8_lossy(&d.description).into_owned(),
            creation_date: d.creation_date,
        }
    }
}

impl InventoryFolder {
    pub fn from_folder_data(d: &inventory_descendents::FolderData) -> Self {
        Self {
            folder_id: d.folder_id,
            parent_id: d.parent_id,
            folder_type: d.type_,
            name: String::from_utf8_lossy(&d.name).into_owned(),
        }
    }
}

pub async fn fetch_descendents(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    folder_id: Uuid,
    owner_id: Uuid,
) -> Result<(), RealmWalkerError> {
    let msg = FetchInventoryDescendents {
        agent_data: fetch_inventory_descendents::AgentData {
            agent_id,
            session_id,
        },
        inventory_data: fetch_inventory_descendents::InventoryData {
            folder_id,
            owner_id,
            fetch_folders: true,
            fetch_items: true,
            sort_order: 0,
        },
    };
    net.send_msg(&msg, true).await
}

pub async fn create_folder(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    folder_id: Uuid,
    parent_id: Uuid,
    folder_type: i8,
    name: &str,
) -> Result<(), RealmWalkerError> {
    let msg = CreateInventoryFolder {
        agent_data: create_inventory_folder::AgentData {
            agent_id,
            session_id,
        },
        folder_data: create_inventory_folder::FolderData {
            folder_id,
            parent_id,
            type_: folder_type,
            name: name.as_bytes().to_vec(),
        },
    };
    net.send_msg(&msg, true).await
}

pub async fn remove_folder(net: &Network, agent_id: Uuid, session_id: Uuid, folder_id: Uuid) -> Result<(), RealmWalkerError> {
    let msg = RemoveInventoryFolder {
        agent_data: remove_inventory_folder::AgentData {
            agent_id,
            session_id,
        },
        folder_data: vec![remove_inventory_folder::FolderData {
            folder_id,
        }],
    };
    net.send_msg(&msg, true).await
}

pub async fn move_item(
    net: &Network,
    agent_id: Uuid,
    session_id: Uuid,
    item_id: Uuid,
    _folder_id: Uuid,
    new_folder_id: Uuid,
) -> Result<(), RealmWalkerError> {
    let msg = MoveInventoryItem {
        agent_data: move_inventory_item::AgentData {
            agent_id,
            session_id,
            stamp: true,
        },
        inventory_data: vec![move_inventory_item::InventoryData {
            item_id,
            folder_id: new_folder_id,
            new_name: Vec::new(),
        }],
    };    net.send_msg(&msg, true).await
}

pub async fn remove_item(net: &Network, agent_id: Uuid, session_id: Uuid, item_id: Uuid) -> Result<(), RealmWalkerError> {
    let msg = RemoveInventoryItem {
        agent_data: remove_inventory_item::AgentData {
            agent_id,
            session_id,
        },
        inventory_data: vec![remove_inventory_item::InventoryData {
            item_id,
        }],
    };
    net.send_msg(&msg, true).await
}

pub async fn update_item(net: &Network, agent_id: Uuid, session_id: Uuid, item: &InventoryItem) -> Result<(), RealmWalkerError> {
    let msg = UpdateInventoryItem {
        agent_data: update_inventory_item::AgentData {
            agent_id,
            session_id,
            transaction_id: Uuid::nil(),
        },
        inventory_data: vec![update_inventory_item::InventoryData {
            item_id: item.item_id,
            folder_id: item.folder_id,
            callback_id: 0,
            creator_id: item.creator_id,
            owner_id: item.owner_id,
            group_id: item.group_id,
            base_mask: item.base_mask,
            owner_mask: item.owner_mask,
            group_mask: item.group_mask,
            everyone_mask: item.everyone_mask,
            next_owner_mask: item.next_owner_mask,
            group_owned: item.group_owned,
            transaction_id: Uuid::nil(),
            type_: item.asset_type as i8,
            inv_type: item.inv_type as i8,
            flags: item.flags,
            sale_type: item.sale_type,
            sale_price: item.sale_price,
            name: item.name.as_bytes().to_vec(),
            description: item.description.as_bytes().to_vec(),
            creation_date: item.creation_date,
            crc: 0,
        }],
    };
    net.send_msg(&msg, true).await
}

/// Parse an InventoryDescendents packet into folders + items.
pub fn try_handle_inventory_descendents(pkt: &IncomingPacket, event_bus: &EventBus) -> bool {
    if pkt.id != InventoryDescendents::ID {
        return false;
    }
    let Ok(msg) = pkt.decode::<InventoryDescendents>() else {
        return false;
    };
    let folders: Vec<InventoryFolder> = msg.folder_data.iter().map(InventoryFolder::from_folder_data).collect();
    let items: Vec<InventoryItem> = msg.item_data.iter().map(InventoryItem::from_item_data).collect();
    let _ = (folders, items);
    event_bus.send(Event::InventoryUpdated);
    true
}

// Need Event variants; if not exists compile fails. We'll add them.

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use welkin_msg::{build_packet, parse_packet};

    #[test]
    fn inventory_descendents_roundtrip() {
        let agent_id = Uuid::from_u128(1);
        let owner_id = Uuid::from_u128(2);
        let folder_id = Uuid::from_u128(3);
        let msg = InventoryDescendents {
            agent_data: inventory_descendents::AgentData {
                agent_id,
                folder_id,
                owner_id,
                version: 1,
                descendents: 1,
            },
            folder_data: vec![inventory_descendents::FolderData {
                folder_id,
                parent_id: Uuid::nil(),
                type_: folder_type::ROOT,
                name: b"Root".to_vec(),
            }],
            item_data: vec![],
        };
        let mut buf = Vec::new();
        build_packet(&msg, 1, false, false, &[], &mut buf);
        let pkt = parse_packet(&buf).expect("parse");
        assert_eq!(pkt.id, InventoryDescendents::ID);

        let bus = crate::events::EventBus::new(16);
        let mut rx = bus.subscribe();
        assert!(try_handle_inventory_descendents(&pkt, &bus));
        let ev = rx.try_recv().expect("event");
        assert!(matches!(ev, crate::events::Event::InventoryUpdated));
    }
}
