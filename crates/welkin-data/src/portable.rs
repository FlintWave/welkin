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
}
