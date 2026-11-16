// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EstateSettingsRow {
    #[sqlx(rename = "EstateID")]
    pub estate_id: i32,
    #[sqlx(rename = "EstateName")]
    pub estate_name: Option<String>,
    #[sqlx(rename = "AbuseEmailToEstateOwner")]
    pub abuse_email_to_estate_owner: i32,
    #[sqlx(rename = "DenyAnonymous")]
    pub deny_anonymous: i32,
    #[sqlx(rename = "ResetHomeOnTeleport")]
    pub reset_home_on_teleport: i32,
    #[sqlx(rename = "FixedSun")]
    pub fixed_sun: i32,
    #[sqlx(rename = "DenyTransacted")]
    pub deny_transacted: i32,
    #[sqlx(rename = "BlockDwell")]
    pub block_dwell: i32,
    #[sqlx(rename = "DenyIdentified")]
    pub deny_identified: i32,
    #[sqlx(rename = "AllowVoice")]
    pub allow_voice: i32,
    #[sqlx(rename = "UseGlobalTime")]
    pub use_global_time: i32,
    #[sqlx(rename = "PricePerMeter")]
    pub price_per_meter: i32,
    #[sqlx(rename = "TaxFree")]
    pub tax_free: i32,
    #[sqlx(rename = "AllowDirectTeleport")]
    pub allow_direct_teleport: i32,
    #[sqlx(rename = "RedirectGridX")]
    pub redirect_grid_x: i32,
    #[sqlx(rename = "RedirectGridY")]
    pub redirect_grid_y: i32,
    #[sqlx(rename = "ParentEstateID")]
    pub parent_estate_id: i32,
    #[sqlx(rename = "SunPosition")]
    pub sun_position: f64,
    #[sqlx(rename = "EstateSkipScripts")]
    pub estate_skip_scripts: i32,
    #[sqlx(rename = "BillableFactor")]
    pub billable_factor: f64,
    #[sqlx(rename = "PublicAccess")]
    pub public_access: i32,
    #[sqlx(rename = "AbuseEmail")]
    pub abuse_email: String,
    #[sqlx(rename = "EstateOwner")]
    pub estate_owner: String,
    #[sqlx(rename = "DenyMinors")]
    pub deny_minors: i32,
    #[sqlx(rename = "AllowLandmark")]
    pub allow_landmark: i32,
    #[sqlx(rename = "AllowParcelChanges")]
    pub allow_parcel_changes: i32,
    #[sqlx(rename = "AllowSetHome")]
    pub allow_set_home: i32,
    #[sqlx(rename = "AllowEnviromentOverride")]
    pub allow_environment_override: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EstateBanRow {
    #[sqlx(rename = "EstateID")]
    pub estate_id: i32,
    #[sqlx(rename = "bannedUUID")]
    pub banned_uuid: String,
    #[sqlx(rename = "bannedIp")]
    pub banned_ip: String,
    #[sqlx(rename = "bannedIpHostMask")]
    pub banned_ip_host_mask: String,
    #[sqlx(rename = "bannedNameMask")]
    pub banned_name_mask: Option<String>,
    #[sqlx(rename = "banningUUID")]
    pub banning_uuid: String,
    #[sqlx(rename = "banTime")]
    pub ban_time: i32,
}

const ESTATE_COLS: &str = "EstateID, EstateName, AbuseEmailToEstateOwner, DenyAnonymous, ResetHomeOnTeleport, FixedSun, DenyTransacted, BlockDwell, DenyIdentified, AllowVoice, UseGlobalTime, PricePerMeter, TaxFree, AllowDirectTeleport, RedirectGridX, RedirectGridY, ParentEstateID, SunPosition, EstateSkipScripts, BillableFactor, PublicAccess, AbuseEmail, EstateOwner, DenyMinors, AllowLandmark, AllowParcelChanges, AllowSetHome, AllowEnviromentOverride";
const ESTATE_COLS_E: &str = "e.EstateID, e.EstateName, e.AbuseEmailToEstateOwner, e.DenyAnonymous, e.ResetHomeOnTeleport, e.FixedSun, e.DenyTransacted, e.BlockDwell, e.DenyIdentified, e.AllowVoice, e.UseGlobalTime, e.PricePerMeter, e.TaxFree, e.AllowDirectTeleport, e.RedirectGridX, e.RedirectGridY, e.ParentEstateID, e.SunPosition, e.EstateSkipScripts, e.BillableFactor, e.PublicAccess, e.AbuseEmail, e.EstateOwner, e.DenyMinors, e.AllowLandmark, e.AllowParcelChanges, e.AllowSetHome, e.AllowEnviromentOverride";

