// SPDX-License-Identifier: GPL-3.0-or-later

//! Asset storage (`assets` table).

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AssetRow {
    pub name: String,
    pub description: String,
    #[sqlx(rename = "assetType")]
    pub asset_type: i32,
    /// tinyint(1) flag, stored as i32 for Any-driver portability.
    pub local: i32,
    /// tinyint(1) flag, stored as i32 for Any-driver portability.
    pub temporary: i32,
    pub data: Vec<u8>,
    pub id: String,
    pub create_time: i32,
    pub access_time: i32,
    pub asset_flags: i32,
    #[sqlx(rename = "CreatorID")]
    pub creator_id: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AssetMetadataRow {
    pub name: String,
    pub description: String,
    #[sqlx(rename = "assetType")]
    pub asset_type: i32,
    pub local: i32,
    pub temporary: i32,
    pub id: String,
    pub create_time: i32,
    pub access_time: i32,
    pub asset_flags: i32,
    #[sqlx(rename = "CreatorID")]
    pub creator_id: String,
}

pub struct AssetStore {
    pool: sqlx::AnyPool,
}

impl AssetStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, id: &str) -> Result<Option<AssetRow>, DataError> {
        let row = sqlx::query_as::<_, AssetRow>(
            "SELECT name, description, assetType, local, temporary, data, id, \
             COALESCE(create_time, 0) AS create_time, COALESCE(access_time, 0) AS access_time, \
             asset_flags, CreatorID FROM assets WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_metadata(&self, id: &str) -> Result<Option<AssetMetadataRow>, DataError> {
        let row = sqlx::query_as::<_, AssetMetadataRow>(
            "SELECT name, description, assetType, local, temporary, id, \
             COALESCE(create_time, 0) AS create_time, COALESCE(access_time, 0) AS access_time, \
             asset_flags, CreatorID FROM assets WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Upsert via delete + insert in one transaction.
    pub async fn store(&self, asset: &AssetRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM assets WHERE id = $1")
            .bind(&asset.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO assets (name, description, assetType, local, temporary, data, id, \
             create_time, access_time, asset_flags, CreatorID) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(&asset.name)
        .bind(&asset.description)
        .bind(asset.asset_type)
        .bind(asset.local)
        .bind(asset.temporary)
        .bind(&asset.data)
        .bind(&asset.id)
        .bind(asset.create_time)
        .bind(asset.access_time)
        .bind(asset.asset_flags)
        .bind(&asset.creator_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn exists(&self, id: &str) -> Result<bool, DataError> {
        let row = sqlx::query("SELECT 1 FROM assets WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    pub async fn delete(&self, id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM assets WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn update_access_time(&self, id: &str, time: i32) -> Result<(), DataError> {
        sqlx::query("UPDATE assets SET access_time = $1 WHERE id = $2")
            .bind(time)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
