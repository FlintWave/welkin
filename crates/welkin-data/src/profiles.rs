// SPDX-License-Identifier: GPL-3.0-or-later

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClassifiedRow {
    #[sqlx(rename = "classifieduuid")]
    pub classified_uuid: String,
    #[sqlx(rename = "creatoruuid")]
    pub creator_uuid: String,
    #[sqlx(rename = "creationdate")]
    pub creation_date: i32,
    #[sqlx(rename = "expirationdate")]
    pub expiration_date: i32,
    #[sqlx(rename = "category")]
    pub category: String,
    #[sqlx(rename = "name")]
    pub name: String,
    #[sqlx(rename = "description")]
    pub description: String,
    #[sqlx(rename = "parceluuid")]
    pub parcel_uuid: String,
    #[sqlx(rename = "parentestate")]
    pub parent_estate: i32,
    #[sqlx(rename = "snapshotuuid")]
    pub snapshot_uuid: String,
    #[sqlx(rename = "simname")]
    pub sim_name: String,
    #[sqlx(rename = "posglobal")]
    pub pos_global: String,
    #[sqlx(rename = "parcelname")]
    pub parcel_name: String,
    #[sqlx(rename = "classifiedflags")]
    pub classified_flags: i32,
    #[sqlx(rename = "priceforlisting")]
    pub price_for_listing: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PickRow {
    #[sqlx(rename = "pickuuid")]
    pub pick_uuid: String,
    #[sqlx(rename = "creatoruuid")]
    pub creator_uuid: String,
    #[sqlx(rename = "toppick")]
    pub top_pick: String,
    #[sqlx(rename = "parceluuid")]
    pub parcel_uuid: String,
    #[sqlx(rename = "name")]
    pub name: String,
    #[sqlx(rename = "description")]
    pub description: String,
    #[sqlx(rename = "snapshotuuid")]
    pub snapshot_uuid: String,
    #[sqlx(rename = "user")]
    pub user: String,
    #[sqlx(rename = "originalname")]
    pub original_name: String,
    #[sqlx(rename = "simname")]
    pub sim_name: String,
    #[sqlx(rename = "posglobal")]
    pub pos_global: String,
    #[sqlx(rename = "sortorder")]
    pub sort_order: i32,
    #[sqlx(rename = "enabled")]
    pub enabled: String,
    #[sqlx(rename = "gatekeeper")]
    pub gatekeeper: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProfileRow {
    #[sqlx(rename = "useruuid")]
    pub user_uuid: String,
    #[sqlx(rename = "profilePartner")]
    pub profile_partner: String,
    #[sqlx(rename = "profileAllowPublish")]
    pub profile_allow_publish: Vec<u8>,
    #[sqlx(rename = "profileMaturePublish")]
    pub profile_mature_publish: Vec<u8>,
    #[sqlx(rename = "profileURL")]
    pub profile_url: String,
    #[sqlx(rename = "profileWantToMask")]
    pub profile_want_to_mask: i32,
    #[sqlx(rename = "profileWantToText")]
    pub profile_want_to_text: String,
    #[sqlx(rename = "profileSkillsMask")]
    pub profile_skills_mask: i32,
    #[sqlx(rename = "profileSkillsText")]
    pub profile_skills_text: String,
    #[sqlx(rename = "profileLanguages")]
    pub profile_languages: String,
    #[sqlx(rename = "profileImage")]
    pub profile_image: String,
    #[sqlx(rename = "profileAboutText")]
    pub profile_about_text: String,
    #[sqlx(rename = "profileFirstImage")]
    pub profile_first_image: String,
    #[sqlx(rename = "profileFirstText")]
    pub profile_first_text: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteRow {
    #[sqlx(rename = "useruuid")]
    pub user_uuid: String,
    #[sqlx(rename = "targetuuid")]
    pub target_uuid: String,
    #[sqlx(rename = "notes")]
    pub notes: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserSettingsRow {
    #[sqlx(rename = "useruuid")]
    pub user_uuid: String,
    #[sqlx(rename = "imviaemail")]
    pub im_via_email: String,
    #[sqlx(rename = "visible")]
    pub visible: String,
    #[sqlx(rename = "email")]
    pub email: String,
}

/// Per-user application data, matching the C# `UserAppData` model.
/// Stores arbitrary app-specific data keyed by (user, app tag).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserAppDataRow {
    #[sqlx(rename = "UserId")]
    pub user_id: String,
    #[sqlx(rename = "TagId")]
    pub tag_id: String,
    #[sqlx(rename = "DataKey")]
    pub data_key: String,
    #[sqlx(rename = "DataVal")]
    pub data_val: String,
}

const CLASSIFIED_COLS: &str = "classifieduuid, creatoruuid, creationdate, expirationdate, \
    category, name, description, parceluuid, parentestate, snapshotuuid, simname, posglobal, \
    parcelname, classifiedflags, priceforlisting";

const PICK_COLS: &str = "pickuuid, creatoruuid, toppick, parceluuid, name, description, \
    snapshotuuid, user, originalname, simname, posglobal, sortorder, enabled, gatekeeper";

const PROFILE_COLS: &str = "useruuid, profilePartner, profileAllowPublish, profileMaturePublish, \
    profileURL, profileWantToMask, profileWantToText, profileSkillsMask, profileSkillsText, \
    profileLanguages, profileImage, profileAboutText, profileFirstImage, profileFirstText";

#[derive(Clone)]
pub struct ProfilesStore {
    pool: sqlx::AnyPool,
}

impl ProfilesStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    // ---- classifieds ----

    pub async fn get_classifieds(
        &self,
        creator_uuid: &str,
    ) -> Result<Vec<ClassifiedRow>, DataError> {
        let sql = format!("SELECT {CLASSIFIED_COLS} FROM classifieds WHERE creatoruuid = $1");
        let rows = sqlx::query_as::<_, ClassifiedRow>(&sql)
            .bind(creator_uuid.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get_classified(&self, id: &str) -> Result<Option<ClassifiedRow>, DataError> {
        let sql = format!("SELECT {CLASSIFIED_COLS} FROM classifieds WHERE classifieduuid = $1");
        let row = sqlx::query_as::<_, ClassifiedRow>(&sql)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_classified(&self, row: &ClassifiedRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM classifieds WHERE classifieduuid = $1")
            .bind(row.classified_uuid.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO classifieds ({CLASSIFIED_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"
        );
        sqlx::query(&sql)
            .bind(row.classified_uuid.clone())
            .bind(row.creator_uuid.clone())
            .bind(row.creation_date)
            .bind(row.expiration_date)
            .bind(row.category.clone())
            .bind(row.name.clone())
            .bind(row.description.clone())
            .bind(row.parcel_uuid.clone())
            .bind(row.parent_estate)
            .bind(row.snapshot_uuid.clone())
            .bind(row.sim_name.clone())
            .bind(row.pos_global.clone())
            .bind(row.parcel_name.clone())
            .bind(row.classified_flags)
            .bind(row.price_for_listing)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_classified(&self, id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM classifieds WHERE classifieduuid = $1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    // ---- picks ----

    pub async fn get_picks(&self, creator_uuid: &str) -> Result<Vec<PickRow>, DataError> {
        let sql = format!("SELECT {PICK_COLS} FROM userpicks WHERE creatoruuid = $1");
        let rows = sqlx::query_as::<_, PickRow>(&sql)
            .bind(creator_uuid.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get_pick(&self, id: &str) -> Result<Option<PickRow>, DataError> {
        let sql = format!("SELECT {PICK_COLS} FROM userpicks WHERE pickuuid = $1");
        let row = sqlx::query_as::<_, PickRow>(&sql)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_pick(&self, row: &PickRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM userpicks WHERE pickuuid = $1")
            .bind(row.pick_uuid.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO userpicks ({PICK_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"
        );
        sqlx::query(&sql)
            .bind(row.pick_uuid.clone())
            .bind(row.creator_uuid.clone())
            .bind(row.top_pick.clone())
            .bind(row.parcel_uuid.clone())
            .bind(row.name.clone())
            .bind(row.description.clone())
            .bind(row.snapshot_uuid.clone())
            .bind(row.user.clone())
            .bind(row.original_name.clone())
            .bind(row.sim_name.clone())
            .bind(row.pos_global.clone())
            .bind(row.sort_order)
            .bind(row.enabled.clone())
            .bind(row.gatekeeper.clone())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_pick(&self, id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM userpicks WHERE pickuuid = $1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    // ---- profile ----

    pub async fn get_profile(&self, uuid: &str) -> Result<Option<ProfileRow>, DataError> {
        let sql = format!("SELECT {PROFILE_COLS} FROM userprofile WHERE useruuid = $1");
        let row = sqlx::query_as::<_, ProfileRow>(&sql)
            .bind(uuid.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_profile(&self, row: &ProfileRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM userprofile WHERE useruuid = $1")
            .bind(row.user_uuid.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO userprofile ({PROFILE_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"
        );
        sqlx::query(&sql)
            .bind(row.user_uuid.clone())
            .bind(row.profile_partner.clone())
            .bind(row.profile_allow_publish.clone())
            .bind(row.profile_mature_publish.clone())
            .bind(row.profile_url.clone())
            .bind(row.profile_want_to_mask)
            .bind(row.profile_want_to_text.clone())
            .bind(row.profile_skills_mask)
            .bind(row.profile_skills_text.clone())
            .bind(row.profile_languages.clone())
            .bind(row.profile_image.clone())
            .bind(row.profile_about_text.clone())
            .bind(row.profile_first_image.clone())
            .bind(row.profile_first_text.clone())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    // ---- notes ----

    pub async fn get_note(
        &self,
        user_uuid: &str,
        target_uuid: &str,
    ) -> Result<Option<NoteRow>, DataError> {
        let row = sqlx::query_as::<_, NoteRow>(
            "SELECT useruuid, targetuuid, notes FROM usernotes \
             WHERE useruuid = $1 AND targetuuid = $2",
        )
        .bind(user_uuid.to_string())
        .bind(target_uuid.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn store_note(
        &self,
        user_uuid: &str,
        target_uuid: &str,
        notes: &str,
    ) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM usernotes WHERE useruuid = $1 AND targetuuid = $2")
            .bind(user_uuid.to_string())
            .bind(target_uuid.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO usernotes (useruuid, targetuuid, notes) VALUES ($1, $2, $3)")
            .bind(user_uuid.to_string())
            .bind(target_uuid.to_string())
            .bind(notes.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    // ---- settings ----

    pub async fn get_settings(&self, uuid: &str) -> Result<Option<UserSettingsRow>, DataError> {
        let row = sqlx::query_as::<_, UserSettingsRow>(
            "SELECT useruuid, imviaemail, visible, email FROM usersettings WHERE useruuid = $1",
        )
        .bind(uuid.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn store_settings(&self, row: &UserSettingsRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM usersettings WHERE useruuid = $1")
            .bind(row.user_uuid.clone())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO usersettings (useruuid, imviaemail, visible, email) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(row.user_uuid.clone())
        .bind(row.im_via_email.clone())
        .bind(row.visible.clone())
        .bind(row.email.clone())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    // ---- app data ----

    pub async fn get_app_data(
        &self,
        user_id: &str,
        tag_id: &str,
    ) -> Result<Option<UserAppDataRow>, DataError> {
        let row = sqlx::query_as::<_, UserAppDataRow>(
            "SELECT UserId, TagId, DataKey, DataVal FROM userdata \
             WHERE UserId = $1 AND TagId = $2",
        )
        .bind(user_id.to_string())
        .bind(tag_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn store_app_data(&self, row: &UserAppDataRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM userdata WHERE UserId = $1 AND TagId = $2")
            .bind(row.user_id.clone())
            .bind(row.tag_id.clone())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO userdata (UserId, TagId, DataKey, DataVal) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(row.user_id.clone())
        .bind(row.tag_id.clone())
        .bind(row.data_key.clone())
        .bind(row.data_val.clone())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
