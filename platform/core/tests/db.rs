//! Tests against a real Postgres: TestDb, per-plugin migrations, scheduler locks.
//! Each test makes its own `kol_fight_test_<random>` database and drops it.

use arena_platform::{
    BoxError, DataLock, Job, LockSpec, RunError,
    migrate::{migrate_plugin, pending_migrations},
    scheduler::{DATA_LOCK, run_job, run_locked},
    testing::{PREFIX, TestDb},
};
use chrono::NaiveTime;
use sqlx::{PgPool, migrate::Migrator};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::oneshot;

async fn db_exists(pool: &PgPool, name: &str) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_db_is_created_and_dropped() {
    let watcher = TestDb::new().await;
    let db = TestDb::new().await;
    let name = db.name().to_string();
    assert!(name.starts_with(PREFIX), "{name}");
    assert!(db_exists(watcher.pool(), &name).await);
    let one: i32 = sqlx::query_scalar("SELECT 1")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(one, 1);
    db.close().await;
    assert!(!db_exists(watcher.pool(), &name).await);

    // Dropping without close also drops the database.
    let db = TestDb::new().await;
    let name = db.name().to_string();
    drop(db);
    assert!(!db_exists(watcher.pool(), &name).await);
    watcher.close().await;
}

fn migrations_dir(tag: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "arena-migrations-{tag}-{}-{}",
        std::process::id(),
        rand_tag()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, sql) in files {
        std::fs::write(dir.join(name), sql).unwrap();
    }
    dir
}

fn rand_tag() -> u64 {
    use std::hash::{BuildHasher, RandomState};
    RandomState::new().hash_one(std::time::SystemTime::now())
}

fn remove_dir(dir: &PathBuf, files: &[(&str, &str)]) {
    for (name, _) in files {
        std::fs::remove_file(dir.join(name)).unwrap();
    }
    std::fs::remove_dir(dir).unwrap();
}

async fn table_schemas(pool: &PgPool, table: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT table_schema::text FROM information_schema.tables WHERE table_name = $1 \
         ORDER BY table_schema",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_migrations_run_in_own_schema() {
    let alpha_files = [
        (
            "0001_schema.sql",
            "CREATE TABLE items (id BIGINT PRIMARY KEY);",
        ),
        (
            "0002_more.sql",
            "CREATE TABLE notes (id BIGINT PRIMARY KEY);",
        ),
    ];
    // Same unqualified table name in another plugin: no clash, each in its schema.
    let beta_files = [(
        "0001_schema.sql",
        "CREATE TABLE items (id TEXT PRIMARY KEY);",
    )];
    let alpha_dir = migrations_dir("alpha", &alpha_files);
    let beta_dir = migrations_dir("beta", &beta_files);
    let alpha = Migrator::new(alpha_dir.as_path()).await.unwrap();
    let beta = Migrator::new(beta_dir.as_path()).await.unwrap();

    let db = TestDb::new().await;
    let pool = db.pool();
    assert_eq!(pending_migrations(pool, "alpha", &alpha).await.unwrap(), 2);
    assert_eq!(pending_migrations(pool, "beta", &beta).await.unwrap(), 1);

    migrate_plugin(pool, "alpha", &alpha).await.unwrap();
    db.migrate("beta", &beta).await;
    // Running again is a no-op.
    migrate_plugin(pool, "alpha", &alpha).await.unwrap();

    assert_eq!(pending_migrations(pool, "alpha", &alpha).await.unwrap(), 0);
    assert_eq!(pending_migrations(pool, "beta", &beta).await.unwrap(), 0);
    assert_eq!(table_schemas(pool, "items").await, ["alpha", "beta"]);
    assert_eq!(table_schemas(pool, "notes").await, ["alpha"]);
    assert_eq!(
        table_schemas(pool, "_sqlx_migrations").await,
        ["alpha", "beta"]
    );

    // The pool's search_path is untouched: no connection was left pointing at a plugin schema.
    for _ in 0..12 {
        let path: String = sqlx::query_scalar("SHOW search_path")
            .fetch_one(pool)
            .await
            .unwrap();
        assert!(!path.contains("alpha") && !path.contains("beta"), "{path}");
    }

    // A bad schema name is refused before touching the database.
    assert!(migrate_plugin(pool, "public", &alpha).await.is_err());
    assert!(migrate_plugin(pool, "Bad-Name", &alpha).await.is_err());

    db.close().await;
    remove_dir(&alpha_dir, &alpha_files);
    remove_dir(&beta_dir, &beta_files);
}

