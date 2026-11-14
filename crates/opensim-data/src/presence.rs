// SPDX-License-Identifier: GPL-3.0-or-later

use crate::DataError;

const DT_FMT: &str = "%Y-%m-%d %H:%M:%S";

fn now_string() -> String {
    chrono::Utc::now().format(DT_FMT).to_string()
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PresenceRow {
    #[sqlx(rename = "UserID")]
    pub user_id: String,
    #[sqlx(rename = "RegionID")]
    pub region_id: String,
    #[sqlx(rename = "SessionID")]
    pub session_id: String,
    #[sqlx(rename = "SecureSessionID")]
    pub secure_session_id: String,
    #[sqlx(rename = "LastSeen")]
    pub last_seen: String,
}

const COLS: &str = "UserID, RegionID, SessionID, SecureSessionID, LastSeen";

pub struct PresenceStore {
    pool: sqlx::AnyPool,
}

impl PresenceStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_session(&self, session_id: &str) -> Result<Option<PresenceRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM Presence WHERE SessionID = $1");
        let row = sqlx::query_as::<_, PresenceRow>(&sql)
            .bind(session_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn get_by_user(&self, user_id: &str) -> Result<Vec<PresenceRow>, DataError> {
        let sql = format!("SELECT {COLS} FROM Presence WHERE UserID = $1");
        let rows = sqlx::query_as::<_, PresenceRow>(&sql)
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn get_agents(&self, user_ids: &[&str]) -> Result<Vec<PresenceRow>, DataError> {
        let mut out = Vec::new();
        for id in user_ids {
            out.extend(self.get_by_user(id).await?);
        }
        Ok(out)
    }

    pub async fn report_agent(&self, session_id: &str, region_id: &str) -> Result<(), DataError> {
        sqlx::query("UPDATE Presence SET RegionID = $1, LastSeen = $2 WHERE SessionID = $3")
            .bind(region_id.to_string())
            .bind(now_string())
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn login(
        &self,
        user_id: &str,
        session_id: &str,
        secure_session_id: &str,
    ) -> Result<(), DataError> {
        sqlx::query(
            "INSERT INTO Presence (UserID, SessionID, SecureSessionID, LastSeen) VALUES ($1, $2, $3, $4)",
        )
        .bind(user_id.to_string())
        .bind(session_id.to_string())
        .bind(secure_session_id.to_string())
        .bind(now_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn logout(&self, session_id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM Presence WHERE SessionID = $1")
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn logout_region(&self, region_id: &str) -> Result<u64, DataError> {
        let res = sqlx::query("DELETE FROM Presence WHERE RegionID = $1")
            .bind(region_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    pub async fn purge_stale(&self, seconds: i64) -> Result<u64, DataError> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::seconds(seconds))
            .format(DT_FMT)
            .to_string();
        let res = sqlx::query("DELETE FROM Presence WHERE LastSeen < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}
