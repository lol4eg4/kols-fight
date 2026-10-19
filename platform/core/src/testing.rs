//! Test helpers.

use crate::migrate::migrate_plugin;
use sqlx::{
    Connection, PgConnection, PgPool,
    migrate::Migrator,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// Prefix of every test database name.
pub const PREFIX: &str = "kol_fight_test_";

/// A throwaway database `kol_fight_test_<random>`, created from the board Postgres env
/// (`$BOARD_PG_ENV`, else `~/.config/board/postgres.env`: PGHOST, PGPORT, PGUSER,
/// PGPASSWORD). Dropped by [`TestDb::close`] or on drop. Touches no other database.
pub struct TestDb {
    name: String,
    admin: PgConnectOptions,
    url: String,
    pool: PgPool,
    dropped: bool,
}

fn env_file() -> PathBuf {
    match std::env::var_os("BOARD_PG_ENV") {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join(".config/board/postgres.env"),
    }
}

fn read_env(path: &PathBuf) -> Result<HashMap<String, String>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("export ").unwrap_or(l).split_once('='))
        .map(|(k, v)| {
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(v);
            (k.trim().to_string(), v.to_string())
        })
        .collect())
}

fn random_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let seed = (nanos, std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
    format!("{:016x}", RandomState::new().hash_one(seed))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn drop_database(admin: &PgConnectOptions, name: &str) -> Result<(), sqlx::Error> {
    assert!(name.starts_with(PREFIX), "TestDb drops only its own databases");
    let mut conn = PgConnection::connect_with(admin).await?;
    sqlx::raw_sql(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
        .execute(&mut conn)
        .await?;
    conn.close().await
}

impl TestDb {
    /// Creates an empty test database. Panics with the reason when it cannot.
    pub async fn new() -> Self {
        match Self::try_new().await {
            Ok(db) => db,
            Err(e) => panic!("TestDb: {e}"),
        }
    }

    /// Creates a test database and runs each named plugin's migrations in its schema.
    pub async fn with_plugins(plugins: &[(&str, &'static Migrator)]) -> Self {
        let db = Self::new().await;
        for (schema, migrator) in plugins {
            db.migrate(schema, migrator).await;
        }
        db
    }

    pub async fn try_new() -> Result<Self, String> {
        let path = env_file();
        let env = read_env(&path)?;
        let get = |k: &str| {
            env.get(k)
                .filter(|v| !v.is_empty())
                .cloned()
                .ok_or_else(|| format!("{k} not configured in {}", path.display()))
        };
        let (host, port, user, password) =
            (get("PGHOST")?, get("PGPORT")?, get("PGUSER")?, get("PGPASSWORD")?);
        let port: u16 = port.parse().map_err(|_| "PGPORT is not a port".to_string())?;
        let server = PgConnectOptions::new()
            .host(&host)
            .port(port)
            .username(&user)
            .password(&password);
        let admin = server.clone().database("postgres");
        let name = format!("{PREFIX}{}", random_suffix());
        let mut conn = PgConnection::connect_with(&admin)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        sqlx::raw_sql(&format!("CREATE DATABASE {name}"))
            .execute(&mut conn)
            .await
            .map_err(|e| format!("create database: {e}"))?;
        let _ = conn.close().await;
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(server.database(&name))
            .await
            .map_err(|e| format!("connect {name}: {e}"))?;
        let url = format!(
            "postgres://{}:{}@{host}:{port}/{name}",
            encode(&user),
            encode(&password)
        );
        Ok(TestDb {
            name,
            admin,
            url,
            pool,
            dropped: false,
        })
    }

    /// Runs `migrator` in schema `schema` (as the server does for a plugin).
    pub async fn migrate(&self, schema: &str, migrator: &Migrator) {
        if let Err(e) = migrate_plugin(&self.pool, schema, migrator).await {
            panic!("TestDb: migrate {schema}: {e}");
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Connection URL (holds the password: never print it).
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Closes the pool and drops the database.
    pub async fn close(mut self) {
        self.pool.close().await;
        if let Err(e) = drop_database(&self.admin, &self.name).await {
            panic!("TestDb: drop {}: {e}", self.name);
        }
        self.dropped = true;
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if self.dropped {
            return;
        }
        let (admin, name) = (self.admin.clone(), self.name.clone());
        // Drop runs inside the test's runtime; use a fresh one on another thread.
        let done = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            rt.block_on(drop_database(&admin, &name))
                .map_err(|e| e.to_string())
        })
        .join();
        if !matches!(done, Ok(Ok(()))) {
            eprintln!("TestDb: could not drop {}", self.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_file_parsing_and_encoding() {
        let dir = std::env::temp_dir().join(format!("testdb-env-{}", random_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pg.env");
        std::fs::write(&path, "# c\nexport PGHOST=localhost\nPGPORT='5432'\nPGUSER=\"u\"\nPGPASSWORD=p@ss/w\n").unwrap();
        let env = read_env(&path).unwrap();
        assert_eq!(env["PGHOST"], "localhost");
        assert_eq!(env["PGPORT"], "5432");
        assert_eq!(env["PGUSER"], "u");
        assert_eq!(encode(&env["PGPASSWORD"]), "p%40ss%2Fw");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
        assert_ne!(random_suffix(), random_suffix());
    }
}
