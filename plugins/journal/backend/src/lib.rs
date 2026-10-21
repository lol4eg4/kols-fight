//! Backend of the `journal` plugin (layer 0): a plain store of changes.
//!
//! Implements [`api_journal::JournalApi`]. No routes, no screens, no permission checks.
//! `before` and `after` are JSON; values under secret-looking keys are replaced with
//! `"[redacted]"` before anything is written (see [`redact`]).

use api_journal::{Entry, EntryId, JournalApi, JournalError};
use arena_platform::{Ctx, Person, Plugin, PluginEntry, Role, StartError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row, migrate::Migrator};
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "journal";

/// Migrations from `plugins/journal/migrations`, run in schema `journal`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// Text written in place of a secret value.
pub const REDACTED: &str = "[redacted]";

pub struct JournalPlugin;

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    ctx.apis.provide::<dyn JournalApi>(
        NAME,
        vec![("default", Arc::new(Journal::new(ctx.pool.clone())))],
        Arc::new(api_journal::stub::Stub),
    )?;
    Ok(Arc::new(JournalPlugin))
}

impl Plugin for JournalPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}

/// True for keys that hold a secret: password, token, hash, session, cookie, key, secret.
fn is_secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "hash",
        "session",
        "cookie",
        "api_key",
        "apikey",
        "api-key",
        "private_key",
        "authorization",
    ]
    .iter()
    .any(|s| k.contains(s))
}

/// A copy of `value` with every value under a secret-looking key replaced by `"[redacted]"`,
/// at any depth.
pub fn redact(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let v = if is_secret_key(&k) {
                        Value::String(REDACTED.into())
                    } else {
                        redact(v)
                    };
                    (k, v)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redact).collect()),
        other => other,
    }
}

/// The journal store over the shared pool.
pub struct Journal {
    pool: PgPool,
}

impl Journal {
    pub fn new(pool: PgPool) -> Self {
        Journal { pool }
    }
}

fn internal(e: sqlx::Error) -> JournalError {
    JournalError::Internal {
        message: e.to_string(),
    }
}

fn role_from(s: &str) -> Role {
    match s {
        "staff" => Role::Staff,
        "admin" => Role::Admin,
        _ => Role::Nobody,
    }
}

#[async_trait]
impl JournalApi for Journal {
    async fn record(
        &self,
        person: &Person,
        block: &str,
        object: &str,
        action: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) -> Result<EntryId, JournalError> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO journal.entry (person_id, role, block, object, action, before, after) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(person.id)
        .bind(person.role.as_str())
        .bind(block)
        .bind(object)
        .bind(action)
        .bind(before.map(redact))
        .bind(after.map(redact))
        .fetch_one(&self.pool)
        .await
        .map_err(internal)?;
        Ok(id)
    }

    async fn history(
        &self,
        _person: &Person,
        block: &str,
        object: &str,
    ) -> Result<Vec<Entry>, JournalError> {
        let rows = sqlx::query(
            "SELECT id, at, person_id, role, block, object, action, before, after \
             FROM journal.entry WHERE block = $1 AND object = $2 ORDER BY at, id",
        )
        .bind(block)
        .bind(object)
        .fetch_all(&self.pool)
        .await
        .map_err(internal)?;
        rows.into_iter()
            .map(|r| {
                Ok(Entry {
                    id: r.try_get("id")?,
                    at: r.try_get::<DateTime<Utc>, _>("at")?,
                    person_id: r.try_get("person_id")?,
                    role: role_from(&r.try_get::<String, _>("role")?),
                    block: r.try_get("block")?,
                    object: r.try_get("object")?,
                    action: r.try_get("action")?,
                    before: r.try_get("before")?,
                    after: r.try_get("after")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_platform::testing::TestDb;
    use serde_json::json;

    async fn journal() -> (TestDb, Journal) {
        let db = TestDb::with_plugins(&[(NAME, &MIGRATOR)]).await;
        let j = Journal::new(db.pool().clone());
        (db, j)
    }

    #[tokio::test]
    async fn record_then_history_oldest_first() {
        let (db, j) = journal().await;
        let admin = Person::admin(7);
        let a = j
            .record(
                &admin,
                "wallets",
                "kol-1",
                "confirm",
                None,
                Some(json!({"status": "confirmed"})),
            )
            .await
            .unwrap();
        let b = j
            .record(
                &Person::staff(9),
                "wallets",
                "kol-1",
                "retire",
                Some(json!({"status": "confirmed"})),
                None,
            )
            .await
            .unwrap();
        j.record(&admin, "wallets", "kol-2", "confirm", None, None)
            .await
            .unwrap();
        j.record(&admin, "registry", "kol-1", "publish", None, None)
            .await
            .unwrap();
        assert!(b > a);

        let h = j
            .history(&Person::NOBODY, "wallets", "kol-1")
            .await
            .unwrap();
        assert_eq!(h.iter().map(|e| e.id).collect::<Vec<_>>(), [a, b]);
        assert_eq!(h[0].person_id, Some(7));
        assert_eq!(h[0].role, Role::Admin);
        assert_eq!(h[0].action, "confirm");
        assert_eq!(h[0].before, None);
        assert_eq!(h[0].after, Some(json!({"status": "confirmed"})));
        assert_eq!(h[1].person_id, Some(9));
        assert_eq!(h[1].role, Role::Staff);
        assert_eq!(h[1].before, Some(json!({"status": "confirmed"})));
        assert_eq!(h[1].after, None);
        assert!(h[0].at <= h[1].at);

        assert!(
            j.history(&Person::NOBODY, "wallets", "ghost")
                .await
                .unwrap()
                .is_empty()
        );
        db.close().await;
    }

    #[tokio::test]
    async fn system_person_has_no_id() {
        let (db, j) = journal().await;
        j.record(&Person::SYSTEM, "auth", "1", "create", None, None)
            .await
            .unwrap();
        let h = j.history(&Person::SYSTEM, "auth", "1").await.unwrap();
        assert_eq!(h[0].person_id, None);
        assert_eq!(h[0].role, Role::Admin);
        db.close().await;
    }

    #[tokio::test]
    async fn secrets_are_never_written() {
        let (db, j) = journal().await;
        j.record(
            &Person::admin(1),
            "auth",
            "5",
            "add",
            None,
            Some(json!({
                "login": "anna",
                "password": "hunter2hunter2",
                "nested": [{"Session_Token": "abc", "ok": 1}],
                "api_key": "k"
            })),
        )
        .await
        .unwrap();
        let h = j.history(&Person::admin(1), "auth", "5").await.unwrap();
        let after = h[0].after.clone().unwrap();
        assert_eq!(after["login"], "anna");
        assert_eq!(after["password"], REDACTED);
        assert_eq!(after["nested"][0]["Session_Token"], REDACTED);
        assert_eq!(after["nested"][0]["ok"], 1);
        assert_eq!(after["api_key"], REDACTED);
        assert!(!after.to_string().contains("hunter2"));
        db.close().await;
    }

    #[test]
    fn redact_leaves_plain_values() {
        assert_eq!(
            redact(json!({"a": [1, "x", null]})),
            json!({"a": [1, "x", null]})
        );
        assert_eq!(redact(json!("password")), json!("password"));
    }
}
