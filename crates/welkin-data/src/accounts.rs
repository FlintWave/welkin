// SPDX-License-Identifier: LGPL-3.0-or-later

//! User account storage (`UserAccounts` table).

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserAccountRow {
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
    #[sqlx(rename = "ScopeID")]
    pub scope_id: String,
    #[sqlx(rename = "FirstName")]
    pub first_name: String,
    #[sqlx(rename = "LastName")]
    pub last_name: String,
    #[sqlx(rename = "Email")]
    pub email: String,
    #[sqlx(rename = "ServiceURLs")]
    pub service_urls: String,
    #[sqlx(rename = "Created")]
    pub created: i32,
    #[sqlx(rename = "UserLevel")]
    pub user_level: i32,
    #[sqlx(rename = "UserFlags")]
    pub user_flags: i32,
    #[sqlx(rename = "UserTitle")]
    pub user_title: String,
    pub active: i32,
}

// Nullable columns are wrapped in COALESCE so they decode into plain values.
const COLS: &str = "PrincipalID, ScopeID, FirstName, LastName, COALESCE(Email, '') AS Email, \
    COALESCE(ServiceURLs, '') AS ServiceURLs, COALESCE(Created, 0) AS Created, UserLevel, \
    UserFlags, UserTitle, active";

pub struct UserAccountStore {
    pool: sqlx::AnyPool,
}

impl UserAccountStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_id(&self, principal_id: &str) -> Result<Option<UserAccountRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM UserAccounts WHERE PrincipalID = $1");
        Ok(sqlx::query_as::<_, UserAccountRow>(&sql)
            .bind(principal_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_by_name(
        &self,
        first: &str,
        last: &str,
    ) -> Result<Option<UserAccountRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM UserAccounts WHERE LOWER(FirstName) = LOWER($1) AND LOWER(LastName) = LOWER($2)");
        Ok(sqlx::query_as::<_, UserAccountRow>(&sql)
            .bind(first)
            .bind(last)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<UserAccountRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM UserAccounts WHERE Email = $1");
        Ok(sqlx::query_as::<_, UserAccountRow>(&sql)
            .bind(email)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Prefix search: `query` is matched as `query%` against FirstName or LastName.
    pub async fn get_accounts(
        &self,
        scope_id: &str,
        query: &str,
    ) -> Result<Vec<UserAccountRow>, DataError> {
        let sql = format!(
            "SELECT {COLS} FROM UserAccounts WHERE ScopeID = $1 \
             AND (FirstName LIKE $2 OR LastName LIKE $3)"
        );
        let pattern = format!("{query}%");
        Ok(sqlx::query_as::<_, UserAccountRow>(&sql)
            .bind(scope_id)
            .bind(&pattern)
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn store(&self, account: &UserAccountRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM UserAccounts WHERE PrincipalID = $1")
            .bind(&account.principal_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO UserAccounts (PrincipalID, ScopeID, FirstName, LastName, Email, \
             ServiceURLs, Created, UserLevel, UserFlags, UserTitle, active) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(&account.principal_id)
        .bind(&account.scope_id)
        .bind(&account.first_name)
        .bind(&account.last_name)
        .bind(&account.email)
        .bind(&account.service_urls)
        .bind(account.created)
        .bind(account.user_level)
        .bind(account.user_flags)
        .bind(&account.user_title)
        .bind(account.active)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_multi_accounts(
        &self,
        ids: &[&str],
    ) -> Result<Vec<Option<UserAccountRow>>, DataError> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            out.push(self.get_by_id(id).await?);
        }
        Ok(out)
    }

    pub async fn set_active(&self, principal_id: &str, active: bool) -> Result<(), DataError> {
        sqlx::query("UPDATE UserAccounts SET active = $1 WHERE PrincipalID = $2")
            .bind(i32::from(active))
            .bind(principal_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
