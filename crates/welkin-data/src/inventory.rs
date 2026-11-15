// SPDX-License-Identifier: LGPL-3.0-or-later

//! Inventory storage (`inventoryitems` and `inventoryfolders` tables).

use crate::DataError;

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryItemRow {
    #[sqlx(rename = "assetID")]
    pub asset_id: String,
    #[sqlx(rename = "assetType")]
    pub asset_type: i32,
    #[sqlx(rename = "inventoryName")]
    pub inventory_name: String,
    #[sqlx(rename = "inventoryDescription")]
    pub inventory_description: String,
    #[sqlx(rename = "inventoryNextPermissions")]
    pub next_permissions: i64,
    #[sqlx(rename = "inventoryCurrentPermissions")]
    pub current_permissions: i64,
    #[sqlx(rename = "invType")]
    pub inv_type: i32,
    #[sqlx(rename = "creatorID")]
    pub creator_id: String,
    #[sqlx(rename = "inventoryBasePermissions")]
    pub base_permissions: i64,
    #[sqlx(rename = "inventoryEveryOnePermissions")]
    pub everyone_permissions: i64,
    #[sqlx(rename = "salePrice")]
    pub sale_price: i32,
    #[sqlx(rename = "saleType")]
    pub sale_type: i32,
    #[sqlx(rename = "creationDate")]
    pub creation_date: i32,
    #[sqlx(rename = "groupID")]
    pub group_id: String,
    /// tinyint flag, stored as i32 for Any-driver portability.
    #[sqlx(rename = "groupOwned")]
    pub group_owned: i32,
    pub flags: i64,
    #[sqlx(rename = "inventoryID")]
    pub inventory_id: String,
    #[sqlx(rename = "avatarID")]
    pub avatar_id: String,
    #[sqlx(rename = "parentFolderID")]
    pub parent_folder_id: String,
    #[sqlx(rename = "inventoryGroupPermissions")]
    pub group_permissions: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InventoryFolderRow {
    #[sqlx(rename = "folderName")]
    pub folder_name: String,
    #[sqlx(rename = "type")]
    pub folder_type: i16,
    pub version: i32,
    #[sqlx(rename = "folderID")]
    pub folder_id: String,
    #[sqlx(rename = "agentID")]
    pub agent_id: String,
    #[sqlx(rename = "parentFolderID")]
    pub parent_folder_id: String,
}

// Nullable columns are wrapped in COALESCE so they decode into plain Strings.
const ITEM_COLS: &str = "COALESCE(assetID, '') AS assetID, COALESCE(assetType, 0) AS assetType, \
    COALESCE(inventoryName, '') AS inventoryName, \
    COALESCE(inventoryDescription, '') AS inventoryDescription, \
    COALESCE(inventoryNextPermissions, 0) AS inventoryNextPermissions, \
    COALESCE(inventoryCurrentPermissions, 0) AS inventoryCurrentPermissions, \
    COALESCE(invType, 0) AS invType, creatorID, inventoryBasePermissions, \
    inventoryEveryOnePermissions, salePrice, saleType, creationDate, groupID, groupOwned, \
    flags, inventoryID, COALESCE(avatarID, '') AS avatarID, \
    COALESCE(parentFolderID, '') AS parentFolderID, inventoryGroupPermissions";

const FOLDER_COLS: &str = "COALESCE(folderName, '') AS folderName, type, version, folderID, \
    COALESCE(agentID, '') AS agentID, COALESCE(parentFolderID, '') AS parentFolderID";

pub struct InventoryStore {
    pool: sqlx::AnyPool,
}

impl InventoryStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get_item(&self, id: &str) -> Result<Option<InventoryItemRow>, DataError> {
        let sql = format!("SELECT {ITEM_COLS} FROM inventoryitems WHERE inventoryID = $1");
        Ok(sqlx::query_as::<_, InventoryItemRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_folder(&self, id: &str) -> Result<Option<InventoryFolderRow>, DataError> {
        let sql = format!("SELECT {FOLDER_COLS} FROM inventoryfolders WHERE folderID = $1");
        Ok(sqlx::query_as::<_, InventoryFolderRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_items_in_folder(
        &self,
        folder_id: &str,
    ) -> Result<Vec<InventoryItemRow>, DataError> {
        let sql = format!("SELECT {ITEM_COLS} FROM inventoryitems WHERE parentFolderID = $1");
        Ok(sqlx::query_as::<_, InventoryItemRow>(&sql)
            .bind(folder_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_folders_in_folder(
        &self,
        parent_id: &str,
    ) -> Result<Vec<InventoryFolderRow>, DataError> {
        let sql = format!("SELECT {FOLDER_COLS} FROM inventoryfolders WHERE parentFolderID = $1");
        Ok(sqlx::query_as::<_, InventoryFolderRow>(&sql)
            .bind(parent_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_user_root_folder(
        &self,
        agent_id: &str,
    ) -> Result<Option<InventoryFolderRow>, DataError> {
        let sql = format!(
            "SELECT {FOLDER_COLS} FROM inventoryfolders WHERE agentID = $1 AND parentFolderID = $2"
        );
        Ok(sqlx::query_as::<_, InventoryFolderRow>(&sql)
            .bind(agent_id)
            .bind(ZERO_UUID)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_user_folders(
        &self,
        agent_id: &str,
    ) -> Result<Vec<InventoryFolderRow>, DataError> {
        let sql = format!("SELECT {FOLDER_COLS} FROM inventoryfolders WHERE agentID = $1");
        Ok(sqlx::query_as::<_, InventoryFolderRow>(&sql)
            .bind(agent_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn store_item(&self, item: &InventoryItemRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM inventoryitems WHERE inventoryID = $1")
            .bind(&item.inventory_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO inventoryitems (assetID, assetType, inventoryName, inventoryDescription, \
             inventoryNextPermissions, inventoryCurrentPermissions, invType, creatorID, \
             inventoryBasePermissions, inventoryEveryOnePermissions, salePrice, saleType, \
             creationDate, groupID, groupOwned, flags, inventoryID, avatarID, parentFolderID, \
             inventoryGroupPermissions) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
             $17, $18, $19, $20)",
        )
        .bind(&item.asset_id)
        .bind(item.asset_type)
        .bind(&item.inventory_name)
        .bind(&item.inventory_description)
        .bind(item.next_permissions)
        .bind(item.current_permissions)
        .bind(item.inv_type)
        .bind(&item.creator_id)
        .bind(item.base_permissions)
        .bind(item.everyone_permissions)
        .bind(item.sale_price)
        .bind(item.sale_type)
        .bind(item.creation_date)
        .bind(&item.group_id)
        .bind(item.group_owned)
        .bind(item.flags)
        .bind(&item.inventory_id)
        .bind(&item.avatar_id)
        .bind(&item.parent_folder_id)
        .bind(item.group_permissions)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn store_folder(&self, folder: &InventoryFolderRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM inventoryfolders WHERE folderID = $1")
            .bind(&folder.folder_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO inventoryfolders (folderName, type, version, folderID, agentID, \
             parentFolderID) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&folder.folder_name)
        .bind(folder.folder_type)
        .bind(folder.version)
        .bind(&folder.folder_id)
        .bind(&folder.agent_id)
        .bind(&folder.parent_folder_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_item(&self, id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM inventoryitems WHERE inventoryID = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn delete_folder(&self, id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM inventoryfolders WHERE folderID = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn increment_folder_version(&self, folder_id: &str) -> Result<(), DataError> {
        sqlx::query("UPDATE inventoryfolders SET version = version + 1 WHERE folderID = $1")
            .bind(folder_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete all items and subfolders under `folder_id`, recursively. The
    /// folder itself is kept (matching OpenSim's PurgeFolder semantics).
    pub async fn purge_folder(&self, folder_id: &str) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        // Breadth-first collect of all descendant folder ids.
        let mut descendants: Vec<String> = Vec::new();
        let mut frontier: Vec<String> = vec![folder_id.to_string()];
        while let Some(parent) = frontier.pop() {
            let rows: Vec<(String,)> =
                sqlx::query_as("SELECT folderID FROM inventoryfolders WHERE parentFolderID = $1")
                    .bind(&parent)
                    .fetch_all(&mut *tx)
                    .await?;
            for (id,) in rows {
                // Guard against cycles.
                if id != folder_id && !descendants.contains(&id) {
                    descendants.push(id.clone());
                    frontier.push(id);
                }
            }
        }
        for id in descendants
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(folder_id))
        {
            sqlx::query("DELETE FROM inventoryitems WHERE parentFolderID = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        for id in &descendants {
            sqlx::query("DELETE FROM inventoryfolders WHERE folderID = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_folder_for_type(
        &self,
        agent_id: &str,
        folder_type: i16,
    ) -> Result<Option<InventoryFolderRow>, DataError> {
        let sql =
            format!("SELECT {FOLDER_COLS} FROM inventoryfolders WHERE agentID = $1 AND type = $2");
        Ok(sqlx::query_as::<_, InventoryFolderRow>(&sql)
            .bind(agent_id)
            .bind(folder_type)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn move_folder(&self, folder_id: &str, parent_id: &str) -> Result<bool, DataError> {
        let res =
            sqlx::query("UPDATE inventoryfolders SET parentFolderID = $1 WHERE folderID = $2")
                .bind(parent_id)
                .bind(folder_id)
                .execute(&self.pool)
                .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn delete_folders(&self, folder_ids: &[&str]) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        for id in folder_ids {
            sqlx::query("DELETE FROM inventoryfolders WHERE folderID = $1")
                .bind(*id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_items(&self, item_ids: &[&str]) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        for id in item_ids {
            sqlx::query("DELETE FROM inventoryitems WHERE inventoryID = $1")
                .bind(*id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// `item_ids` and `dest_ids` are parallel arrays; extra entries are ignored.
    pub async fn move_items(&self, item_ids: &[&str], dest_ids: &[&str]) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        for (id, dest) in item_ids.iter().zip(dest_ids.iter()) {
            sqlx::query("UPDATE inventoryitems SET parentFolderID = $1 WHERE inventoryID = $2")
                .bind(*dest)
                .bind(*id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_active_gestures(
        &self,
        agent_id: &str,
    ) -> Result<Vec<InventoryItemRow>, DataError> {
        let sql = format!(
            "SELECT {ITEM_COLS} FROM inventoryitems WHERE avatarID = $1 AND invType = 21 \
             AND (flags & 1) = 1"
        );
        Ok(sqlx::query_as::<_, InventoryItemRow>(&sql)
            .bind(agent_id)
            .fetch_all(&self.pool)
            .await?)
    }

    /// OR of `inventoryCurrentPermissions` over matching items, as OpenSim does.
    pub async fn get_asset_permissions(
        &self,
        principal_id: &str,
        asset_id: &str,
    ) -> Result<i32, DataError> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT COALESCE(inventoryCurrentPermissions, 0) FROM inventoryitems \
             WHERE avatarID = $1 AND assetID = $2",
        )
        .bind(principal_id)
        .bind(asset_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().fold(0i64, |acc, (p,)| acc | p) as i32)
    }

    pub async fn get_multiple_items(
        &self,
        item_ids: &[&str],
    ) -> Result<Vec<Option<InventoryItemRow>>, DataError> {
        let mut out = Vec::with_capacity(item_ids.len());
        for id in item_ids {
            out.push(self.get_item(id).await?);
        }
        Ok(out)
    }

    pub async fn create_user_inventory(&self, principal_id: &str) -> Result<(), DataError> {
        if self.get_user_root_folder(principal_id).await?.is_some() {
            return Ok(());
        }
        const SKELETON: &[(&str, i16)] = &[
            ("Animations", 20),
            ("Body Parts", 13),
            ("Calling Cards", 2),
            ("Clothing", 5),
            ("Current Outfit", 46),
            ("Favorites", 23),
            ("Gestures", 21),
            ("Landmarks", 3),
            ("Lost And Found", 16),
            ("My Outfits", 48),
            ("Notecards", 7),
            ("Objects", 6),
            ("Scripts", 10),
            ("Sounds", 1),
            ("Textures", 0),
            ("Trash", 14),
        ];
        let root_id = uuid::Uuid::new_v4().to_string();
        self.store_folder(&InventoryFolderRow {
            folder_name: "My Inventory".to_string(),
            folder_type: -1,
            version: 1,
            folder_id: root_id.clone(),
            agent_id: principal_id.to_string(),
            parent_folder_id: ZERO_UUID.to_string(),
        })
        .await?;
        for (name, ty) in SKELETON {
            self.store_folder(&InventoryFolderRow {
                folder_name: (*name).to_string(),
                folder_type: *ty,
                version: 1,
                folder_id: uuid::Uuid::new_v4().to_string(),
                agent_id: principal_id.to_string(),
                parent_folder_id: root_id.clone(),
            })
            .await?;
        }
        Ok(())
    }
}
