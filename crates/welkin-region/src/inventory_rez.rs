// SPDX-License-Identifier: LGPL-3.0-or-later

//! Turning inventory objects into scene objects: fetch the item, fetch its asset,
//! read the object XML. Shared by "rez from inventory" and attachments.

use glam::Vec3;
use uuid::Uuid;
use welkin_data::inventory::InventoryItemRow;
use welkin_msg::welkin_types::PermissionMask;
use welkin_scene::group::SceneObjectGroup;
use welkin_scene::xml2::deserialize_group;
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
        let _ = std::fs::remove_file(&path);
    }
}
