// SPDX-License-Identifier: LGPL-3.0-or-later

//! Turning inventory objects into scene objects: fetch the item, fetch its asset,
//! read the object XML. Shared by "rez from inventory" and attachments.

use glam::Vec3;
use uuid::Uuid;
use welkin_data::assets::AssetRow;
use welkin_data::inventory::InventoryItemRow;
use welkin_msg::welkin_types::PermissionMask;
use welkin_scene::group::SceneObjectGroup;
use welkin_scene::xml2::{deserialize_group, serialize_group};
use welkin_services::remote::asset::RemoteAssetConnector;
use welkin_services::remote::inventory::RemoteInventoryConnector;

/// Inventory/asset type code of an object (`AssetType.Object`).
const ASSET_TYPE_OBJECT: i32 = 6;

/// Build a scene object from an object asset's bytes. The result has fresh UUIDs, is
/// owned by `agent`, takes its permissions from the inventory `item`, and sits at
/// `position`.
pub fn group_from_asset(
    item: &InventoryItemRow,
    data: &[u8],
    agent: Uuid,
    position: Vec3,
) -> Result<SceneObjectGroup, String> {
    let text = std::str::from_utf8(data).map_err(|_| "object asset is not valid UTF-8 XML".to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    if text.contains("<CoalescedObject") {
        return Err("coalesced objects are not supported yet".to_string());
    }
    let mut group = deserialize_group(text).map_err(|e| format!("object asset XML: {e}"))?;
    group.reset_ids();
    group.set_position(position);

    let creator = Uuid::parse_str(item.creator_id.trim()).unwrap_or(Uuid::nil());
    for part in group.parts_mut() {
        part.owner_id = agent;
        if part.creator_id.is_nil() {
            part.creator_id = creator;
        }
    }
    let root = group.root_part_mut();
    root.base_mask = PermissionMask::from_bits_truncate(item.base_permissions as u32);
    root.owner_mask = PermissionMask::from_bits_truncate(item.current_permissions as u32);
    root.group_mask = PermissionMask::from_bits_truncate(item.group_permissions as u32);
    root.everyone_mask = PermissionMask::from_bits_truncate(item.everyone_permissions as u32);
    root.next_owner_mask = PermissionMask::from_bits_truncate(item.next_permissions as u32);
    Ok(group)
}

/// Fetch an object from an agent's inventory and build it as a scene object.
/// Returns the inventory item as well (callers need its name, flags and asset).
pub async fn rez_from_inventory(
    inventory: &RemoteInventoryConnector,
    assets: &RemoteAssetConnector,
    agent: Uuid,
    item_id: Uuid,
    position: Vec3,
) -> Result<(InventoryItemRow, SceneObjectGroup), String> {
    let item = inventory
        .get_item(&item_id.to_string())
        .await
        .map_err(|e| format!("inventory service: {e}"))?
        .ok_or_else(|| "inventory item not found".to_string())?;
    if Uuid::parse_str(item.avatar_id.trim()).ok() != Some(agent) {
        return Err("inventory item belongs to another avatar".to_string());
    }
    if item.asset_type != ASSET_TYPE_OBJECT {
        return Err(format!("inventory item is not an object (asset type {})", item.asset_type));
    }
    let asset = assets
        .get_asset(&item.asset_id)
        .await
        .map_err(|e| format!("asset service: {e}"))?
        .ok_or_else(|| "the item's asset was not found".to_string())?;
    let group = group_from_asset(&item, &asset.data, agent, position)?;
    Ok((item, group))
}

/// `FolderType` codes of the standard inventory folders.
const FOLDER_OBJECTS: i32 = 6;
const FOLDER_TRASH: i32 = 14;
const FOLDER_LOST_AND_FOUND: i32 = 16;

/// What a take/delete/return does with the object's copy in inventory (OpenSim `DeRezAction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeRezAction {
    TakeCopy,
    Take,
    Delete,
    Return,
}

impl DeRezAction {
    /// From the viewer's `DeRezObject.AgentBlock.Destination` code.
    pub fn from_destination(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::TakeCopy),
            4 => Some(Self::Take),
            6 => Some(Self::Delete),
            9 => Some(Self::Return),
            _ => None,
        }
    }
}