const ESTATE_UPDATE: &str = "UPDATE estate_settings SET EstateName = $1, AbuseEmailToEstateOwner = $2, DenyAnonymous = $3, ResetHomeOnTeleport = $4, FixedSun = $5, DenyTransacted = $6, BlockDwell = $7, DenyIdentified = $8, AllowVoice = $9, UseGlobalTime = $10, PricePerMeter = $11, TaxFree = $12, AllowDirectTeleport = $13, RedirectGridX = $14, RedirectGridY = $15, ParentEstateID = $16, SunPosition = $17, EstateSkipScripts = $18, BillableFactor = $19, PublicAccess = $20, AbuseEmail = $21, EstateOwner = $22, DenyMinors = $23, AllowLandmark = $24, AllowParcelChanges = $25, AllowSetHome = $26, AllowEnviromentOverride = $27 WHERE EstateID = $28";

#[derive(Clone)]
pub struct EstateStore {
    pool: sqlx::AnyPool,
}

impl EstateStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get_estate(&self, estate_id: i32) -> Result<Option<EstateSettingsRow>, DataError> {
        let sql = format!("SELECT {ESTATE_COLS} FROM estate_settings WHERE EstateID = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        let row = sqlx::query_as::<_, EstateSettingsRow>(&q)
            .bind(estate_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn get_estate_by_name(
        &self,
        name: &str,
    ) -> Result<Option<EstateSettingsRow>, DataError> {
        let sql = format!("SELECT {ESTATE_COLS} FROM estate_settings WHERE EstateName = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        let row = sqlx::query_as::<_, EstateSettingsRow>(&q)
            .bind(name.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn get_estate_for_region(
        &self,
        region_id: &str,
    ) -> Result<Option<EstateSettingsRow>, DataError> {
        let sql = format!(
            "SELECT {ESTATE_COLS_E} FROM estate_settings e \
             JOIN estate_map m ON e.EstateID = m.EstateID WHERE m.RegionID = $1"
        );
        let q = crate::portable::sql(&self.pool, &sql);
        let row = sqlx::query_as::<_, EstateSettingsRow>(&q)
            .bind(region_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Create an estate, choosing the next free id (OpenSim starts new estates at 100).
    /// Returns the id; `estate.estate_id` is ignored.
    pub async fn create_estate(&self, estate: &EstateSettingsRow) -> Result<i32, DataError> {
        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(
            &self.pool,
            "SELECT COALESCE(MAX(EstateID), 99) + 1 FROM estate_settings",
        );
        let next: (i64,) = sqlx::query_as(&q)
            .fetch_one(&mut *tx)
            .await?;
        let id = next.0 as i32;
        let sql = format!(
            "INSERT INTO estate_settings ({ESTATE_COLS}) VALUES \
             ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28)"
        );
        let q = crate::portable::sql(&self.pool, &sql);
        sqlx::query(&q)
            .bind(id)
            .bind(estate.estate_name.clone())
            .bind(estate.abuse_email_to_estate_owner)
            .bind(estate.deny_anonymous)
            .bind(estate.reset_home_on_teleport)
            .bind(estate.fixed_sun)
            .bind(estate.deny_transacted)
            .bind(estate.block_dwell)
            .bind(estate.deny_identified)
            .bind(estate.allow_voice)
            .bind(estate.use_global_time)
            .bind(estate.price_per_meter)
            .bind(estate.tax_free)
            .bind(estate.allow_direct_teleport)
            .bind(estate.redirect_grid_x)
            .bind(estate.redirect_grid_y)
            .bind(estate.parent_estate_id)
            .bind(estate.sun_position)
            .bind(estate.estate_skip_scripts)
            .bind(estate.billable_factor)
            .bind(estate.public_access)
            .bind(estate.abuse_email.clone())
            .bind(estate.estate_owner.clone())
            .bind(estate.deny_minors)
            .bind(estate.allow_landmark)
            .bind(estate.allow_parcel_changes)
            .bind(estate.allow_set_home)
            .bind(estate.allow_environment_override)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn store_estate(&self, estate: &EstateSettingsRow) -> Result<(), DataError> {
        let q = crate::portable::sql(&self.pool, ESTATE_UPDATE);
        sqlx::query(&q)
            .bind(estate.estate_name.clone())
            .bind(estate.abuse_email_to_estate_owner)
            .bind(estate.deny_anonymous)
            .bind(estate.reset_home_on_teleport)
            .bind(estate.fixed_sun)
            .bind(estate.deny_transacted)
            .bind(estate.block_dwell)
            .bind(estate.deny_identified)
            .bind(estate.allow_voice)
            .bind(estate.use_global_time)
            .bind(estate.price_per_meter)
            .bind(estate.tax_free)
            .bind(estate.allow_direct_teleport)
            .bind(estate.redirect_grid_x)
            .bind(estate.redirect_grid_y)
            .bind(estate.parent_estate_id)
            .bind(estate.sun_position)
            .bind(estate.estate_skip_scripts)
            .bind(estate.billable_factor)
            .bind(estate.public_access)
            .bind(estate.abuse_email.clone())
            .bind(estate.estate_owner.clone())
            .bind(estate.deny_minors)
            .bind(estate.allow_landmark)
            .bind(estate.allow_parcel_changes)
            .bind(estate.allow_set_home)
            .bind(estate.allow_environment_override)
            .bind(estate.estate_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn link_region(&self, estate_id: i32, region_id: &str) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM estate_map WHERE RegionID = $1");
        sqlx::query(&q)
            .bind(region_id.to_string())
            .execute(&mut *tx)
            .await?;
        let q = crate::portable::sql(&self.pool, "INSERT INTO estate_map (RegionID, EstateID) VALUES ($1, $2)");
        sqlx::query(&q)
            .bind(region_id.to_string())
            .bind(estate_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn list_uuids(&self, table: &str, estate_id: i32) -> Result<Vec<String>, DataError> {
        let sql = format!("SELECT uuid FROM {table} WHERE EstateID = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        let rows: Vec<(String,)> = sqlx::query_as(&q)
            .bind(estate_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn add_uuid(&self, table: &str, estate_id: i32, uuid: &str) -> Result<(), DataError> {
        let sql = format!("INSERT INTO {table} (EstateID, uuid) VALUES ($1, $2)");
        let q = crate::portable::sql(&self.pool, &sql);
        sqlx::query(&q)
            .bind(estate_id)
            .bind(uuid.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn remove_uuid(
        &self,
        table: &str,
        estate_id: i32,
        uuid: &str,
    ) -> Result<bool, DataError> {
        let sql = format!("DELETE FROM {table} WHERE EstateID = $1 AND uuid = $2");
        let q = crate::portable::sql(&self.pool, &sql);
        let res = sqlx::query(&q)
            .bind(estate_id)
            .bind(uuid.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn get_estate_users(&self, estate_id: i32) -> Result<Vec<String>, DataError> {
        self.list_uuids("estate_users", estate_id).await
    }

    pub async fn add_estate_user(&self, estate_id: i32, uuid: &str) -> Result<(), DataError> {
        self.add_uuid("estate_users", estate_id, uuid).await
    }

    pub async fn remove_estate_user(&self, estate_id: i32, uuid: &str) -> Result<bool, DataError> {
        self.remove_uuid("estate_users", estate_id, uuid).await
    }

    pub async fn get_estate_groups(&self, estate_id: i32) -> Result<Vec<String>, DataError> {
        self.list_uuids("estate_groups", estate_id).await
    }

    pub async fn add_estate_group(&self, estate_id: i32, uuid: &str) -> Result<(), DataError> {
        self.add_uuid("estate_groups", estate_id, uuid).await
    }

    pub async fn remove_estate_group(&self, estate_id: i32, uuid: &str) -> Result<bool, DataError> {
        self.remove_uuid("estate_groups", estate_id, uuid).await
    }

    pub async fn get_estate_managers(&self, estate_id: i32) -> Result<Vec<String>, DataError> {
        self.list_uuids("estate_managers", estate_id).await
    }

    pub async fn add_estate_manager(&self, estate_id: i32, uuid: &str) -> Result<(), DataError> {
        self.add_uuid("estate_managers", estate_id, uuid).await
    }

    pub async fn remove_estate_manager(
        &self,
        estate_id: i32,
        uuid: &str,
    ) -> Result<bool, DataError> {
        self.remove_uuid("estate_managers", estate_id, uuid).await
    }

    pub async fn get_estate_bans(&self, estate_id: i32) -> Result<Vec<EstateBanRow>, DataError> {
        let q = crate::portable::sql(
            &self.pool,
            "SELECT EstateID, bannedUUID, bannedIp, bannedIpHostMask, bannedNameMask, \
             banningUUID, banTime FROM estateban WHERE EstateID = $1",
        );
        let rows = sqlx::query_as::<_, EstateBanRow>(&q)
            .bind(estate_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn add_estate_ban(&self, ban: &EstateBanRow) -> Result<(), DataError> {
        let q = crate::portable::sql(
            &self.pool,
            "INSERT INTO estateban (EstateID, bannedUUID, bannedIp, bannedIpHostMask, \
             bannedNameMask, banningUUID, banTime) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        );
        sqlx::query(&q)
            .bind(ban.estate_id)
            .bind(ban.banned_uuid.clone())
            .bind(ban.banned_ip.clone())
            .bind(ban.banned_ip_host_mask.clone())
            .bind(ban.banned_name_mask.clone())
            .bind(ban.banning_uuid.clone())
            .bind(ban.ban_time)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_estate_ban(&self, estate_id: i32, uuid: &str) -> Result<bool, DataError> {
        let q = crate::portable::sql(&self.pool, "DELETE FROM estateban WHERE EstateID = $1 AND bannedUUID = $2");
        let res = sqlx::query(&q)
            .bind(estate_id)
            .bind(uuid.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn get_all_estates(&self) -> Result<Vec<EstateSettingsRow>, DataError> {
        let sql = format!("SELECT {ESTATE_COLS} FROM estate_settings");
        let q = crate::portable::sql(&self.pool, &sql);
        let rows = sqlx::query_as::<_, EstateSettingsRow>(&q)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get_estate_ids_by_name(&self, name: &str) -> Result<Vec<i32>, DataError> {
        let q = crate::portable::sql(&self.pool, "SELECT EstateID FROM estate_settings WHERE EstateName = $1");
        let rows: Vec<(i32,)> = sqlx::query_as(&q)
            .bind(name.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn get_estate_ids_by_owner(&self, owner: &str) -> Result<Vec<i32>, DataError> {
        let q = crate::portable::sql(&self.pool, "SELECT EstateID FROM estate_settings WHERE EstateOwner = $1");
        let rows: Vec<(i32,)> = sqlx::query_as(&q)
            .bind(owner.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn get_regions_for_estate(&self, estate_id: i32) -> Result<Vec<String>, DataError> {
        let q = crate::portable::sql(&self.pool, "SELECT RegionID FROM estate_map WHERE EstateID = $1");
        let rows: Vec<(String,)> = sqlx::query_as(&q)
            .bind(estate_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
