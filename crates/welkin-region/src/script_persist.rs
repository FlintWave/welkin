// SPDX-License-Identifier: LGPL-3.0-or-later

use sqlx::any::AnyRow;
use sqlx::{AnyPool, Row};
use uuid::Uuid;
use welkin_data::migrations::Backend;
use welkin_script_vm::ScriptState;

pub async fn create_script_states_table(pool: &AnyPool, backend: Backend) -> Result<(), sqlx::Error> {
    let sql = "CREATE TABLE IF NOT EXISTS script_states (
            script_id VARCHAR(36) PRIMARY KEY,
            prim_id VARCHAR(36) NOT NULL,
            bytecode BLOB NOT NULL,
            globals_json TEXT NOT NULL
        )"
    .to_string();
    let sql = welkin_data::portable::ddl(backend, &sql);
    sqlx::query(&sql)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn save_script_states(
    pool: &AnyPool,
    backend: Backend,
    states: &[(Uuid, Uuid, ScriptState)],
) -> Result<usize, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let delete_sql = "DELETE FROM script_states".to_string();
    sqlx::query(&delete_sql)
        .execute(&mut *tx)
        .await?;
    let count = states.len();
    for (script_id, prim_id, state) in states {
        let sql = format!(
            "INSERT INTO script_states (script_id, prim_id, bytecode, globals_json) VALUES ({}, {}, {}, {})",
            backend.ph(1),
            backend.ph(2),
            backend.ph(3),
            backend.ph(4)
        );
        sqlx::query(&sql)
            .bind(script_id.to_string())
            .bind(prim_id.to_string())
            .bind(&state.bytecode)
            .bind(&state.globals_json)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(count)
}

/// Remove one saved script state (for example when its object no longer exists).
pub async fn delete_script_state(pool: &AnyPool, backend: Backend, script_id: Uuid) -> Result<(), sqlx::Error> {
    let sql = format!(
        "DELETE FROM script_states WHERE script_id = {}",
        backend.ph(1)
    );
    sqlx::query(&sql)
        .bind(script_id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn load_script_states(
    pool: &AnyPool,
    backend: Backend,
) -> Result<Vec<(Uuid, Uuid, ScriptState)>, sqlx::Error> {
    let _ = backend;
    let sql = "SELECT script_id, prim_id, bytecode, globals_json FROM script_states".to_string();
    let rows: Vec<AnyRow> =
        sqlx::query(&sql)
            .fetch_all(pool)
            .await?;
    let mut result = Vec::with_capacity(rows.len());
    for row in &rows {
        let sid_str: String = row.get("script_id");
        let pid_str: String = row.get("prim_id");
        let bytecode: Vec<u8> = row.get("bytecode");
        let globals_json: String = row.get("globals_json");
        let script_id = Uuid::parse_str(&sid_str).map_err(|e| {
            sqlx::Error::ColumnDecode {
                index: "script_id".into(),
                source: Box::new(e),
            }
        })?;
        let prim_id = Uuid::parse_str(&pid_str).map_err(|e| sqlx::Error::ColumnDecode {
            index: "prim_id".into(),
            source: Box::new(e),
        })?;
        result.push((
            script_id,
            prim_id,
            ScriptState {
                bytecode,
                globals_json,
            },
        ));
    }
    Ok(result)
}
