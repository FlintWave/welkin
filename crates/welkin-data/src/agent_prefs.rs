// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentPrefsRow {
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
    #[sqlx(rename = "AccessPrefs")]
    pub access_prefs: String,
    #[sqlx(rename = "HoverHeight")]
    pub hover_height: f64,
    #[sqlx(rename = "Language")]
    pub language: String,
    #[sqlx(rename = "LanguageIsPublic")]
    pub language_is_public: i32,
    #[sqlx(rename = "PermEveryone")]
    pub perm_everyone: i32,
    #[sqlx(rename = "PermGroup")]
    pub perm_group: i32,
    #[sqlx(rename = "PermNextOwner")]
    pub perm_next_owner: i32,
}

#[derive(Clone)]
pub struct AgentPrefsStore {
    pool: sqlx::AnyPool,
}

impl AgentPrefsStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, principal_id: &str) -> Result<Option<AgentPrefsRow>, DataError> {
        let q = crate::portable::sql(
            &self.pool,
            "SELECT PrincipalID, AccessPrefs, HoverHeight, Language, LanguageIsPublic, \
             PermEveryone, PermGroup, PermNextOwner FROM AgentPrefs WHERE PrincipalID = $1",
        );
        let row = sqlx::query_as::<_, AgentPrefsRow>(&q)
            .bind(principal_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn store(&self, prefs: &AgentPrefsRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM AgentPrefs WHERE PrincipalID = $1");
        sqlx::query(&q)
            .bind(prefs.principal_id.clone())
            .execute(&mut *tx)
            .await?;
        let q = crate::portable::sql(
            &self.pool,
            "INSERT INTO AgentPrefs (PrincipalID, AccessPrefs, HoverHeight, Language, \
             LanguageIsPublic, PermEveryone, PermGroup, PermNextOwner) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        );
        sqlx::query(&q)
            .bind(prefs.principal_id.clone())
            .bind(prefs.access_prefs.clone())
            .bind(prefs.hover_height)
            .bind(prefs.language.clone())
            .bind(prefs.language_is_public)
            .bind(prefs.perm_everyone)
            .bind(prefs.perm_group)
            .bind(prefs.perm_next_owner)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete(&self, principal_id: &str) -> Result<bool, DataError> {
        let q = crate::portable::sql(&self.pool, "DELETE FROM AgentPrefs WHERE PrincipalID = $1");
        let res = sqlx::query(&q)
            .bind(principal_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }
}
