// SPDX-License-Identifier: GPL-3.0-or-later

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FriendRow {
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
    #[sqlx(rename = "Friend")]
    pub friend: String,
    #[sqlx(rename = "Flags")]
    pub flags: String,
    #[sqlx(rename = "Offered")]
    pub offered: String,
}

pub struct FriendsStore {
    pool: sqlx::AnyPool,
}

impl FriendsStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get_friends(&self, principal_id: &str) -> Result<Vec<FriendRow>, DataError> {
        let rows = sqlx::query_as::<_, FriendRow>(
            "SELECT PrincipalID, Friend, Flags, Offered FROM Friends WHERE PrincipalID = $1",
        )
        .bind(principal_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn store_friend(
        &self,
        principal_id: &str,
        friend: &str,
        flags: &str,
    ) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM Friends WHERE PrincipalID = $1 AND Friend = $2")
            .bind(principal_id.to_string())
            .bind(friend.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO Friends (PrincipalID, Friend, Flags, Offered) VALUES ($1, $2, $3, '0')")
            .bind(principal_id.to_string())
            .bind(friend.to_string())
            .bind(flags.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_friend(&self, principal_id: &str, friend: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM Friends WHERE PrincipalID = $1 AND Friend = $2")
            .bind(principal_id.to_string())
            .bind(friend.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }
}