#[tokio::test(flavor = "multi_thread")]
async fn locked_runs_never_overlap() {
    let db = TestDb::new().await;
    let pool = db.pool().clone();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let first = tokio::spawn({
        let pool = pool.clone();
        async move {
            run_locked(&pool, vec![LockSpec::job("collect")], async move {
                started_tx.send(()).unwrap();
                release_rx.await.unwrap();
                7
            })
            .await
        }
    });
    started_rx.await.unwrap();

    let second = run_locked(&pool, vec![LockSpec::job("collect")], async { 8 }).await;
    match second {
        Err(e @ RunError::Busy { .. }) => assert_eq!(e.to_string(), "collect already running"),
        other => panic!("expected busy, got {other:?}"),
    }
    // Another job's lock is free.
    assert_eq!(
        run_locked(&pool, vec![LockSpec::job("other")], async { 9 })
            .await
            .unwrap(),
        9
    );

    release_tx.send(()).unwrap();
    assert_eq!(first.await.unwrap().unwrap(), 7);
    // After the run ends the lock is free again.
    assert_eq!(
        run_locked(&pool, vec![LockSpec::job("collect")], async { 10 })
            .await
            .unwrap(),
        10
    );
    db.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn data_lock_is_shared_and_waits() {
    let db = TestDb::new().await;
    let pool = db.pool().clone();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let holder = tokio::spawn({
        let pool = pool.clone();
        async move {
            run_locked(&pool, vec![LockSpec::try_once(DATA_LOCK)], async move {
                started_tx.send(()).unwrap();
                release_rx.await.unwrap();
            })
            .await
        }
    });
    started_rx.await.unwrap();

    // A job with DataLock::Try answers busy at once with the collection text.
    let job = Job::daily("radar", NaiveTime::MIN, DataLock::Try, || async { Ok(()) });
    match run_job(&pool, &job).await {
        Err(e @ RunError::Busy { .. }) => {
            assert_eq!(e.to_string(), "collection running, try again later")
        }
        other => panic!("expected busy, got {other:?}"),
    }

    // A job with DataLock::Wait waits for the holder.
    let ran = Arc::new(AtomicUsize::new(0));
    let job = Job::daily(
        "collect",
        NaiveTime::MIN,
        DataLock::Wait(Duration::from_secs(10)),
        {
            let ran = ran.clone();
            move || {
                let ran = ran.clone();
                async move {
                    ran.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        },
    );
    let waiter = tokio::spawn({
        let pool = pool.clone();
        async move { run_job(&pool, &job).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        ran.load(Ordering::SeqCst),
        0,
        "ran while the data lock was held"
    );
    release_tx.send(()).unwrap();
    holder.await.unwrap().unwrap();
    waiter.await.unwrap().unwrap();
    assert_eq!(ran.load(Ordering::SeqCst), 1);

    // A short wait runs out and answers busy.
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let holder = tokio::spawn({
        let pool = pool.clone();
        async move {
            run_locked(&pool, vec![LockSpec::try_once(DATA_LOCK)], async move {
                started_tx.send(()).unwrap();
                release_rx.await.unwrap();
            })
            .await
        }
    });
    started_rx.await.unwrap();
    let short = run_locked(
        &pool,
        vec![LockSpec::wait(DATA_LOCK, Duration::from_millis(300))],
        async {},
    )
    .await;
    assert!(matches!(short, Err(RunError::Busy { .. })), "{short:?}");
    release_tx.send(()).unwrap();
    holder.await.unwrap().unwrap();
    db.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn lock_is_released_after_failing_or_panicking_run() {
    let db = TestDb::new().await;
    let pool = db.pool().clone();

    let failing = Job::daily("collect", NaiveTime::MIN, DataLock::Try, || async {
        Err::<(), BoxError>("source down".into())
    });
    match run_job(&pool, &failing).await {
        Err(RunError::Failed(m)) => assert_eq!(m, "source down"),
        other => panic!("expected failed, got {other:?}"),
    }

    let panicking = Job::daily("collect", NaiveTime::MIN, DataLock::Try, || async {
        panic!("boom");
    });
    assert!(matches!(
        run_job(&pool, &panicking).await,
        Err(RunError::Panicked)
    ));

    // Both the job lock and the data lock are free again.
    let ok = Job::daily("collect", NaiveTime::MIN, DataLock::Try, || async {
        Ok(())
    });
    run_job(&pool, &ok).await.unwrap();
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' \
         AND database = (SELECT oid FROM pg_database WHERE datname = current_database())",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(held, 0);
    db.close().await;
}
