// SPDX-License-Identifier: LGPL-3.0-or-later

//! User account storage (`UserAccounts` table).

use crate::DataError;

pub fn fold(s: &str) -> String {
    s.trim().to_lowercase()
}

pub fn name_key(first: &str, last: &str) -> String {
    format!("{} {}", fold(first), fold(last))
}

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
    names_ready: tokio::sync::OnceCell<()>,
}

impl UserAccountStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self {
            pool,
            names_ready: tokio::sync::OnceCell::new(),
        }
    }

    pub async fn ensure_names(&self) -> Result<(), DataError> {
        let q = crate::portable::sql(
            &self.pool,
            "CREATE TABLE IF NOT EXISTS UserAccountNames (NameKey VARCHAR(255) NOT NULL PRIMARY KEY, PrincipalID VARCHAR(36) NOT NULL)",
        );
        sqlx::query(&q)
            .execute(&self.pool)
            .await?;

        let q = crate::portable::sql(
            &self.pool,
            "SELECT PrincipalID, FirstName, LastName FROM UserAccounts WHERE PrincipalID NOT IN (SELECT PrincipalID FROM UserAccountNames)",
        );
        let rows: Vec<(String, String, String)> = sqlx::query_as::<_, (String, String, String)>(&q)
            .fetch_all(&self.pool)
            .await?;

        for (principal_id, first_name, last_name) in rows {
            let key = name_key(&first_name, &last_name);
            let q = crate::portable::sql(
                &self.pool,
                "INSERT INTO UserAccountNames (NameKey, PrincipalID) VALUES ($1, $2)",
            );
            if let Err(err) = sqlx::query(&q)
                .bind(&key)
                .bind(&principal_id)
                .execute(&self.pool)
                .await
            {
                tracing::warn!(principal_id = %principal_id, error = %err, "failed to backfill user account name; continuing");
            }
        }

        Ok(())
    }

    pub async fn get_by_id(&self, principal_id: &str) -> Result<Option<UserAccountRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM UserAccounts WHERE PrincipalID = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        Ok(sqlx::query_as::<_, UserAccountRow>(&q)
            .bind(principal_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_by_name(
        &self,
        first: &str,
        last: &str,
    ) -> Result<Option<UserAccountRow>, DataError> {
        self.names_ready
            .get_or_try_init(|| self.ensure_names())
            .await?;

        let key = name_key(first, last);
        let q = crate::portable::sql(
            &self.pool,
            "SELECT PrincipalID FROM UserAccountNames WHERE NameKey = $1",
        );
        let principal_id: Option<String> = sqlx::query_scalar::<_, String>(&q)
            .bind(&key)
            .fetch_optional(&self.pool)
            .await?;

        match principal_id {
            Some(id) => self.get_by_id(&id).await,
            None => Ok(None),
        }
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<UserAccountRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM UserAccounts WHERE Email = $1");
        let q = crate::portable::sql(&self.pool, &sql);
        Ok(sqlx::query_as::<_, UserAccountRow>(&q)
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
        self.names_ready
            .get_or_try_init(|| self.ensure_names())
            .await?;
        // NameKey is "first last", both Unicode-folded, so a first-name prefix is
        // `key LIKE 'q%'` and a last-name prefix is `key LIKE '% q%'`. Escape LIKE
        // wildcards in the query so they match literally.
        let folded: String = fold(query)
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let first_pattern = format!("{folded}%");
        let last_pattern = format!("% {folded}%");
        let sql = format!(
            "SELECT {COLS} FROM UserAccounts WHERE ScopeID = $1 AND PrincipalID IN \
             (SELECT PrincipalID FROM UserAccountNames \
              WHERE NameKey LIKE $2 ESCAPE '\\' OR NameKey LIKE $3 ESCAPE '\\')"
        );
        let q = crate::portable::sql(&self.pool, &sql);
        Ok(sqlx::query_as::<_, UserAccountRow>(&q)
            .bind(scope_id)
            .bind(&first_pattern)
            .bind(&last_pattern)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn store(&self, account: &UserAccountRow) -> Result<(), DataError> {
        self.names_ready
            .get_or_try_init(|| self.ensure_names())
            .await?;

        let mut tx = self.pool.begin().await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM UserAccounts WHERE PrincipalID = $1");
        sqlx::query(&q)
            .bind(&account.principal_id)
            .execute(&mut *tx)
            .await?;
        let q = crate::portable::sql(&self.pool, "DELETE FROM UserAccountNames WHERE PrincipalID = $1");
        sqlx::query(&q)
            .bind(&account.principal_id)
            .execute(&mut *tx)
            .await?;
        let q = crate::portable::sql(
            &self.pool,
            "INSERT INTO UserAccounts (PrincipalID, ScopeID, FirstName, LastName, Email, \
             ServiceURLs, Created, UserLevel, UserFlags, UserTitle, active) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        );
        sqlx::query(&q)
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
        let q = crate::portable::sql(
            &self.pool,
            "INSERT INTO UserAccountNames (NameKey, PrincipalID) VALUES ($1, $2)",
        );
        sqlx::query(&q)
            .bind(name_key(&account.first_name, &account.last_name))
            .bind(&account.principal_id)
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
        let q = crate::portable::sql(
            &self.pool,
            "UPDATE UserAccounts SET active = $1 WHERE PrincipalID = $2",
        );
        sqlx::query(&q)
            .bind(i32::from(active))
            .bind(principal_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (UserAccountStore, sqlx::AnyPool) {
        sqlx::any::install_default_drivers();
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE UserAccounts (PrincipalID CHAR(36) PRIMARY KEY, ScopeID CHAR(36) NOT NULL, FirstName VARCHAR(64) NOT NULL, LastName VARCHAR(64) NOT NULL, Email VARCHAR(64), ServiceURLs TEXT, Created INT(11), UserLevel INTEGER NOT NULL DEFAULT 0, UserFlags INTEGER NOT NULL DEFAULT 0, UserTitle VARCHAR(64) NOT NULL DEFAULT '', active INTEGER NOT NULL DEFAULT 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        (UserAccountStore::new(pool.clone()), pool)
    }

    fn make_account(principal_id: &str, first: &str, last: &str, email: &str) -> UserAccountRow {
        UserAccountRow {
            principal_id: principal_id.to_string(),
            scope_id: "00000000-0000-0000-0000-000000000000".to_string(),
            first_name: first.to_string(),
            last_name: last.to_string(),
            email: email.to_string(),
            service_urls: String::new(),
            created: 0,
            user_level: 0,
            user_flags: 0,
            user_title: String::new(),
            active: 1,
        }
    }

    #[tokio::test]
    async fn store_then_get_by_name_case_insensitive() {
        let (store, _pool) = setup().await;
        store
            .store(&make_account("pid1", "Emile", "Zola", "a@example.com"))
            .await
            .unwrap();
        let found = store
            .get_by_name("EMILE", "zola")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.principal_id, "pid1");
    }

    #[tokio::test]
    async fn duplicate_name_is_rejected() {
        let (store, _pool) = setup().await;
        store
            .store(&make_account("pid1", "Emile", "Zola", "a@example.com"))
            .await
            .unwrap();
        let err = store
            .store(&make_account("pid2", "emile", "zola", "b@example.com"))
            .await;
        assert!(err.is_err());
        let found = store
            .get_by_name("Emile", "Zola")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.principal_id, "pid1");
    }

    #[tokio::test]
    async fn non_ascii_duplicate_is_rejected() {
        let (store, _pool) = setup().await;
        store
            .store(&make_account("pid1", "Émile", "Zola", "a@example.com"))
            .await
            .unwrap();
        let err = store
            .store(&make_account("pid2", "émile", "zola", "b@example.com"))
            .await;
        assert!(err.is_err());
        let found = store
            .get_by_name("ÉMILE", "zola")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.principal_id, "pid1");
    }

    #[tokio::test]
    async fn restoring_same_principal_updates() {
        let (store, _pool) = setup().await;
        let mut account = make_account("pid1", "Emile", "Zola", "a@example.com");
        store.store(&account).await.unwrap();
        account.email = "b@example.com".to_string();
        store.store(&account).await.unwrap();
        let found = store
            .get_by_name("Emile", "Zola")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.email, "b@example.com");
    }

    #[tokio::test]
    async fn direct_row_is_backfilled() {
        let (store, pool) = setup().await;
        sqlx::query(
            "INSERT INTO UserAccounts (PrincipalID, ScopeID, FirstName, LastName, Email, ServiceURLs, Created, UserLevel, UserFlags, UserTitle, active) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind("pid-direct")
        .bind("00000000-0000-0000-0000-000000000000")
        .bind("Direct")
        .bind("User")
        .bind("direct@example.com")
        .bind("")
        .bind(0_i32)
        .bind(0_i32)
        .bind(0_i32)
        .bind("")
        .bind(1_i32)
        .execute(&pool)
        .await
        .unwrap();
        let found = store
            .get_by_name("direct", "USER")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.principal_id, "pid-direct");
    }

    #[tokio::test]
    async fn prefix_search_is_unicode_case_insensitive() {
        let (store, _pool) = setup().await;
        store.store(&make_account("11111111-1111-1111-1111-111111111111", "Über", "Resident", "")).await.unwrap();
        store.store(&make_account("22222222-2222-2222-2222-222222222222", "Plain", "Üvula", "")).await.unwrap();
        store.store(&make_account("33333333-3333-3333-3333-333333333333", "Other", "Person", "")).await.unwrap();
        let scope = "00000000-0000-0000-0000-000000000000";

        let by_first = store.get_accounts(scope, "üb").await.unwrap();
        assert_eq!(by_first.len(), 1);
        assert_eq!(by_first[0].first_name, "Über");

        let by_last = store.get_accounts(scope, "ÜV").await.unwrap();
        assert_eq!(by_last.len(), 1);
        assert_eq!(by_last[0].last_name, "Üvula");

        assert!(store.get_accounts(scope, "%").await.unwrap().is_empty());
        assert_eq!(store.get_accounts(scope, "oth").await.unwrap().len(), 1);
    }
}
