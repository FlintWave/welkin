// SPDX-License-Identifier: LGPL-3.0-or-later

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

#[derive(Debug, Clone)]
pub struct AvatarProperties {
    pub user_uuid: String,
    pub partner_id: String,
    pub about_text: String,
    pub first_life_text: String,
    pub image_id: String,
    pub first_life_image_id: String,
    pub web_url: String,
    pub profile_url: String,
    pub want_to_mask: i32,
    pub want_to_text: String,
    pub skills_mask: i32,
    pub skills_text: String,
    pub languages: String,
    pub publish_profile: bool,
    pub publish_mature: bool,
    pub born_on: String,
    pub charter_member: Vec<u8>,
    pub flags: u32,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
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
        let q = crate::portable::sql(&self.pool, &sql);
        let rows = sqlx::query_as::<_, ClassifiedRow>(&q)
            .bind(creator_uuid.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get_classified(&self, id: &str) -> Result<Option<ClassifiedRow>, DataError> {
        let sql = format!("SELECT {CLASSIFIED_COLS} FROM classifieds WHERE classifieduuid = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        let row = sqlx::query_as::<_, ClassifiedRow>(&q)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_classified(&self, row: &ClassifiedRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM classifieds WHERE classifieduuid = $1");
        sqlx::query(&q)
            .bind(row.classified_uuid.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO classifieds ({CLASSIFIED_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"
        );
        let q = crate::portable::sql(&self.pool, &sql);
        sqlx::query(&q)
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
        let q = crate::portable::sql(&self.pool, "DELETE FROM classifieds WHERE classifieduuid = $1");
        let res = sqlx::query(&q)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    // ---- picks ----

    pub async fn get_picks(&self, creator_uuid: &str) -> Result<Vec<PickRow>, DataError> {
        let sql = format!("SELECT {PICK_COLS} FROM userpicks WHERE creatoruuid = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        let rows = sqlx::query_as::<_, PickRow>(&q)
            .bind(creator_uuid.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get_pick(&self, id: &str) -> Result<Option<PickRow>, DataError> {
        let sql = format!("SELECT {PICK_COLS} FROM userpicks WHERE pickuuid = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        let row = sqlx::query_as::<_, PickRow>(&q)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_pick(&self, row: &PickRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM userpicks WHERE pickuuid = $1");
        sqlx::query(&q)
            .bind(row.pick_uuid.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO userpicks ({PICK_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"
        );
        let q = crate::portable::sql(&self.pool, &sql);
        sqlx::query(&q)
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
        let q = crate::portable::sql(&self.pool, "DELETE FROM userpicks WHERE pickuuid = $1");
        let res = sqlx::query(&q)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    // ---- profile ----

    pub async fn get_profile(&self, uuid: &str) -> Result<Option<ProfileRow>, DataError> {
        let sql = format!("SELECT {PROFILE_COLS} FROM userprofile WHERE useruuid = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        let row = sqlx::query_as::<_, ProfileRow>(&q)
            .bind(uuid.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_profile(&self, row: &ProfileRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM userprofile WHERE useruuid = $1");
        sqlx::query(&q)
            .bind(row.user_uuid.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO userprofile ({PROFILE_COLS}) VALUES \
             ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"
        );
        let q = crate::portable::sql(&self.pool, &sql);
        sqlx::query(&q)
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

    // ---- avatar properties (profile + account info) ----

    pub async fn get_avatar_properties(
        &self,
        uuid: &str,
    ) -> Result<Option<AvatarProperties>, DataError> {
        let profile = self.get_profile(uuid).await?;
        #[derive(sqlx::FromRow)]
        struct AccountRow {
            #[sqlx(rename = "FirstName")]
            first_name: String,
            #[sqlx(rename = "LastName")]
            last_name: String,
            #[sqlx(rename = "Created")]
            created: i32,
            #[sqlx(rename = "UserFlags")]
            user_flags: Option<i32>,
            #[sqlx(rename = "UserTitle")]
            user_title: Option<String>,
        }
        let account: Option<AccountRow> = sqlx::query_as::<_, AccountRow>(
            &crate::portable::sql(
                &self.pool,
                "SELECT FirstName, LastName, Created, UserFlags, UserTitle FROM UserAccounts WHERE PrincipalID = $1",
            ),
        )
        .bind(uuid.to_string())
        .fetch_optional(&self.pool)
        .await
        .unwrap_or_default();

        if profile.is_none() && account.is_none() {
            return Ok(None);
        }

        let p = profile.unwrap_or_else(|| ProfileRow {
            user_uuid: uuid.to_string(),
            profile_partner: "00000000-0000-0000-0000-000000000000".to_string(),
            profile_allow_publish: vec![0],
            profile_mature_publish: vec![0],
            profile_url: String::new(),
            profile_want_to_mask: 0,
            profile_want_to_text: String::new(),
            profile_skills_mask: 0,
            profile_skills_text: String::new(),
            profile_languages: String::new(),
            profile_image: "00000000-0000-0000-0000-000000000000".to_string(),
            profile_about_text: String::new(),
            profile_first_image: "00000000-0000-0000-0000-000000000000".to_string(),
            profile_first_text: String::new(),
        });

        let born_on = account
            .as_ref()
            .and_then(|a| {
                if a.created > 0 {
                    chrono::DateTime::from_timestamp(a.created as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d").to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let (charter_member, flags) = account
            .as_ref()
            .map(|a| {
                let uf = a.user_flags.unwrap_or(0);
                let mut membership = vec![((uf & 0x0f00) >> 8) as u8];
                let title = a.user_title.clone().unwrap_or_default();
                if !title.is_empty() {
                    membership = title.into_bytes();
                }
                let flgs = (uf & 0xff) as u32;
                (membership, flgs)
            })
            .unwrap_or((vec![0], 0));
        Ok(Some(AvatarProperties {
            user_uuid: uuid.to_string(),
            partner_id: p.profile_partner.clone(),
            about_text: p.profile_about_text.clone(),
            first_life_text: p.profile_first_text.clone(),
            image_id: p.profile_image.clone(),
            first_life_image_id: p.profile_first_image.clone(),
            web_url: p.profile_url.clone(),
            profile_url: p.profile_url.clone(),
            want_to_mask: p.profile_want_to_mask,
            want_to_text: p.profile_want_to_text.clone(),
            skills_mask: p.profile_skills_mask,
            skills_text: p.profile_skills_text.clone(),
            languages: p.profile_languages.clone(),
            publish_profile: p.profile_allow_publish.first().copied().unwrap_or(0) != 0,
            publish_mature: p.profile_mature_publish.first().copied().unwrap_or(0) != 0,
            born_on,
            charter_member: charter_member.clone(),
            flags,
            first_name: account.as_ref().map(|a| a.first_name.clone()),
            last_name: account.as_ref().map(|a| a.last_name.clone()),
        }))
    }

    pub async fn fetch_avatar_properties(
        &self,
        uuid: &str,
    ) -> Result<Option<AvatarProperties>, DataError> {
        self.get_avatar_properties(uuid).await
    }

    pub async fn get_profile_by_agent_uuid(
        &self,
        uuid: &str,
    ) -> Result<Option<AvatarProperties>, DataError> {
        self.get_avatar_properties(uuid).await
    }

    pub async fn fetch_profile_by_agent_uuid(
        &self,
        uuid: &str,
    ) -> Result<Option<AvatarProperties>, DataError> {
        self.get_avatar_properties(uuid).await
    }

    pub async fn fetch_profile(
        &self,
        uuid: &str,
    ) -> Result<Option<AvatarProperties>, DataError> {
        self.get_avatar_properties(uuid).await
    }

    // ---- notes ----

    pub async fn get_note(
        &self,
        user_uuid: &str,
        target_uuid: &str,
    ) -> Result<Option<NoteRow>, DataError> {
        let q = crate::portable::sql(
            &self.pool,
            "SELECT useruuid, targetuuid, notes FROM usernotes \
             WHERE useruuid = $1 AND targetuuid = $2",
        );
        let row = sqlx::query_as::<_, NoteRow>(&q)
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
        let q = crate::portable::sql(&self.pool, "DELETE FROM usernotes WHERE useruuid = $1 AND targetuuid = $2");
        sqlx::query(&q)
            .bind(user_uuid.to_string())
            .bind(target_uuid.to_string())
            .execute(&mut *tx)
            .await?;
        let q = crate::portable::sql(&self.pool, "INSERT INTO usernotes (useruuid, targetuuid, notes) VALUES ($1, $2, $3)");
        sqlx::query(&q)
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
        let q = crate::portable::sql(
            &self.pool,
            "SELECT useruuid, imviaemail, visible, email FROM usersettings WHERE useruuid = $1",
        );
        let row = sqlx::query_as::<_, UserSettingsRow>(&q)
            .bind(uuid.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_settings(&self, row: &UserSettingsRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM usersettings WHERE useruuid = $1");
        sqlx::query(&q)
            .bind(row.user_uuid.clone())
            .execute(&mut *tx)
            .await?;
        let q = crate::portable::sql(
            &self.pool,
            "INSERT INTO usersettings (useruuid, imviaemail, visible, email) \
             VALUES ($1, $2, $3, $4)",
        );
        sqlx::query(&q)
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
        let q = crate::portable::sql(
            &self.pool,
            "SELECT UserId, TagId, DataKey, DataVal FROM userdata \
             WHERE UserId = $1 AND TagId = $2",
        );
        let row = sqlx::query_as::<_, UserAppDataRow>(&q)
            .bind(user_id.to_string())
            .bind(tag_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store_app_data(&self, row: &UserAppDataRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM userdata WHERE UserId = $1 AND TagId = $2");
        sqlx::query(&q)
            .bind(row.user_id.clone())
            .bind(row.tag_id.clone())
            .execute(&mut *tx)
            .await?;
        let q = crate::portable::sql(
            &self.pool,
            "INSERT INTO userdata (UserId, TagId, DataKey, DataVal) \
             VALUES ($1, $2, $3, $4)",
        );
        sqlx::query(&q)
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::any::AnyPoolOptions;

    async fn pool() -> sqlx::AnyPool {
        sqlx::any::install_default_drivers();
        let p = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS userprofile (
              useruuid varchar(36) NOT NULL PRIMARY KEY,
              profilePartner varchar(36) NOT NULL,
              profileAllowPublish binary(1) NOT NULL,
              profileMaturePublish binary(1) NOT NULL,
              profileURL varchar(255) NOT NULL,
              profileWantToMask int(3) NOT NULL,
              profileWantToText text NOT NULL,
              profileSkillsMask int(3) NOT NULL,
              profileSkillsText text NOT NULL,
              profileLanguages text NOT NULL,
              profileImage varchar(36) NOT NULL,
              profileAboutText text NOT NULL,
              profileFirstImage varchar(36) NOT NULL,
              profileFirstText text NOT NULL
            )",
        )
        .execute(&p)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS UserAccounts (
              PrincipalID CHAR(36) PRIMARY KEY,
              ScopeID CHAR(36) NOT NULL,
              FirstName VARCHAR(64) NOT NULL,
              LastName VARCHAR(64) NOT NULL,
              Email VARCHAR(64),
              ServiceURLs TEXT,
              Created INT(11),
              UserLevel integer NOT NULL DEFAULT 0,
              UserFlags integer NOT NULL DEFAULT 0,
              UserTitle varchar(64) NOT NULL DEFAULT ''
            )",
        )
        .execute(&p)
        .await
        .unwrap();
        p
    }

    #[tokio::test]
    async fn avatar_properties_roundtrip() {
        let p = pool().await;
        let store = ProfilesStore::new(p.clone());
        let uuid = "11111111-2222-3333-4444-555555555555";
        let row = ProfileRow {
            user_uuid: uuid.to_string(),
            profile_partner: "00000000-0000-0000-0000-000000000000".to_string(),
            profile_allow_publish: vec![1],
            profile_mature_publish: vec![0],
            profile_url: "http://example".to_string(),
            profile_want_to_mask: 3,
            profile_want_to_text: "fun".to_string(),
            profile_skills_mask: 4,
            profile_skills_text: "rust".to_string(),
            profile_languages: "en".to_string(),
            profile_image: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
            profile_about_text: "about me".to_string(),
            profile_first_image: "11111111-2222-3333-4444-555555555555".to_string(),
            profile_first_text: "first life".to_string(),
        };
        store.store_profile(&row).await.unwrap();
        sqlx::query("INSERT INTO UserAccounts (PrincipalID, ScopeID, FirstName, LastName, Created) VALUES ($1, '00000000-0000-0000-0000-000000000000', 'Test', 'User', 1700000000)")
            .bind(uuid.to_string())
            .execute(&p)
            .await
            .unwrap();
        let ap = store.get_avatar_properties(uuid).await.unwrap().unwrap();
        assert_eq!(ap.about_text, "about me");
        assert_eq!(ap.first_name.as_deref(), Some("Test"));
        assert!(!ap.born_on.is_empty());
        let missing = store
            .get_avatar_properties("00000000-0000-0000-0000-000000000000")
            .await
            .unwrap();
        assert!(missing.is_none());
    }
}
