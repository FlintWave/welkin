// SPDX-License-Identifier: LGPL-3.0-or-later

//! Test helpers: the real inventory and asset services on a throwaway database.

use std::sync::Arc;

use uuid::Uuid;
use welkin_data::assets::{AssetRow, AssetStore};
use welkin_data::inventory::{InventoryItemRow, InventoryStore};
use welkin_scene::group::SceneObjectGroup;
use welkin_scene::part::SceneObjectPart;
use welkin_scene::xml2::serialize_group;

pub struct Services {
    pub base: String,
    pub pool: welkin_data::DataPool,
    path: std::path::PathBuf,
}

impl Drop for Services {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Start the inventory and asset services (HTTP, ephemeral port) over a fresh sqlite file.
pub async fn spawn_services(tag: &str) -> Services {
    sqlx::any::install_default_drivers();
    let path = std::env::temp_dir().join(format!("welkin-svc-{tag}-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let pool = welkin_data::DataPool::connect(&format!("sqlite:{}?mode=rwc", path.display()))
        .await
        .expect("pool");
    let backend = welkin_data::migrations::Backend::SQLite;
    let runner = welkin_data::migrations::MigrationRunner::new(pool.inner().clone(), backend);
    runner.ensure_migrations_table().await.expect("migrations table");
    let sql = welkin_data::migrations::migration_content("AssetStore", backend).unwrap();
    runner.run_migrations("AssetStore", sql).await.expect("asset migrations");
    for ddl in [
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
        "CREATE TABLE inventoryfolders(\
         folderName varchar(64), type integer, version integer, \
         folderID varchar(36) primary key, \
         agentID varchar(36) not null default '00000000-0000-0000-0000-000000000000', \
         parentFolderID varchar(36) not null default '00000000-0000-0000-0000-000000000000')",
    ] {
        sqlx::query(ddl).execute(pool.inner()).await.expect("inventory tables");
    }
    let app = welkin_services::inventory::router(welkin_services::inventory::InventoryServiceState {
        store: Arc::new(InventoryStore::new(pool.inner().clone())),
    })
    .merge(welkin_services::assets::router(welkin_services::assets::AssetServiceState::new(
        AssetStore::new(pool.inner().clone()),
        welkin_services::assets::DeleteAllowed::None,
    )));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    Services { base, pool, path }
}

/// Store a two-prim object as an asset and an inventory item for `owner` pointing at it.
/// Returns the item.
pub async fn add_object_item(svc: &Services, owner: Uuid, name: &str) -> InventoryItemRow {
    let mut root = SceneObjectPart::new(name, owner);
    root.creator_id = owner;
    let mut g = SceneObjectGroup::new(root);
    g.add_part(SceneObjectPart::new("Part", owner));
    let asset_id = Uuid::new_v4();
    AssetStore::new(svc.pool.inner().clone())
        .store(&AssetRow {
            name: name.into(),
            description: String::new(),
            asset_type: 6,
            local: 0,
            temporary: 0,
            data: serialize_group(&g).unwrap().into_bytes(),
            id: asset_id.to_string(),
            create_time: 0,
            access_time: 0,
            asset_flags: 0,
            creator_id: owner.to_string(),
        })
        .await
        .expect("store asset");
    let item = InventoryItemRow {
        asset_id: asset_id.to_string(),
        asset_type: 6,
        inventory_name: name.into(),
        inventory_description: String::new(),
        next_permissions: 0x0008e000,
        current_permissions: 0x0008e000,
        inv_type: 6,
        creator_id: owner.to_string(),
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
    };
    InventoryStore::new(svc.pool.inner().clone()).store_item(&item).await.expect("store item");
    item
}
