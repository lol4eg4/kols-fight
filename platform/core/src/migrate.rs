//! Per-plugin schema migrations.
//!
//! Each plugin's migrator runs on a detached connection with
//! `CREATE SCHEMA IF NOT EXISTS <name>; SET search_path TO <name>`, so its
//! `_sqlx_migrations` and tables live in its schema. The connection is closed afterwards and
//! never returns to the pool; the pool keeps the default search_path.

use sqlx::{PgPool, migrate::Migrator};

/// A plugin migration failed.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("schema name {0:?} is not a plain lowercase identifier")]
    BadSchema(String),
    #[error("schema {schema}: {source}")]
    Db {
        schema: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("schema {schema}: {source}")]
    Migrate {
        schema: String,
        #[source]
        source: sqlx::migrate::MigrateError,
    },
}

fn check_schema(schema: &str) -> Result<(), MigrateError> {
    let ok = schema.starts_with(|c: char| c.is_ascii_lowercase())
        && schema
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && schema != "public"
        && !schema.starts_with("pg_");
    if ok {
        Ok(())
    } else {
        Err(MigrateError::BadSchema(schema.to_string()))
    }
}

/// Runs `migrator` inside schema `schema` on a detached connection.
pub async fn migrate_plugin(
    pool: &PgPool,
    schema: &str,
    migrator: &Migrator,
) -> Result<(), MigrateError> {
    check_schema(schema)?;
    let db = |source| MigrateError::Db {
        schema: schema.to_string(),
        source,
    };
    let mut conn = pool.acquire().await.map_err(db)?.detach();
    let result = async {
        sqlx::raw_sql(&format!(
            "CREATE SCHEMA IF NOT EXISTS {schema}; SET search_path TO {schema}"
        ))
        .execute(&mut conn)
        .await
        .map_err(db)?;
        migrator
            .run(&mut conn)
            .await
            .map_err(|source| MigrateError::Migrate {
                schema: schema.to_string(),
                source,
            })
    }
    .await;
    // Closing ends the session, so the search_path change never reaches the pool.
    let _ = sqlx::Connection::close(conn).await;
    result
}

/// Number of `migrator`'s up migrations not yet applied in schema `schema`.
pub async fn pending_migrations(
    pool: &PgPool,
    schema: &str,
    migrator: &Migrator,
) -> Result<usize, sqlx::Error> {
    check_schema(schema).map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    let ups = migrator
        .iter()
        .filter(|m| !m.migration_type.is_down_migration());
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(format!("{schema}._sqlx_migrations"))
        .fetch_one(pool)
        .await?;
    if !exists {
        return Ok(ups.count());
    }
    let applied: Vec<i64> = sqlx::query_scalar(&format!(
        "SELECT version FROM {schema}._sqlx_migrations WHERE success"
    ))
    .fetch_all(pool)
    .await?;
    Ok(ups.filter(|m| !applied.contains(&m.version)).count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_names_are_checked() {
        assert!(check_schema("snapshots").is_ok());
        assert!(check_schema("kol_2").is_ok());
        for bad in ["", "public", "pg_x", "Bad", "a;drop", "1a", "a-b"] {
            assert!(check_schema(bad).is_err(), "{bad}");
        }
    }
}
