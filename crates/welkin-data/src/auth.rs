// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::DataError;

const DT_FMT: &str = "%Y-%m-%d %H:%M:%S";

fn now_string() -> String {
    chrono::Utc::now().format(DT_FMT).to_string()
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthRow {
    #[sqlx(rename = "UUID")]
    pub uuid: String,
    #[sqlx(rename = "passwordHash")]
    pub password_hash: String,
    #[sqlx(rename = "passwordSalt")]
    pub password_salt: String,
    #[sqlx(rename = "webLoginKey")]
    pub web_login_key: String,
    #[sqlx(rename = "accountType")]
    pub account_type: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TokenRow {
    #[sqlx(rename = "UUID")]
    pub uuid: String,
    pub token: String,
    pub validity: String,
}

pub struct AuthStore {
    pool: sqlx::AnyPool,
}

impl AuthStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, uuid: &str) -> Result<Option<AuthRow>, DataError> {
        let row = sqlx::query_as::<_, AuthRow>(
            "SELECT UUID, passwordHash, passwordSalt, webLoginKey, accountType FROM auth WHERE UUID = $1",
        )
        .bind(uuid.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn store(&self, auth: &AuthRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM auth WHERE UUID = $1")
            .bind(auth.uuid.clone())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO auth (UUID, passwordHash, passwordSalt, webLoginKey, accountType) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(auth.uuid.clone())
        .bind(auth.password_hash.clone())
        .bind(auth.password_salt.clone())
        .bind(auth.web_login_key.clone())
        .bind(auth.account_type.clone())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_password(&self, uuid: &str, hash: &str, salt: &str) -> Result<(), DataError> {
        sqlx::query("UPDATE auth SET passwordHash = $1, passwordSalt = $2 WHERE UUID = $3")
            .bind(hash.to_string())
            .bind(salt.to_string())
            .bind(uuid.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_token(&self, uuid: &str, token: &str) -> Result<Option<TokenRow>, DataError> {
        let row = sqlx::query_as::<_, TokenRow>(
            "SELECT UUID, token, validity FROM tokens WHERE UUID = $1 AND token = $2",
        )
        .bind(uuid.to_string())
        .bind(token.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn store_token(
        &self,
        uuid: &str,
        token: &str,
        validity_minutes: i32,
    ) -> Result<(), DataError> {
        let validity = (chrono::Utc::now()
            + chrono::Duration::minutes(i64::from(validity_minutes)))
        .format(DT_FMT)
        .to_string();
        sqlx::query("INSERT INTO tokens (UUID, token, validity) VALUES ($1, $2, $3)")
            .bind(uuid.to_string())
            .bind(token.to_string())
            .bind(validity)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn check_token(&self, uuid: &str, token: &str) -> Result<bool, DataError> {
        let row = sqlx::query_as::<_, TokenRow>(
            "SELECT UUID, token, validity FROM tokens WHERE UUID = $1 AND token = $2 AND validity > $3",
        )
        .bind(uuid.to_string())
        .bind(token.to_string())
        .bind(now_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn release_token(&self, uuid: &str, token: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM tokens WHERE UUID = $1 AND token = $2")
            .bind(uuid)
            .bind(token)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn purge_expired_tokens(&self) -> Result<u64, DataError> {
        let res = sqlx::query("DELETE FROM tokens WHERE validity < $1")
            .bind(now_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}
