// SPDX-License-Identifier: LGPL-3.0-or-later

//! Schema migration runner compatible with the OpenSimulator `migrations` table.

use crate::DataError;

/// Database backend, used to select migration files and SQL placeholders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    MySQL,
    PostgreSQL,
    SQLite,
}

/// Stores migrated by `run_all` (those used by the grid services).
pub const GRID_STORES: &[&str] = &[
    "AssetStore",
    "AuthStore",
    "Avatar",
    "EstateStore",
    "FriendsStore",
    "GridStore",
    "GridUserStore",
    "InventoryStore",
    "MuteListStore",
    "Presence",
    "UserAccount",
    "UserProfiles",
    "AgentPrefs",
];

/// Parse a `.migrations` file into `(version, sql)` pairs.
pub fn parse_migrations(content: &str) -> Vec<(i32, String)> {
    let content = content.trim_start_matches('\u{feff}');
    let mut out: Vec<(i32, String)> = Vec::new();
    let mut current: Option<(i32, String)> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(":VERSION") {
            let tok = rest.split_whitespace().next().unwrap_or("");
            if let Ok(v) = tok.parse::<i32>() {
                if let Some(c) = current.take() {
                    out.push(c);
                }
                current = Some((v, String::new()));
                continue;
            }
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some((_, sql)) = current.as_mut() {
            sql.push_str(line);
            sql.push('\n');
        }
    }
    if let Some(c) = current.take() {
        out.push(c);
    }
    for (_, sql) in out.iter_mut() {
        *sql = sql.trim().to_string();
    }
    out
}

/// Split a SQL block into individual statements, dropping BEGIN/COMMIT.
/// Respects quoted strings, backtick identifiers and `--` comments.
pub fn split_statements(sql: &str) -> Vec<String> {
    let mut stmts = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    if chars.peek() == Some(&q) {
                        cur.push(chars.next().unwrap());
                    } else {
                        quote = None;
                    }
                }
            }
            None => match c {
                '\'' | '"' | '`' => {
                    quote = Some(c);
                    cur.push(c);
                }
                '-' if chars.peek() == Some(&'-') => {
                    // line comment: skip to end of line
                    for n in chars.by_ref() {
                        if n == '\n' {
                            cur.push('\n');
                            break;
                        }
                    }
                }
                ';' => {
                    push_stmt(&mut stmts, &cur);
                    cur.clear();
                }
                _ => cur.push(c),
            },
        }
    }
    push_stmt(&mut stmts, &cur);
    stmts
}

fn push_stmt(stmts: &mut Vec<String>, raw: &str) {
    let s = raw.trim();
    if s.is_empty() {
        return;
    }
    let up = s.to_ascii_uppercase();
    let up = up.split_whitespace().collect::<Vec<_>>().join(" ");
    if up == "BEGIN" || up == "BEGIN TRANSACTION" || up == "COMMIT" || up == "END TRANSACTION" {
        return;
    }
    stmts.push(s.to_string());
}

