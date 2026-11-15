// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::DataError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HgTravelingDataRow {
    #[sqlx(rename = "SessionID")]
    pub session_id: String,
    #[sqlx(rename = "UserID")]
    pub user_id: String,
    #[sqlx(rename = "GridExternalName")]
    pub grid_external_name: String,
    #[sqlx(rename = "ServiceToken")]
    pub service_token: String,
    #[sqlx(rename = "ClientIPAddress")]
    pub client_ip_address: String,
}

pub struct HgTravelingDataStore {
    pool: sqlx::AnyPool,
}

impl HgTravelingDataStore {
    pub fn new(pool: sqlx::AnyPool) -> Self {
        Self { pool }
    }

    pub async fn create_table(&self) -> Result<(), DataError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS hg_traveling_data (\
                SessionID VARCHAR(36) NOT NULL PRIMARY KEY, \
                UserID VARCHAR(36) NOT NULL, \
                GridExternalName VARCHAR(255) NOT NULL DEFAULT '', \
                ServiceToken VARCHAR(255) NOT NULL DEFAULT '', \
                ClientIPAddress VARCHAR(64) NOT NULL DEFAULT '' \
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Store a traveling-data record, replacing any existing row with the same
    /// session ID (upsert via delete-then-insert inside a transaction).
    pub async fn store(&self, row: &HgTravelingDataRow) -> Result<(), DataError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM hg_traveling_data WHERE SessionID = $1")
            .bind(row.session_id.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO hg_traveling_data \
                (SessionID, UserID, GridExternalName, ServiceToken, ClientIPAddress) \
                VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(row.session_id.to_string())
        .bind(row.user_id.to_string())
        .bind(row.grid_external_name.to_string())
        .bind(row.service_token.to_string())
        .bind(row.client_ip_address.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_by_session(
        &self,
        session_id: &str,
    ) -> Result<Option<HgTravelingDataRow>, DataError> {
        let row = sqlx::query_as::<_, HgTravelingDataRow>(
            "SELECT SessionID, UserID, GridExternalName, ServiceToken, ClientIPAddress \
             FROM hg_traveling_data WHERE SessionID = $1",
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_by_user(&self, user_id: &str) -> Result<Vec<HgTravelingDataRow>, DataError> {
        let rows = sqlx::query_as::<_, HgTravelingDataRow>(
            "SELECT SessionID, UserID, GridExternalName, ServiceToken, ClientIPAddress \
             FROM hg_traveling_data WHERE UserID = $1",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete(&self, session_id: &str) -> Result<bool, DataError> {
        let res = sqlx::query("DELETE FROM hg_traveling_data WHERE SessionID = $1")
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Delete records older than 72 hours.  Placeholder -- the table currently
    /// has no timestamp column, so this is a no-op that returns 0.
    pub async fn delete_old(&self) -> Result<u64, DataError> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Once;
    use std::sync::atomic::{AtomicU32, Ordering};
    static INIT_DRIVERS: Once = Once::new();
    static DB_COUNTER: AtomicU32 = AtomicU32::new(0);

    async fn setup() -> HgTravelingDataStore {
        INIT_DRIVERS.call_once(|| {
            sqlx::any::install_default_drivers();
        });
        // Each test gets its own named in-memory database so a pool with
        // multiple connections still sees the same schema.
        let n = DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let url = format!("sqlite:file:hg_travel_test_{n}?mode=memory&cache=shared");
        let opts = sqlx::any::AnyPoolOptions::new().max_connections(1);
        let pool = opts
            .connect(&url)
            .await
            .expect("failed to create in-memory pool");
        let store = HgTravelingDataStore::new(pool);
        store.create_table().await.expect("failed to create table");
        store
    }

    fn sample_row(session: &str, user: &str) -> HgTravelingDataRow {
        HgTravelingDataRow {
            session_id: session.to_string(),
            user_id: user.to_string(),
            grid_external_name: "http://other-grid.example:9000/".to_string(),
            service_token: "http://other-grid.example:9000/;some-token".to_string(),
            client_ip_address: "192.168.1.100".to_string(),
        }
    }

    #[tokio::test]
    async fn test_store_and_get() {
        let store = setup().await;
        let row = sample_row("sess-1", "user-1");
        store.store(&row).await.expect("store failed");

        let fetched = store
            .get_by_session("sess-1")
            .await
            .expect("get failed")
            .expect("row not found");

        assert_eq!(fetched.session_id, "sess-1");
        assert_eq!(fetched.user_id, "user-1");
        assert_eq!(
            fetched.grid_external_name,
            "http://other-grid.example:9000/"
        );
        assert_eq!(
            fetched.service_token,
            "http://other-grid.example:9000/;some-token"
        );
        assert_eq!(fetched.client_ip_address, "192.168.1.100");
    }

    #[tokio::test]
    async fn test_get_by_user() {
        let store = setup().await;
        store
            .store(&sample_row("sess-a", "user-shared"))
            .await
            .unwrap();
        store
            .store(&sample_row("sess-b", "user-shared"))
            .await
            .unwrap();
        store
            .store(&sample_row("sess-c", "user-other"))
            .await
            .unwrap();

        let rows = store.get_by_user("user-shared").await.unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.user_id == "user-shared"));
    }

    #[tokio::test]
    async fn test_delete() {
        let store = setup().await;
        store
            .store(&sample_row("sess-del", "user-1"))
            .await
            .unwrap();
        assert!(store.delete("sess-del").await.unwrap());
        assert!(store.get_by_session("sess-del").await.unwrap().is_none());
        // Deleting a non-existent row returns false.
        assert!(!store.delete("sess-del").await.unwrap());
    }

    #[tokio::test]
    async fn test_store_upsert() {
        let store = setup().await;
        let mut row = sample_row("sess-up", "user-1");
        store.store(&row).await.unwrap();

        // Update with different fields via the same session ID.
        row.grid_external_name = "http://new-grid.example:9000/".to_string();
        row.client_ip_address = "10.0.0.1".to_string();
        store.store(&row).await.unwrap();

        let fetched = store
            .get_by_session("sess-up")
            .await
            .unwrap()
            .expect("row not found after upsert");
        assert_eq!(fetched.grid_external_name, "http://new-grid.example:9000/");
        assert_eq!(fetched.client_ip_address, "10.0.0.1");

        // Ensure there's still only one row for that session.
        let all = store.get_by_user("user-1").await.unwrap();
        assert_eq!(all.len(), 1);
    }
}
