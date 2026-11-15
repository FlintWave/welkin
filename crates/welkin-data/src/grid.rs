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
        let sql =
            format!("SELECT {COLS} FROM regions WHERE ScopeID = $1 AND locX = $2 AND locY = $3");
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

    async fn query_flagged(&self, scope_id: &str, mask: i32) -> Result<Vec<RegionRow>, DataError> {
        let sql =
            format!("SELECT {COLS} FROM regions WHERE ScopeID = $1 AND (flags & {mask}) <> 0");
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(scope_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_region_by_uuid(
        &self,
        scope_id: &str,
        region_id: &str,
    ) -> Result<Option<RegionRow>, DataError> {
        if scope_id.is_empty() {
            return self.get_region(region_id).await;
        }
        let sql = format!("SELECT {COLS} FROM regions WHERE ScopeID = $1 AND uuid = $2");
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(scope_id)
            .bind(region_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Regions whose extents lie within one 256m cell of the given region.
    pub async fn get_neighbours(
        &self,
        scope_id: &str,
        region_id: &str,
    ) -> Result<Vec<RegionRow>, DataError> {
        let Some(r) = self.get_region_by_uuid(scope_id, region_id).await? else {
            return Ok(Vec::new());
        };
        let sql = format!(
            "SELECT {COLS} FROM regions WHERE ScopeID = $1 AND uuid <> $2 \
             AND locX + sizeX > $3 AND locX < $4 AND locY + sizeY > $5 AND locY < $6"
        );
        Ok(sqlx::query_as::<_, RegionRow>(&sql)
            .bind(r.scope_id.clone())
            .bind(r.uuid.clone())
            .bind(r.loc_x - 256)
            .bind(r.loc_x + r.size_x + 256)
            .bind(r.loc_y - 256)
            .bind(r.loc_y + r.size_y + 256)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Regions with the DefaultHypergridRegion flag (bit 2) set.
    pub async fn get_default_hypergrid_regions(
        &self,
        scope_id: &str,
    ) -> Result<Vec<RegionRow>, DataError> {
        self.query_flagged(scope_id, 4).await
    }

    /// Regions with the FallbackRegion flag (bit 1) set.
    pub async fn get_fallback_regions(&self, scope_id: &str) -> Result<Vec<RegionRow>, DataError> {
        self.query_flagged(scope_id, 2).await
    }

    /// Regions with the RegionOnline flag (bit 3) set, nearest to (x, y) first.
    pub async fn get_online_regions(
        &self,
        scope_id: &str,
        x: i32,
        y: i32,
        max_count: usize,
    ) -> Result<Vec<RegionRow>, DataError> {
        let mut rows = self.query_flagged(scope_id, 8).await?;
        let dist = |r: &RegionRow| {
            let dx = i64::from(r.loc_x - x);
            let dy = i64::from(r.loc_y - y);
            dx * dx + dy * dy
        };
        rows.sort_by_key(dist);
        rows.truncate(max_count);
        Ok(rows)
    }

    /// Regions with the Hyperlink flag (bit 4) set.
    pub async fn get_hyperlinks(&self, scope_id: &str) -> Result<Vec<RegionRow>, DataError> {
        self.query_flagged(scope_id, 16).await
    }

    pub async fn get_region_flags(
        &self,
        scope_id: &str,
        region_id: &str,
    ) -> Result<Option<i32>, DataError> {
        Ok(self
            .get_region_by_uuid(scope_id, region_id)
            .await?
            .map(|r| r.flags))
    }

    pub async fn deregister(&self, region_id: &str) -> Result<bool, DataError> {
        self.delete_region(region_id).await
    }
}
