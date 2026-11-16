// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AvatarDataRow {
    #[sqlx(rename = "PrincipalID")]
    pub principal_id: String,
    #[sqlx(rename = "Name")]
    pub name: String,
    #[sqlx(rename = "Value")]
    pub value: Option<String>,
}

#[derive(Clone)]
pub struct AvatarStore {
    pool: sqlx::AnyPool,
}

impl AvatarStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, principal_id: &str) -> Result<Vec<AvatarDataRow>, DataError> {
        let rows = sqlx::query_as::<_, AvatarDataRow>(
            &crate::portable::sql(&self.pool, "SELECT PrincipalID, Name, Value FROM Avatars WHERE PrincipalID = $1"),
        )
        .bind(principal_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_value(
        &self,
        principal_id: &str,
        name: &str,
    ) -> Result<Option<String>, DataError> {
        let row = sqlx::query_as::<_, AvatarDataRow>(
            &crate::portable::sql(&self.pool, "SELECT PrincipalID, Name, Value FROM Avatars WHERE PrincipalID = $1 AND Name = $2"),
        )
        .bind(principal_id.to_string())
        .bind(name.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|r| r.value))
    }

    pub async fn store(
        &self,
        principal_id: &str,
        entries: &[(String, String)],
    ) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(&crate::portable::sql(&self.pool, "DELETE FROM Avatars WHERE PrincipalID = $1"))
            .bind(principal_id.to_string())
            .execute(&mut *tx)
            .await?;
        for (name, value) in entries {
            sqlx::query(&crate::portable::sql(&self.pool, "INSERT INTO Avatars (PrincipalID, Name, Value) VALUES ($1, $2, $3)"))
                .bind(principal_id.to_string())
                .bind(name.clone())
                .bind(value.clone())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete(&self, principal_id: &str) -> Result<bool, DataError> {
        let res = sqlx::query(&crate::portable::sql(&self.pool, "DELETE FROM Avatars WHERE PrincipalID = $1"))
            .bind(principal_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn set_value(
        &self,
        principal_id: &str,
        name: &str,
        value: &str,
    ) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(&crate::portable::sql(&self.pool, "DELETE FROM Avatars WHERE PrincipalID = $1 AND Name = $2"))
            .bind(principal_id.to_string())
            .bind(name.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query(&crate::portable::sql(&self.pool, "INSERT INTO Avatars (PrincipalID, Name, Value) VALUES ($1, $2, $3)"))
            .bind(principal_id.to_string())
            .bind(name.to_string())
            .bind(value.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn reset(&self, principal_id: &str) -> Result<bool, DataError> {
        self.delete(principal_id).await
    }

    pub async fn set_items(
        &self,
        principal_id: &str,
        names: &[&str],
        values: &[&str],
    ) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        for (name, value) in names.iter().zip(values.iter()) {
            sqlx::query(&crate::portable::sql(&self.pool, "DELETE FROM Avatars WHERE PrincipalID = $1 AND Name = $2"))
                .bind(principal_id.to_string())
                .bind(name.to_string())
                .execute(&mut *tx)
                .await?;
            sqlx::query(&crate::portable::sql(&self.pool, "INSERT INTO Avatars (PrincipalID, Name, Value) VALUES ($1, $2, $3)"))
                .bind(principal_id.to_string())
                .bind(name.to_string())
                .bind(value.to_string())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_items(&self, principal_id: &str, names: &[&str]) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        for name in names {
            sqlx::query(&crate::portable::sql(&self.pool, "DELETE FROM Avatars WHERE PrincipalID = $1 AND Name = $2"))
                .bind(principal_id.to_string())
                .bind(name.to_string())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