/// Return embedded migration content for `store` on `backend`, if present.
pub fn migration_content(store: &str, backend: Backend) -> Option<&'static str> {
    Some(match backend {
        Backend::MySQL => match store {
            "AssetStore" => include_str!("../migrations/mysql/AssetStore.migrations"),
            "AuthStore" => include_str!("../migrations/mysql/AuthStore.migrations"),
            "Avatar" => include_str!("../migrations/mysql/Avatar.migrations"),
            "EstateStore" => include_str!("../migrations/mysql/EstateStore.migrations"),
            "FriendsStore" => include_str!("../migrations/mysql/FriendsStore.migrations"),
            "FSAssetStore" => include_str!("../migrations/mysql/FSAssetStore.migrations"),
            "GridStore" => include_str!("../migrations/mysql/GridStore.migrations"),
            "GridUserStore" => include_str!("../migrations/mysql/GridUserStore.migrations"),
            "HGTravelStore" => include_str!("../migrations/mysql/HGTravelStore.migrations"),
            "IM_Store" => include_str!("../migrations/mysql/IM_Store.migrations"),
            "InventoryStore" => include_str!("../migrations/mysql/InventoryStore.migrations"),
            "LogStore" => include_str!("../migrations/mysql/LogStore.migrations"),
            "MuteListStore" => include_str!("../migrations/mysql/MuteListStore.migrations"),
            "os_groups_Store" => include_str!("../migrations/mysql/os_groups_Store.migrations"),
            "Presence" => include_str!("../migrations/mysql/Presence.migrations"),
            "RegionStore" => include_str!("../migrations/mysql/RegionStore.migrations"),
            "UserAccount" => include_str!("../migrations/mysql/UserAccount.migrations"),
            "UserProfiles" => include_str!("../migrations/mysql/UserProfiles.migrations"),
            "XAssetStore" => include_str!("../migrations/mysql/XAssetStore.migrations"),
            "AgentPrefs" => include_str!("../migrations/mysql/AgentPrefs.migrations"),
            _ => return None,
        },
        Backend::PostgreSQL => match store {
            "AssetStore" => include_str!("../migrations/pgsql/AssetStore.migrations"),
            "AuthStore" => include_str!("../migrations/pgsql/AuthStore.migrations"),
            "Avatar" => include_str!("../migrations/pgsql/Avatar.migrations"),
            "EstateStore" => include_str!("../migrations/pgsql/EstateStore.migrations"),
            "FriendsStore" => include_str!("../migrations/pgsql/FriendsStore.migrations"),
            "FSAssetStore" => include_str!("../migrations/pgsql/FSAssetStore.migrations"),
            "GridStore" => include_str!("../migrations/pgsql/GridStore.migrations"),
            "GridUserStore" => include_str!("../migrations/pgsql/GridUserStore.migrations"),
            "HGTravelStore" => include_str!("../migrations/pgsql/HGTravelStore.migrations"),
            "IM_Store" => include_str!("../migrations/pgsql/IM_Store.migrations"),
            "InventoryStore" => include_str!("../migrations/pgsql/InventoryStore.migrations"),
            "LogStore" => include_str!("../migrations/pgsql/LogStore.migrations"),
            "MuteListStore" => include_str!("../migrations/pgsql/MuteListStore.migrations"),
            "os_groups_Store" => include_str!("../migrations/pgsql/os_groups_Store.migrations"),
            "Presence" => include_str!("../migrations/pgsql/Presence.migrations"),
            "RegionStore" => include_str!("../migrations/pgsql/RegionStore.migrations"),
            "UserAccount" => include_str!("../migrations/pgsql/UserAccount.migrations"),
            "UserProfiles" => include_str!("../migrations/pgsql/UserProfiles.migrations"),
            "XAssetStore" => include_str!("../migrations/pgsql/XAssetStore.migrations"),
            "AgentPrefs" => include_str!("../migrations/pgsql/AgentPrefs.migrations"),
            _ => return None,
        },
        Backend::SQLite => match store {
            "AssetStore" => include_str!("../migrations/sqlite/AssetStore.migrations"),
            "AuthStore" => include_str!("../migrations/sqlite/AuthStore.migrations"),
            "Avatar" => include_str!("../migrations/sqlite/Avatar.migrations"),
            "EstateStore" => include_str!("../migrations/sqlite/EstateStore.migrations"),
            "FriendsStore" => include_str!("../migrations/sqlite/FriendsStore.migrations"),
            "HGTravelStore" => include_str!("../migrations/sqlite/HGTravelStore.migrations"),
            "InventoryStore" => include_str!("../migrations/sqlite/XInventoryStore.migrations"),
            "MuteListStore" => include_str!("../migrations/sqlite/MuteListStore.migrations"),
            "RegionStore" => include_str!("../migrations/sqlite/RegionStore.migrations"),
            "UserAccount" => include_str!("../migrations/sqlite/UserAccount.migrations"),
            "UserProfiles" => include_str!("../migrations/sqlite/UserProfiles.migrations"),
            "AgentPrefs" => include_str!("../migrations/sqlite/AgentPrefs.migrations"),
            _ => return None,
        },
    })
}