/// Permission bits an inventory item may carry (OpenSim's AddPermissions mask).
fn effective_mask() -> u32 {
    (PermissionMask::COPY | PermissionMask::TRANSFER | PermissionMask::MODIFY | PermissionMask::MOVE).bits()
}

/// Inventory permissions for a copy of `group` going to `taker` (OpenSim `AddPermissions`):
/// the owner keeps the object's permissions; anyone else gets them cut down to the
/// next-owner permissions.
pub fn item_permissions(group: &SceneObjectGroup, taker: Uuid) -> (u32, u32, u32, u32, u32) {
    let root = group.root_part();
    let effective = root.owner_mask.bits() & effective_mask();
    if taker != root.owner_id {
        let base = effective & root.next_owner_mask.bits();
        (base, base, base & root.next_owner_mask.bits(), base & root.everyone_mask.bits(), base & root.group_mask.bits())
    } else {
        (
            effective,
            effective,
            root.next_owner_mask.bits() & effective,
            root.everyone_mask.bits() & effective,
            root.group_mask.bits() & effective,
        )
    }
}

/// Build the inventory item for `group`, placed in `folder`, to be owned by `owner`.
pub fn item_for_group(
    group: &SceneObjectGroup,
    asset_id: Uuid,
    owner: Uuid,
    taker: Uuid,
    folder: Uuid,
    now: i32,
) -> InventoryItemRow {
    let root = group.root_part();
    let (base, current, next, everyone, group_perm) = item_permissions(group, taker);
    InventoryItemRow {
        asset_id: asset_id.to_string(),
        asset_type: ASSET_TYPE_OBJECT,
        inventory_name: root.name.clone(),
        inventory_description: root.description.clone(),
        next_permissions: next as i64,
        current_permissions: current as i64,
        inv_type: 6,
        creator_id: root.creator_id.to_string(),
        base_permissions: base as i64,
        everyone_permissions: everyone as i64,
        sale_price: root.sale_price,
        sale_type: i32::from(root.object_sale_type),
        creation_date: now,
        group_id: Uuid::nil().to_string(),
        group_owned: 0,
        flags: 0,
        inventory_id: Uuid::new_v4().to_string(),
        avatar_id: owner.to_string(),
        parent_folder_id: folder.to_string(),
        group_permissions: group_perm as i64,
    }
}

