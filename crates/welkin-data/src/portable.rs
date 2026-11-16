// SPDX-License-Identifier: LGPL-3.0-or-later
//! Backend-portable SQL text.
//!
//! Stores are written with PostgreSQL-style `$n` placeholders, each used once and in
//! bind order. [`sql`] rewrites them to `?` for SQLite and MySQL, which do not accept `$n`.

use sqlx::AnyPool;

/// True when the pool talks to PostgreSQL.
pub fn is_postgres(pool: &AnyPool) -> bool {
    let scheme = pool.connect_options().database_url.scheme().to_ascii_lowercase();
    scheme == "postgres" || scheme == "postgresql"
}

/// The dialect of the database behind `pool`.
pub fn backend_of(pool: &AnyPool) -> crate::migrations::Backend {
    use crate::migrations::Backend;
    let scheme = pool.connect_options().database_url.scheme().to_ascii_lowercase();
    if scheme.starts_with("mysql") || scheme.starts_with("mariadb") {
        Backend::MySQL
    } else if scheme.starts_with("postgres") {
        Backend::PostgreSQL
    } else {
        Backend::SQLite
    }
}

/// Rewrite `$n` placeholders in `query` for the pool's backend.
pub fn sql(pool: &AnyPool, query: &str) -> String {
    if is_postgres(pool) {
        query.to_string()
    } else {
        rewrite_placeholders(query)
    }
}

/// Replace every `$<digits>` outside a single-quoted literal with `?`.
pub fn rewrite_placeholders(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    let mut in_str = false;
    let mut chars = query.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            in_str = !in_str;
            out.push(c);
        } else if c == '$' && !in_str && chars.peek().is_some_and(|n| n.is_ascii_digit()) {
            while chars.peek().is_some_and(|n| n.is_ascii_digit()) {
                chars.next();
            }
            out.push('?');
        } else {
            out.push(c);
        }
    }
    out
}

/// Adapt column types in a `CREATE TABLE` statement written in the SQLite/MySQL dialect
/// to `backend`. PostgreSQL has no `BLOB`, `LONGBLOB`, `INT(n)` or `DATETIME`; MySQL
/// rejects a literal `DEFAULT` on `TEXT` columns.
pub fn ddl(backend: crate::migrations::Backend, statement: &str) -> String {
    use crate::migrations::Backend;
    match backend {
        Backend::SQLite => statement.to_string(),
        Backend::PostgreSQL => statement
            .replace("LONGBLOB", "BYTEA")
            .replace("MEDIUMBLOB", "BYTEA")
            .replace("BLOB", "BYTEA")
            .replace("INT(11)", "INTEGER")
            .replace("DATETIME", "TIMESTAMP"),
        Backend::MySQL => statement.replace("TEXT NOT NULL DEFAULT ''", "TEXT NOT NULL"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_numbered_placeholders() {
        assert_eq!(
            rewrite_placeholders("SELECT * FROM t WHERE a = $1 AND b = $12"),
            "SELECT * FROM t WHERE a = ? AND b = ?"
        );
    }

    #[test]
    fn leaves_string_literals_alone() {
        assert_eq!(rewrite_placeholders("SELECT '$1' , $2"), "SELECT '$1' , ?");
    }

    #[tokio::test]
    async fn sqlite_pool_is_not_postgres_and_gets_question_marks() {
        sqlx::any::install_default_drivers();
        let pool = sqlx::pool::PoolOptions::<sqlx::Any>::new()
            .connect("sqlite::memory:")
            .await
            .expect("connect");
        assert!(!is_postgres(&pool));
        assert_eq!(sql(&pool, "DELETE FROM t WHERE a = $1"), "DELETE FROM t WHERE a = ?");
    }

    #[test]
    fn ddl_adapts_types_per_backend() {
        use crate::migrations::Backend;
        let s = "CREATE TABLE t (a BLOB NOT NULL, b INT(11), c TEXT NOT NULL DEFAULT '')";
        assert_eq!(ddl(Backend::SQLite, s), s);
        assert_eq!(
            ddl(Backend::PostgreSQL, s),
            "CREATE TABLE t (a BYTEA NOT NULL, b INTEGER, c TEXT NOT NULL DEFAULT '')"
        );
        assert_eq!(
            ddl(Backend::MySQL, s),
            "CREATE TABLE t (a BLOB NOT NULL, b INT(11), c TEXT NOT NULL)"
        );
        assert_eq!(ddl(Backend::PostgreSQL, "x LONGBLOB"), "x BYTEA");
    }
}
