// SPDX-License-Identifier: GPL-3.0-or-later

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
    pub billable_factor: f32,
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

const ESTATE_UPDATE: &str = "UPDATE estate_settings SET EstateName = $2, AbuseEmailToEstateOwner = $3, DenyAnonymous = $4, ResetHomeOnTeleport = $5, FixedSun = $6, DenyTransacted = $7, BlockDwell = $8, DenyIdentified = $9, AllowVoice = $10, UseGlobalTime = $11, PricePerMeter = $12, TaxFree = $13, AllowDirectTeleport = $14, RedirectGridX = $15, RedirectGridY = $16, ParentEstateID = $17, SunPosition = $18, EstateSkipScripts = $19, BillableFactor = $20, PublicAccess = $21, AbuseEmail = $22, EstateOwner = $23, DenyMinors = $24, AllowLandmark = $25, AllowParcelChanges = $26, AllowSetHome = $27, AllowEnviromentOverride = $28 WHERE EstateID = $1";

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
        let row = sqlx::query_as::<_, EstateSettingsRow>(&sql)
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
        let row = sqlx::query_as::<_, EstateSettingsRow>(&sql)
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
        let row = sqlx::query_as::<_, EstateSettingsRow>(&sql)
            .bind(region_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_estate(&self, estate: &EstateSettingsRow) -> Result<(), DataError> {
        sqlx::query(ESTATE_UPDATE)
            .bind(estate.estate_id)
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
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn link_region(&self, estate_id: i32, region_id: &str) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM estate_map WHERE RegionID = $1")
            .bind(region_id.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO estate_map (RegionID, EstateID) VALUES ($1, $2)")
            .bind(region_id.to_string())
            .bind(estate_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn list_uuids(&self, table: &str, estate_id: i32) -> Result<Vec<String>, DataError> {
        let sql = format!("SELECT uuid FROM {table} WHERE EstateID = $1");
        let rows: Vec<(String,)> = sqlx::query_as(&sql)
            .bind(estate_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn add_uuid(&self, table: &str, estate_id: i32, uuid: &str) -> Result<(), DataError> {
        let sql = format!("INSERT INTO {table} (EstateID, uuid) VALUES ($1, $2)");
        sqlx::query(&sql)
            .bind(estate_id)
            .bind(uuid.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn remove_uuid(&self, table: &str, estate_id: i32, uuid: &str) -> Result<bool, DataError> {
        let sql = format!("DELETE FROM {table} WHERE EstateID = $1 AND uuid = $2");
        let res = sqlx::query(&sql)
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

    pub async fn remove_estate_manager(&self, estate_id: i32, uuid: &str) -> Result<bool, DataError> {
        self.remove_uuid("estate_managers", estate_id, uuid).await
    }

    pub async fn get_estate_bans(&self, estate_id: i32) -> Result<Vec<EstateBanRow>, DataError> {
        let rows = sqlx::query_as::<_, EstateBanRow>(
            "SELECT EstateID, bannedUUID, bannedIp, bannedIpHostMask, bannedNameMask, \
             banningUUID, banTime FROM estateban WHERE EstateID = $1",
        )
        .bind(estate_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn add_estate_ban(&self, ban: &EstateBanRow) -> Result<(), DataError> {
        sqlx::query(
            "INSERT INTO estateban (EstateID, bannedUUID, bannedIp, bannedIpHostMask, \
             bannedNameMask, banningUUID, banTime) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
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
        let res = sqlx::query("DELETE FROM estateban WHERE EstateID = $1 AND bannedUUID = $2")
            .bind(estate_id)
            .bind(uuid.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }
}