/// Save `group` into an avatar's inventory as an object item: store the object XML as an
/// asset, find the destination folder, add the item. Nothing is removed from the scene
/// here; callers remove the object only after this succeeds.
///
/// * `taker` is the avatar who asked (it receives Take/TakeCopy items).
/// * `folder_hint` is the viewer's chosen folder (nil = default for the action).
pub async fn save_group_to_inventory(
    inventory: &RemoteInventoryConnector,
    assets: &RemoteAssetConnector,
    action: DeRezAction,
    taker: Uuid,
    folder_hint: Uuid,
    group: &SceneObjectGroup,
) -> Result<InventoryItemRow, String> {
    let object_owner = group.root_part().owner_id;
    // Take and TakeCopy go to the taker; return and delete go to the object's owner.
    let owner = match action {
        DeRezAction::Take | DeRezAction::TakeCopy => taker,
        DeRezAction::Delete | DeRezAction::Return => object_owner,
    };
    if owner.is_nil() {
        return Err("no inventory owner for this object".to_string());
    }

    let folder = if !folder_hint.is_nil() && matches!(action, DeRezAction::Take | DeRezAction::TakeCopy) {
        folder_hint
    } else {
        let folder_type = match action {
            DeRezAction::Return => FOLDER_LOST_AND_FOUND,
            DeRezAction::Delete if taker == object_owner => FOLDER_TRASH,
            DeRezAction::Delete => FOLDER_LOST_AND_FOUND,
            _ => FOLDER_OBJECTS,
        };
        let found = inventory
            .get_folder_for_type(&owner.to_string(), folder_type)
            .await
            .map_err(|e| format!("inventory service: {e}"))?
            .ok_or_else(|| format!("inventory folder type {folder_type} not found"))?;
        Uuid::parse_str(found.folder_id.trim()).map_err(|_| "inventory folder id is invalid".to_string())?
    };

    let xml = serialize_group(group).map_err(|e| format!("serialising the object: {e}"))?;
    let asset_id = Uuid::new_v4();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i32)
        .unwrap_or(0);
    assets
        .store_asset(&AssetRow {
            name: group.root_part().name.clone(),
            description: group.root_part().description.clone(),
            asset_type: ASSET_TYPE_OBJECT,
            local: 0,
            temporary: 0,
            data: xml.into_bytes(),
            id: asset_id.to_string(),
            create_time: now,
            access_time: now,
            asset_flags: 0,
            creator_id: group.root_part().creator_id.to_string(),
        })
        .await
        .map_err(|e| format!("asset service: {e}"))?;

    let item = item_for_group(group, asset_id, owner, taker, folder, now);
    if !inventory.add_item(&item).await.map_err(|e| format!("inventory service: {e}"))? {
        return Err("the inventory service refused the new item".to_string());
    }
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_scene::part::SceneObjectPart;
    use welkin_scene::xml2::serialize_group;

    fn item(owner: Uuid) -> InventoryItemRow {
        InventoryItemRow {
            asset_id: Uuid::new_v4().to_string(),
            asset_type: 6,
            inventory_name: "Hat".into(),
            inventory_description: String::new(),
            next_permissions: 0x0008e000,
            current_permissions: 0x0008e000,
            inv_type: 6,
            creator_id: Uuid::new_v4().to_string(),
            base_permissions: 0x7fffffff,
            everyone_permissions: 0,
            sale_price: 0,
            sale_type: 0,
            creation_date: 0,
            group_id: Uuid::nil().to_string(),
            group_owned: 0,
            flags: 0,
            inventory_id: Uuid::new_v4().to_string(),
            avatar_id: owner.to_string(),
            parent_folder_id: Uuid::new_v4().to_string(),
            group_permissions: 0,
        }
    }

    fn two_prim_xml(original_owner: Uuid) -> (String, Uuid) {
        let root = SceneObjectPart::new("Hat", original_owner);
        let root_id = root.uuid;
        let mut g = SceneObjectGroup::new(root);
        g.add_part(SceneObjectPart::new("Brim", original_owner));
        (serialize_group(&g).unwrap(), root_id)
    }

    #[test]
    fn builds_an_owned_copy_with_new_ids_at_the_position() {
        let agent = Uuid::new_v4();
        let (xml, original_root) = two_prim_xml(Uuid::new_v4());
        let g = group_from_asset(&item(agent), xml.as_bytes(), agent, Vec3::new(10.0, 20.0, 30.0)).unwrap();
        assert_eq!(g.part_count(), 2);
        assert_ne!(g.uuid(), original_root);
        assert!(g.parts().all(|p| p.owner_id == agent));
        assert_eq!(g.position(), Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(g.root_part().next_owner_mask.bits(), 0x0008e000);
    }

    #[test]
    fn two_rezzes_do_not_share_identity() {
        let agent = Uuid::new_v4();
        let (xml, _) = two_prim_xml(agent);
        let a = group_from_asset(&item(agent), xml.as_bytes(), agent, Vec3::ZERO).unwrap();
        let b = group_from_asset(&item(agent), xml.as_bytes(), agent, Vec3::ZERO).unwrap();
        assert_ne!(a.uuid(), b.uuid());
    }

    #[test]
    fn rejects_garbage_and_coalesced_objects() {
        let agent = Uuid::new_v4();
        assert!(group_from_asset(&item(agent), b"not xml at all", agent, Vec3::ZERO).is_err());
        assert!(group_from_asset(&item(agent), &[0xff, 0xfe, 0x00], agent, Vec3::ZERO).is_err());
        let err = group_from_asset(&item(agent), b"<CoalescedObject><x/></CoalescedObject>", agent, Vec3::ZERO).unwrap_err();
        assert!(err.contains("coalesced"));
    }

    /// Real services over HTTP: an object stored in the asset service and an inventory
    /// item pointing at it rez into a scene object through the remote connectors.
    #[tokio::test]
    async fn rezzes_an_object_through_the_real_inventory_and_asset_services() {
        use std::sync::Arc;
        use welkin_data::assets::{AssetRow, AssetStore};
        use welkin_data::inventory::InventoryStore;

        sqlx::any::install_default_drivers();
        let path = std::env::temp_dir().join(format!("welkin-rez-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let pool = welkin_data::DataPool::connect(&format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .expect("pool");
        let backend = welkin_data::migrations::Backend::SQLite;
        let runner = welkin_data::migrations::MigrationRunner::new(pool.inner().clone(), backend);
        runner.ensure_migrations_table().await.expect("migrations table");
        let sql = welkin_data::migrations::migration_content("AssetStore", backend).unwrap();
        runner.run_migrations("AssetStore", sql).await.expect("asset migrations");
        // The inventory migrations assume tables the grid server creates first; make the
        // one table this test needs (same DDL as the grid server).
        sqlx::query(
            "CREATE TABLE inventoryitems(\
             assetID varchar(36), assetType integer, inventoryName varchar(64), \
             inventoryDescription varchar(128), inventoryNextPermissions integer, \
             inventoryCurrentPermissions integer, invType integer, creatorID varchar(128), \
             inventoryBasePermissions integer, inventoryEveryOnePermissions integer, \
             salePrice integer default 99, saleType integer default 0, \
             creationDate integer default 2000, \
             groupID varchar(36) default '00000000-0000-0000-0000-000000000000', \
             groupOwned integer default 0, flags integer default 0, \
             inventoryID varchar(36) primary key, \
             parentFolderID varchar(36) not null default '00000000-0000-0000-0000-000000000000', \
             avatarID varchar(36) not null default '00000000-0000-0000-0000-000000000000', \
             inventoryGroupPermissions integer not null default 0)",
        )
        .execute(pool.inner())
        .await
        .expect("inventoryitems table");
        sqlx::query(
            "CREATE TABLE inventoryfolders(\
             folderName varchar(64), type integer, version integer, \
             folderID varchar(36) primary key, \
             agentID varchar(36) not null default '00000000-0000-0000-0000-000000000000', \
             parentFolderID varchar(36) not null default '00000000-0000-0000-0000-000000000000')",
        )
        .execute(pool.inner())
        .await
        .expect("inventoryfolders table");

        let agent = Uuid::new_v4();
        let (xml, _) = two_prim_xml(agent);
        let asset_id = Uuid::new_v4();
        let asset_store = AssetStore::new(pool.inner().clone());
        asset_store
            .store(&AssetRow {
                name: "Hat".into(),
                description: String::new(),
                asset_type: 6,
                local: 0,
                temporary: 0,
                data: xml.into_bytes(),
                id: asset_id.to_string(),
                create_time: 0,
                access_time: 0,
                asset_flags: 0,
                creator_id: agent.to_string(),
            })
            .await
            .expect("store asset");
        let inv_store = Arc::new(InventoryStore::new(pool.inner().clone()));
        let mut it = item(agent);
        it.asset_id = asset_id.to_string();
        inv_store.store_item(&it).await.expect("store item");

        let app = welkin_services::inventory::router(welkin_services::inventory::InventoryServiceState {
            store: inv_store,
        })
        .merge(welkin_services::assets::router(welkin_services::assets::AssetServiceState::new(
            asset_store,
            welkin_services::assets::DeleteAllowed::None,
        )));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let inv = RemoteInventoryConnector::new(&base);
        let assets = RemoteAssetConnector::new(&base);
        let item_id = Uuid::parse_str(&it.inventory_id).unwrap();
        let pos = Vec3::new(1.0, 2.0, 3.0);

        let (got_item, group) = rez_from_inventory(&inv, &assets, agent, item_id, pos).await.expect("rez");
        assert_eq!(got_item.inventory_name, "Hat");
        assert_eq!(group.part_count(), 2);
        assert!(group.parts().all(|p| p.owner_id == agent));

        // Someone else's item, a missing item and a non-object are refused.
        let other = Uuid::new_v4();
        assert!(rez_from_inventory(&inv, &assets, other, item_id, pos).await.unwrap_err().contains("another avatar"));
        assert!(rez_from_inventory(&inv, &assets, agent, Uuid::new_v4(), pos).await.unwrap_err().contains("not found"));

        // Take it back into inventory (to the Objects folder), then rez the new item.
        let objects_folder = Uuid::new_v4();
        let trash_folder = Uuid::new_v4();
        let lost_folder = Uuid::new_v4();
        for (id, kind) in [(objects_folder, 6), (trash_folder, 14), (lost_folder, 16)] {
            sqlx::query("INSERT INTO inventoryfolders (folderName,type,version,folderID,agentID,parentFolderID) VALUES ('f',$1,1,$2,$3,$4)")
                .bind(kind)
                .bind(id.to_string())
                .bind(agent.to_string())
                .bind(Uuid::nil().to_string())
                .execute(pool.inner())
                .await
                .unwrap();
        }
        let taken = save_group_to_inventory(&inv, &assets, DeRezAction::Take, agent, Uuid::nil(), &group)
            .await
            .expect("take");
        assert_eq!(taken.parent_folder_id, objects_folder.to_string());
        assert_eq!(taken.avatar_id, agent.to_string());
        assert_eq!(taken.inventory_name, "Hat");
        let (_, again) = rez_from_inventory(&inv, &assets, agent, Uuid::parse_str(&taken.inventory_id).unwrap(), pos)
            .await
            .expect("rez the taken item");
        assert_eq!(again.part_count(), 2);
        assert_ne!(again.uuid(), group.uuid());

        // Deleting your own object goes to the trash; returning goes to lost and found.
        let deleted = save_group_to_inventory(&inv, &assets, DeRezAction::Delete, agent, Uuid::nil(), &group).await.expect("delete");
        assert_eq!(deleted.parent_folder_id, trash_folder.to_string());
        let returned = save_group_to_inventory(&inv, &assets, DeRezAction::Return, other, Uuid::nil(), &group).await.expect("return");
        assert_eq!(returned.parent_folder_id, lost_folder.to_string());
        assert_eq!(returned.avatar_id, agent.to_string(), "returned objects go to their owner");
        // A viewer-chosen folder is used for Take.
        let chosen = Uuid::new_v4();
        let to_chosen = save_group_to_inventory(&inv, &assets, DeRezAction::Take, agent, chosen, &group).await.expect("take to folder");
        assert_eq!(to_chosen.parent_folder_id, chosen.to_string());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn item_permissions_follow_opensim_add_permissions() {
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mut root = SceneObjectPart::new("Box", owner);
        root.owner_mask = PermissionMask::from_bits_truncate(0x7fffffff);
        root.next_owner_mask = PermissionMask::MODIFY | PermissionMask::MOVE;
        root.everyone_mask = PermissionMask::MOVE;
        let g = SceneObjectGroup::new(root);
        let all = effective_mask();
        let (base, cur, next, every, _) = item_permissions(&g, owner);
        assert_eq!((base, cur), (all, all));
        assert_eq!(next, (PermissionMask::MODIFY | PermissionMask::MOVE).bits());
        assert_eq!(every, PermissionMask::MOVE.bits());
        // Someone else taking it only gets what the next owner may have.
        let (base, cur, _, _, _) = item_permissions(&g, other);
        assert_eq!(base, (PermissionMask::MODIFY | PermissionMask::MOVE).bits());
        assert_eq!(base, cur);
    }

    #[test]
    fn derez_destination_codes_match_opensim() {
        assert_eq!(DeRezAction::from_destination(1), Some(DeRezAction::TakeCopy));
        assert_eq!(DeRezAction::from_destination(4), Some(DeRezAction::Take));
        assert_eq!(DeRezAction::from_destination(6), Some(DeRezAction::Delete));
        assert_eq!(DeRezAction::from_destination(9), Some(DeRezAction::Return));
        assert_eq!(DeRezAction::from_destination(3), None);
    }
}
