// SPDX-License-Identifier: GPL-3.0-or-later

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MuteRow {
    #[sqlx(rename = "AgentID")]
    pub agent_id: String,
    #[sqlx(rename = "MuteID")]
    pub mute_id: String,
    #[sqlx(rename = "MuteName")]
    pub mute_name: String,
    #[sqlx(rename = "MuteType")]
    pub mute_type: i32,
    #[sqlx(rename = "MuteFlags")]
    pub mute_flags: i32,
    #[sqlx(rename = "Stamp")]
    pub stamp: i32,
}

#[derive(Clone)]
pub struct MuteStore {
    pool: sqlx::AnyPool,
}

impl MuteStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn get_mutes(&self, agent_id: &str) -> Result<Vec<MuteRow>, DataError> {
        let rows = sqlx::query_as::<_, MuteRow>(
            "SELECT AgentID, MuteID, MuteName, MuteType, MuteFlags, Stamp \
             FROM MuteList WHERE AgentID = $1",
        )
        .bind(agent_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn add_mute(&self, mute: &MuteRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM MuteList WHERE AgentID = $1 AND MuteID = $2 AND MuteName = $3")
            .bind(mute.agent_id.clone())
            .bind(mute.mute_id.clone())
            .bind(mute.mute_name.clone())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO MuteList (AgentID, MuteID, MuteName, MuteType, MuteFlags, Stamp) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(mute.agent_id.clone())
        .bind(mute.mute_id.clone())
        .bind(mute.mute_name.clone())
        .bind(mute.mute_type)
        .bind(mute.mute_flags)
        .bind(mute.stamp)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_mute(
        &self,
        agent_id: &str,
        mute_id: &str,
        mute_name: &str,
    ) -> Result<bool, DataError> {
        let res = sqlx::query(
            "DELETE FROM MuteList WHERE AgentID = $1 AND MuteID = $2 AND MuteName = $3",
        )
        .bind(agent_id.to_string())
        .bind(mute_id.to_string())
        .bind(mute_name.to_string())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }
}
