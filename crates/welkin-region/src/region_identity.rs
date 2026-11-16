// SPDX-License-Identifier: LGPL-3.0-or-later

//! A region keeps the same UUID across restarts.
//!
//! Everything the region saves (terrain, estate link, prims, settings) is keyed by
//! its UUID, so a new random id on every start would orphan all of it. The id comes
//! from `[Region] RegionUUID` when set; otherwise it is generated once and stored in
//! the region database under the region's name and grid location.

use sqlx::{AnyPool, Row};
use uuid::Uuid;
use welkin_data::migrations::Backend;

pub async fn create_table(pool: &AnyPool, backend: Backend) -> Result<(), sqlx::Error> {
    let sql = "CREATE TABLE IF NOT EXISTS region_identity (
            region_name VARCHAR(255) NOT NULL,
            loc_x INTEGER NOT NULL,
            loc_y INTEGER NOT NULL,
            region_id VARCHAR(36) NOT NULL,
            PRIMARY KEY (region_name, loc_x, loc_y)
        )"
    .to_string();
    let sql = welkin_data::portable::ddl(backend, &sql);
    sqlx::query(&sql)
        .execute(pool)
        .await?;
    Ok(())
}

/// The region's id: `configured` if given, else the stored one, else a new one that is stored.
pub async fn resolve(
    pool: &AnyPool,
    backend: Backend,
    name: &str,
    loc_x: u32,
    loc_y: u32,
    configured: Option<&str>,
) -> Result<Uuid, String> {
    create_table(pool, backend).await.map_err(|e| format!("region identity table: {e}"))?;
    let (x, y) = (i64::from(loc_x), i64::from(loc_y));

    if let Some(text) = configured.map(str::trim).filter(|t| !t.is_empty()) {
        let id = Uuid::parse_str(text).map_err(|_| format!("[Region] RegionUUID \"{text}\" is not a UUID"))?;
        // Remember it so removing the setting later keeps the same id.
        let sql = format!(
            "DELETE FROM region_identity WHERE region_name = {} AND loc_x = {} AND loc_y = {}",
            backend.ph(1),
            backend.ph(2),
            backend.ph(3)
        );
        sqlx::query(&sql)
            .bind(name)
            .bind(x)
            .bind(y)
            .execute(pool)
            .await
            .map_err(|e| format!("region identity: {e}"))?;
        store(pool, backend, name, x, y, id).await?;
        return Ok(id);
    }

    let sql = format!(
        "SELECT region_id FROM region_identity WHERE region_name = {} AND loc_x = {} AND loc_y = {}",
        backend.ph(1),
        backend.ph(2),
        backend.ph(3)
    );
    let row = sqlx::query(&sql)
        .bind(name)
        .bind(x)
        .bind(y)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("region identity: {e}"))?;
    if let Some(row) = row {
        let text: String = row.get("region_id");
        return Uuid::parse_str(text.trim()).map_err(|_| format!("stored region id \"{text}\" is not a UUID"));
    }
    let id = Uuid::new_v4();
    store(pool, backend, name, x, y, id).await?;
    Ok(id)
}

async fn store(pool: &AnyPool, backend: Backend, name: &str, x: i64, y: i64, id: Uuid) -> Result<(), String> {
    let sql = format!(
        "INSERT INTO region_identity (region_name, loc_x, loc_y, region_id) VALUES ({}, {}, {}, {})",
        backend.ph(1),
        backend.ph(2),
        backend.ph(3),
        backend.ph(4)
    );
    sqlx::query(&sql)
        .bind(name)
        .bind(x)
        .bind(y)
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(|e| format!("region identity: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> (welkin_data::DataPool, std::path::PathBuf) {
        sqlx::any::install_default_drivers();
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "welkin-identity-test-{}-{}.db",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let _ = std::fs::remove_file(&path);
        let pool = welkin_data::DataPool::connect(&format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .expect("pool");
        (pool, path)
    }

    #[tokio::test]
    async fn the_same_region_gets_the_same_id_on_every_start() {
        let (pool, path) = pool().await;
        let first = resolve(pool.inner(), welkin_data::migrations::Backend::SQLite, "Home", 1000, 1000, None).await.unwrap();
        let second = resolve(pool.inner(), welkin_data::migrations::Backend::SQLite, "Home", 1000, 1000, None).await.unwrap();
        assert_eq!(first, second);
        let other_name = resolve(pool.inner(), welkin_data::migrations::Backend::SQLite, "Away", 1000, 1000, None).await.unwrap();
        let other_place = resolve(pool.inner(), welkin_data::migrations::Backend::SQLite, "Home", 1001, 1000, None).await.unwrap();
        assert_ne!(first, other_name);
        assert_ne!(first, other_place);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn a_configured_id_wins_and_is_remembered() {
        let (pool, path) = pool().await;
        let generated = resolve(pool.inner(), welkin_data::migrations::Backend::SQLite, "Home", 1000, 1000, None).await.unwrap();
        let wanted = Uuid::new_v4();
        let got = resolve(pool.inner(), welkin_data::migrations::Backend::SQLite, "Home", 1000, 1000, Some(&wanted.to_string())).await.unwrap();
        assert_eq!(got, wanted);
        assert_ne!(got, generated);
        // The setting is removed later: the configured id stays.
        assert_eq!(resolve(pool.inner(), welkin_data::migrations::Backend::SQLite, "Home", 1000, 1000, None).await.unwrap(), wanted);
        assert!(resolve(pool.inner(), welkin_data::migrations::Backend::SQLite, "Home", 1000, 1000, Some("not-a-uuid")).await.is_err());
        let _ = std::fs::remove_file(path);
    }
}