pub struct MigrationRunner {
    pub pool: sqlx::AnyPool,
    pub backend: Backend,
}

impl MigrationRunner {
    pub fn new(pool: sqlx::AnyPool, backend: Backend) -> Self {
        Self { pool, backend }
    }

    fn ph(&self, n: usize) -> String {
        match self.backend {
            Backend::PostgreSQL => format!("${}", n),
            _ => "?".to_string(),
        }
    }

    pub async fn ensure_migrations_table(&self) -> Result<(), DataError> {
        sqlx::query("CREATE TABLE IF NOT EXISTS migrations(name VARCHAR(100), version INT)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Current version: 0 if no row, -1 if the table does not exist.
    pub async fn get_version(&self, name: &str) -> Result<i32, DataError> {
        let sql = format!("SELECT version FROM migrations WHERE name = {}", self.ph(1));
        match sqlx::query_scalar::<_, i32>(&sql)
            .bind(name.to_string())
            .fetch_optional(&self.pool)
            .await
        {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Ok(0),
            Err(_) => Ok(-1),
        }
    }

    pub async fn set_version(&self, name: &str, version: i32) -> Result<(), DataError> {
        let exists_sql = format!("SELECT version FROM migrations WHERE name = {}", self.ph(1));
        let exists = sqlx::query_scalar::<_, i32>(&exists_sql)
            .bind(name.to_string())
            .fetch_optional(&self.pool)
            .await?
            .is_some();
        if exists {
            let sql = format!(
                "UPDATE migrations SET version = {} WHERE name = {}",
                self.ph(1),
                self.ph(2)
            );
            sqlx::query(&sql)
                .bind(version)
                .bind(name.to_string())
                .execute(&self.pool)
                .await?;
        } else {
            let sql = format!(
                "INSERT INTO migrations(name, version) VALUES ({}, {})",
                self.ph(1),
                self.ph(2)
            );
            sqlx::query(&sql)
                .bind(name.to_string())
                .bind(version)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Apply all blocks with version greater than the current one.
    /// BEGIN/COMMIT lines are stripped and each block runs in one sqlx
    /// transaction (note: MySQL commits DDL implicitly).
    pub async fn run_migrations(&self, name: &str, content: &str) -> Result<i32, DataError> {
        let mut current = self.get_version(name).await?;
        if current < 0 {
            self.ensure_migrations_table().await?;
            current = self.get_version(name).await?.max(0);
        }
        for (version, sql) in parse_migrations(content) {
            if version <= current {
                continue;
            }
            let mut tx = self.pool.begin().await?;
            for stmt in split_statements(&sql) {
                sqlx::query(&stmt).execute(&mut *tx).await?;
            }
            tx.commit().await?;
            self.set_version(name, version).await?;
            current = version;
        }
        Ok(current)
    }

    /// Run migrations for all grid-service stores for this backend.
    pub async fn run_all(&self) -> Result<(), DataError> {
        self.ensure_migrations_table().await?;
        for store in GRID_STORES {
            if let Some(content) = migration_content(store, self.backend) {
                self.run_migrations(store, content).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocks() {
        let c = "\u{feff}:VERSION 1  # x\r\n\r\nBEGIN;\r\nCREATE TABLE a(x INT);\r\nCOMMIT;\r\n# c\r\n:VERSION 2\nALTER TABLE a ADD y INT;\n";
        let p = parse_migrations(c);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].0, 1);
        assert!(p[0].1.starts_with("BEGIN;") && p[0].1.ends_with("COMMIT;"));
        assert_eq!(p[1].0, 2);
    }

    #[test]
    fn splits_statements() {
        let s = split_statements(
            "BEGIN;\nINSERT INTO a VALUES ('x;y'); -- hi; there\nCREATE TABLE b(i INT);\nCOMMIT;",
        );
        assert_eq!(s.len(), 2);
        assert!(s[0].contains("'x;y'"));
    }
}
