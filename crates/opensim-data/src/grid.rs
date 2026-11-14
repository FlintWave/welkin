// SPDX-License-Identifier: GPL-3.0-or-later

//! Grid region storage (`regions` table). Legacy columns are intentionally omitted.

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RegionRow {
    pub uuid: String,
    #[sqlx(rename = "regionHandle")]
    pub region_handle: i64,
    #[sqlx(rename = "regionName")]
    pub region_name: String,
    #[sqlx(rename = "regionSecret")]
    pub region_secret: String,
    #[sqlx(rename = "serverIP")]
    pub server_ip: String,
    #[sqlx(rename = "serverPort")]
    pub server_port: Option<i32>,
    #[sqlx(rename = "serverURI")]
    pub server_uri: String,
    #[sqlx(rename = "locX")]
    pub loc_x: i32,
    #[sqlx(rename = "locY")]
    pub loc_y: i32,
    #[sqlx(rename = "locZ")]
    pub loc_z: i32,
    #[sqlx(rename = "regionMapTexture")]
    pub region_map_texture: String,
    #[sqlx(rename = "serverHttpPort")]
    pub server_http_port: Option<i32>,
    pub owner_uuid: String,
    pub access: i32,
    #[sqlx(rename = "ScopeID")]
    pub scope_id: String,
    #[sqlx(rename = "sizeX")]
    pub size_x: i32,
    #[sqlx(rename = "sizeY")]
    pub size_y: i32,
    pub flags: i32,
    pub last_seen: i32,
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
    #[sqlx(rename = "Token")]
    pub token: String,
    #[sqlx(rename = "parcelMapTexture")]
    pub parcel_map_texture: String,
}

// Nullable columns are wrapped in COALESCE so they decode into plain values.
const COLS: &str = "uuid, regionHandle, COALESCE(regionName, '') AS regionName, \
    COALESCE(regionSecret, '') AS regionSecret, COALESCE(serverIP, '') AS serverIP, serverPort, \
    COALESCE(serverURI, '') AS serverURI, COALESCE(locX, 0) AS locX, COALESCE(locY, 0) AS locY, \
    COALESCE(locZ, 0) AS locZ, COALESCE(regionMapTexture, '') AS regionMapTexture, \
    serverHttpPort, owner_uuid, COALESCE(access, 1) AS access, ScopeID, sizeX, sizeY, flags, \
    last_seen, PrincipalID, Token, COALESCE(parcelMapTexture, '') AS parcelMapTexture";

pub struct GridStore {
    pool: sqlx::AnyPool,
}

impl GridStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get_region(&self, uuid: &str) -> Result<Option<RegionRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM regions WHERE uuid = $1");
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_region_by_name(&self, name: &str) -> Result<Option<RegionRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM regions WHERE regionName = $1");
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(name)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_region_by_position(
        &self,
        scope_id: &str,
        x: i32,
        y: i32,
    ) -> Result<Option<RegionRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM regions WHERE ScopeID = $1 AND locX = $2 AND locY = $3");
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(scope_id)
            .bind(x)
            .bind(y)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_regions_in_range(
        &self,
        scope_id: &str,
        x_min: i32,
        x_max: i32,
        y_min: i32,
        y_max: i32,
    ) -> Result<Vec<RegionRow>, DataError> {
        let sql = format!(
            "SELECT {COLS} FROM regions WHERE ScopeID = $1 AND locX BETWEEN $2 AND $3 \
             AND locY BETWEEN $4 AND $5"
        );
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(scope_id)
            .bind(x_min)
            .bind(x_max)
            .bind(y_min)
            .bind(y_max)
            .fetch_all(&self.pool)
            .await?)
    }

    /// `name_pattern` is passed to LIKE verbatim (caller supplies any `%`).
    pub async fn get_regions_by_name(
        &self,
        scope_id: &str,
        name_pattern: &str,
    ) -> Result<Vec<RegionRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM regions WHERE ScopeID = $1 AND regionName LIKE $2");
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(scope_id)
            .bind(name_pattern)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Regions with the DefaultRegion flag (bit 0) set.
    pub async fn get_default_regions(&self, scope_id: &str) -> Result<Vec<RegionRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM regions WHERE ScopeID = $1 AND (flags & 1) <> 0");
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(scope_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn store_region(&self, region: &RegionRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM regions WHERE uuid = $1")
            .bind(&region.uuid)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO regions (uuid, regionHandle, regionName, regionSecret, serverIP, \
             serverPort, serverURI, locX, locY, locZ, regionMapTexture, serverHttpPort, \
             owner_uuid, access, ScopeID, sizeX, sizeY, flags, last_seen, PrincipalID, Token, \
             parcelMapTexture) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
             $17, $18, $19, $20, $21, $22)",
        )
        .bind(&region.uuid)
        .bind(region.region_handle)
        .bind(&region.region_name)
        .bind(&region.region_secret)
        .bind(&region.server_ip)
        .bind(region.server_port)
        .bind(&region.server_uri)
        .bind(region.loc_x)
        .bind(region.loc_y)
        .bind(region.loc_z)
        .bind(&region.region_map_texture)
        .bind(region.server_http_port)
        .bind(&region.owner_uuid)
        .bind(region.access)
        .bind(&region.scope_id)
        .bind(region.size_x)
        .bind(region.size_y)
        .bind(region.flags)
        .bind(region.last_seen)
        .bind(&region.principal_id)
        .bind(&region.token)
        .bind(&region.parcel_map_texture)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_region(&self, uuid: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM regions WHERE uuid = $1")
            .bind(uuid)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }
}
