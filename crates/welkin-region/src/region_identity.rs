// SPDX-License-Identifier: LGPL-3.0-or-later

//! A region keeps the same UUID across restarts.
//!
//! Everything the region saves (terrain, estate link, prims, settings) is keyed by
//! its UUID, so a new random id on every start would orphan all of it. The id comes
//! from `[Region] RegionUUID` when set; otherwise it is generated once and stored in
//! the region database under the region's name and grid location.

use sqlx::{AnyPool, Row};
use uuid::Uuid;

pub async fn create_table(pool: &AnyPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS region_identity (
            region_name VARCHAR(255) NOT NULL,
            loc_x INTEGER NOT NULL,
            loc_y INTEGER NOT NULL,
            region_id VARCHAR(36) NOT NULL,
            PRIMARY KEY (region_name, loc_x, loc_y)
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// The region's id: `configured` if given, else the stored one, else a new one that is stored.
pub async fn resolve(
    pool: &AnyPool,
    name: &str,
    loc_x: u32,
    loc_y: u32,
    configured: Option<&str>,
) -> Result<Uuid, String> {
    create_table(pool).await.map_err(|e| format!("region identity table: {e}"))?;
    let (x, y) = (i64::from(loc_x), i64::from(loc_y));

    if let Some(text) = configured.map(str::trim).filter(|t| !t.is_empty()) {
        let id = Uuid::parse_str(text).map_err(|_| format!("[Region] RegionUUID \"{text}\" is not a UUID"))?;
        // Remember it so removing the setting later keeps the same id.
        sqlx::query("DELETE FROM region_identity WHERE region_name = $1 AND loc_x = $2 AND loc_y = $3")
            .bind(name)
            .bind(x)
            .bind(y)
            .execute(pool)
            .await
            .map_err(|e| format!("region identity: {e}"))?;
        store(pool, name, x, y, id).await?;
        return Ok(id);
    }

    let row = sqlx::query("SELECT region_id FROM region_identity WHERE region_name = $1 AND loc_x = $2 AND loc_y = $3")
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
    store(pool, name, x, y, id).await?;
    Ok(id)
}

async fn store(pool: &AnyPool, name: &str, x: i64, y: i64, id: Uuid) -> Result<(), String> {
    sqlx::query("INSERT INTO region_identity (region_name, loc_x, loc_y, region_id) VALUES ($1, $2, $3, $4)")
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
        let first = resolve(pool.inner(), "Home", 1000, 1000, None).await.unwrap();
        let second = resolve(pool.inner(), "Home", 1000, 1000, None).await.unwrap();
        assert_eq!(first, second);
        let other_name = resolve(pool.inner(), "Away", 1000, 1000, None).await.unwrap();
        let other_place = resolve(pool.inner(), "Home", 1001, 1000, None).await.unwrap();
        assert_ne!(first, other_name);
        assert_ne!(first, other_place);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn a_configured_id_wins_and_is_remembered() {
        let (pool, path) = pool().await;
        let generated = resolve(pool.inner(), "Home", 1000, 1000, None).await.unwrap();
        let wanted = Uuid::new_v4();
        let got = resolve(pool.inner(), "Home", 1000, 1000, Some(&wanted.to_string())).await.unwrap();
        assert_eq!(got, wanted);
        assert_ne!(got, generated);
        // The setting is removed later: the configured id stays.
        assert_eq!(resolve(pool.inner(), "Home", 1000, 1000, None).await.unwrap(), wanted);
        assert!(resolve(pool.inner(), "Home", 1000, 1000, Some("not-a-uuid")).await.is_err());
        let _ = std::fs::remove_file(path);
    }
}
