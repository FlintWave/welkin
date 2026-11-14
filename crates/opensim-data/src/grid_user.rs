// SPDX-License-Identifier: GPL-3.0-or-later

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GridUserRow {
    #[sqlx(rename = "UserID")]
    pub user_id: String,
    #[sqlx(rename = "HomeRegionID")]
    pub home_region_id: String,
    #[sqlx(rename = "HomePosition")]
    pub home_position: String,
    #[sqlx(rename = "HomeLookAt")]
    pub home_look_at: String,
    #[sqlx(rename = "LastRegionID")]
    pub last_region_id: String,
    #[sqlx(rename = "LastPosition")]
    pub last_position: String,
    #[sqlx(rename = "LastLookAt")]
    pub last_look_at: String,
    #[sqlx(rename = "Online")]
    pub online: String,
    #[sqlx(rename = "Login")]
    pub login: String,
    #[sqlx(rename = "Logout")]
    pub logout: String,
}

const COLS: &str = "UserID, HomeRegionID, HomePosition, HomeLookAt, LastRegionID, LastPosition, LastLookAt, Online, Login, Logout";

pub struct GridUserStore {
    pool: sqlx::AnyPool,
}

impl GridUserStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, user_id: &str) -> Result<Option<GridUserRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM GridUser WHERE UserID = $1");
        let row = sqlx::query_as::<_, GridUserRow>(&sql)
            .bind(user_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn get_online(&self) -> Result<Vec<GridUserRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM GridUser WHERE Online = 'true'");
        let rows = sqlx::query_as::<_, GridUserRow>(&sql)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn store(&self, row: &GridUserRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM GridUser WHERE UserID = $1")
            .bind(row.user_id.clone())
            .execute(&mut *tx)
            .await?;
        let sql = format!(
            "INSERT INTO GridUser ({COLS}) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
        );
        sqlx::query(&sql)
            .bind(row.user_id.clone())
            .bind(row.home_region_id.clone())
            .bind(row.home_position.clone())
            .bind(row.home_look_at.clone())
            .bind(row.last_region_id.clone())
            .bind(row.last_position.clone())
            .bind(row.last_look_at.clone())
            .bind(row.online.clone())
            .bind(row.login.clone())
            .bind(row.logout.clone())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_home(
        &self,
        user_id: &str,
        region_id: &str,
        position: &str,
        look_at: &str,
    ) -> Result<(), DataError> {
        sqlx::query(
            "UPDATE GridUser SET HomeRegionID = $1, HomePosition = $2, HomeLookAt = $3 WHERE UserID = $4",
        )
        .bind(region_id.to_string())
        .bind(position.to_string())
        .bind(look_at.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_last_position(
        &self,
        user_id: &str,
        region_id: &str,
        position: &str,
        look_at: &str,
    ) -> Result<(), DataError> {
        sqlx::query(
            "UPDATE GridUser SET LastRegionID = $1, LastPosition = $2, LastLookAt = $3 WHERE UserID = $4",
        )
        .bind(region_id.to_string())
        .bind(position.to_string())
        .bind(look_at.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_online(
        &self,
        user_id: &str,
        online: bool,
        timestamp: &str,
    ) -> Result<(), DataError> {
        let sql = if online {
            "UPDATE GridUser SET Online = $1, Login = $2 WHERE UserID = $3"
        } else {
            "UPDATE GridUser SET Online = $1, Logout = $2 WHERE UserID = $3"
        };
        sqlx::query(sql)
            .bind(if online { "true" } else { "false" }.to_string())
            .bind(timestamp.to_string())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
