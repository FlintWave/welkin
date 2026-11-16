// SPDX-License-Identifier: LGPL-3.0-or-later

//! Per-user balance / transaction persistence.

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountBalance {
    #[sqlx(rename = "uuid")]
    pub uuid: String,
    #[sqlx(rename = "balance")]
    pub balance: i64,
    #[sqlx(rename = "last_tx")]
    pub last_tx: Option<String>,
}

#[derive(Clone)]
pub struct AccountBalanceStore {
    pool: sqlx::AnyPool,
}

impl AccountBalanceStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    async fn ensure_table(&self) -> Result<(), DataError> {
        let q = crate::portable::sql(
            &self.pool,
            "CREATE TABLE IF NOT EXISTS AccountBalance (uuid VARCHAR(36) NOT NULL PRIMARY KEY, balance BIGINT NOT NULL DEFAULT 0, last_tx VARCHAR(36))",
        );
        let q = crate::portable::ddl(crate::portable::backend_of(&self.pool), &q);
        sqlx::query(&q).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_balance<U: ToString + Send>(
        &self,
        uuid: U,
    ) -> Result<Option<AccountBalance>, DataError> {
        self.ensure_table().await?;
        let uuid = uuid.to_string();
        let q = crate::portable::sql(
            &self.pool,
            "SELECT uuid, balance, last_tx FROM AccountBalance WHERE uuid = $1",
        );
        Ok(sqlx::query_as::<_, AccountBalance>(&q)
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn set_balance<U: ToString + Send>(
        &self,
        uuid: U,
        balance: i64,
    ) -> Result<(), DataError> {
        self.ensure_table().await?;
        let uuid = uuid.to_string();
        let q = crate::portable::sql(&self.pool, "DELETE FROM AccountBalance WHERE uuid = $1");
        sqlx::query(&q).bind(&uuid).execute(&self.pool).await?;
        let q = crate::portable::sql(
            &self.pool,
            "INSERT INTO AccountBalance (uuid, balance, last_tx) VALUES ($1, $2, $3)",
        );
        sqlx::query(&q)
            .bind(&uuid)
            .bind(balance)
            .bind(Option::<String>::None)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn record_transaction<U: ToString + Send>(
        &self,
        uuid: U,
        amount: i64,
    ) -> Result<(), DataError> {
        self.ensure_table().await?;
        let uuid = uuid.to_string();
        let tx_id = uuid::Uuid::new_v4().to_string();
        let current = self.get_balance(&uuid).await?.map(|b| b.balance).unwrap_or(0);
        let q = crate::portable::sql(&self.pool, "DELETE FROM AccountBalance WHERE uuid = $1");
        sqlx::query(&q).bind(&uuid).execute(&self.pool).await?;
        let q = crate::portable::sql(
            &self.pool,
            "INSERT INTO AccountBalance (uuid, balance, last_tx) VALUES ($1, $2, $3)",
        );
        sqlx::query(&q)
            .bind(&uuid)
            .bind(current + amount)
            .bind(tx_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Return up to `limit` recently recorded transaction ids for `uuid`.
    ///
    /// The store persists only the latest transaction id (``last_tx``) per
    /// account, so this surfaces that value (capped to ``limit``) rather
    /// than a full history.
    pub async fn recent_transactions<U: ToString + Send>(
        &self,
        uuid: U,
        limit: usize,
    ) -> Result<Vec<String>, DataError> {
        self.ensure_table().await?;
        if limit == 0 {
            return Ok(Vec::new());
        }
        let uuid = uuid.to_string();
        let q = crate::portable::sql(
            &self.pool,
            "SELECT last_tx FROM AccountBalance WHERE uuid = $1",
        );
        let row: Option<(Option<String>,)> = sqlx::query_as(&q)
            .bind(&uuid)
            .fetch_optional(&self.pool)
            .await?;
        let mut out = Vec::new();
        if let Some((Some(tx),)) = row {
            out.push(tx);
        }
        out.truncate(limit);
        Ok(out)
    }
}

pub type BalanceStore = AccountBalanceStore;
pub type UserBalanceStore = AccountBalanceStore;