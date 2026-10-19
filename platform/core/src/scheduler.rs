//! Jobs at fixed UTC times, each run under Postgres advisory locks.
//!
//! A run takes a connection from the pool, detaches it, takes `pg_try_advisory_lock` for the
//! job (and for the shared [`DATA_LOCK`] when the job touches arena data), runs, and closes
//! that connection. Closing ends the session, so the locks end with the run, also on error or
//! panic. A held job lock answers «already running»; a held data lock answers «collection
//! running, try again later» unless the job waits for it. Missed runs are not caught up.
//! «Run now» uses the same locks through [`run_job`] or [`run_locked`].

use crate::{BoxError, BoxFuture};
use chrono::{DateTime, Duration as ChronoDuration, NaiveTime, Utc};
use sqlx::{Connection, PgConnection, PgPool};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::{task::JoinHandle, time::Instant};

/// Lock shared by roster sync, wallet matching and collection: they never overlap.
pub const DATA_LOCK: &str = "arena_data";

const POLL: Duration = Duration::from_millis(250);

/// Advisory lock key for a lock name (64-bit FNV-1a of `arena:<name>`).
pub fn lock_key(name: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in "arena:".bytes().chain(name.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h as i64
}

/// One advisory lock a run needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockSpec {
    pub name: String,
    /// How long to wait for it; zero = try once.
    pub wait: Duration,
}

impl LockSpec {
    /// Try once; busy at once when held.
    pub fn try_once(name: impl Into<String>) -> Self {
        LockSpec {
            name: name.into(),
            wait: Duration::ZERO,
        }
    }

    /// Retry until `wait` has passed.
    pub fn wait(name: impl Into<String>, wait: Duration) -> Self {
        LockSpec {
            name: name.into(),
            wait,
        }
    }

    /// Lock of job `name`.
    pub fn job(name: &str) -> Self {
        Self::try_once(format!("job:{name}"))
    }
}

/// Whether a job takes the shared [`DATA_LOCK`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataLock {
    /// Does not touch arena data.
    No,
    /// Takes it or answers busy at once.
    Try,
    /// Waits for it up to this long, then fails busy (06:00 collection: 30 minutes).
    Wait(Duration),
}

/// Why a locked run did not finish with a value.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// A lock is held by another run.
    #[error("{}", busy_text(lock))]
    Busy { lock: String },
    #[error("lock connection: {0}")]
    Db(#[from] sqlx::Error),
    /// The job returned an error.
    #[error("run failed: {0}")]
    Failed(String),
    #[error("run panicked")]
    Panicked,
}

fn busy_text(lock: &str) -> String {
    if lock == DATA_LOCK {
        "collection running, try again later".to_string()
    } else {
        format!("{} already running", lock.strip_prefix("job:").unwrap_or(lock))
    }
}

async fn take(conn: &mut PgConnection, lock: &LockSpec) -> Result<bool, sqlx::Error> {
    let key = lock_key(&lock.name);
    let deadline = Instant::now() + lock.wait;
    loop {
        let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(key)
            .fetch_one(&mut *conn)
            .await?;
        if got {
            return Ok(true);
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(POLL.min(deadline - now)).await;
    }
}

/// Runs `fut` while holding every lock in `locks` (taken in order) on one detached
/// connection, then closes it. The run lives in its own task: dropping the caller does not
/// release the locks before the run ends.
pub async fn run_locked<T, F>(pool: &PgPool, locks: Vec<LockSpec>, fut: F) -> Result<T, RunError>
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    let pool = pool.clone();
    tokio::spawn(async move {
        let mut conn = pool.acquire().await?.detach();
        for lock in &locks {
            match take(&mut conn, lock).await {
                Ok(true) => {}
                Ok(false) => {
                    let _ = conn.close().await;
                    return Err(RunError::Busy {
                        lock: lock.name.clone(),
                    });
                }
                Err(e) => {
                    let _ = conn.close().await;
                    return Err(e.into());
                }
            }
        }
        let joined = tokio::spawn(fut).await;
        let _ = conn.close().await;
        joined.map_err(|_| RunError::Panicked)
    })
    .await
    .map_err(|_| RunError::Panicked)?
}

/// Body of a job.
pub type JobFn = Arc<dyn Fn() -> BoxFuture<'static, Result<(), BoxError>> + Send + Sync>;

/// A daily job at a fixed UTC time.
#[derive(Clone)]
pub struct Job {
    /// Unique name; its lock is `job:<name>`.
    pub name: &'static str,
    /// UTC time of day.
    pub at: NaiveTime,
    pub data_lock: DataLock,
    pub run: JobFn,
}

impl Job {
    pub fn daily<F, Fut>(name: &'static str, at: NaiveTime, data_lock: DataLock, run: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), BoxError>> + Send + 'static,
    {
        Job {
            name,
            at,
            data_lock,
            run: Arc::new(move || Box::pin(run())),
        }
    }

    /// Locks of one run: the job lock, then the data lock if any.
    pub fn locks(&self) -> Vec<LockSpec> {
        let mut locks = vec![LockSpec::job(self.name)];
        match self.data_lock {
            DataLock::No => {}
            DataLock::Try => locks.push(LockSpec::try_once(DATA_LOCK)),
            DataLock::Wait(d) => locks.push(LockSpec::wait(DATA_LOCK, d)),
        }
        locks
    }
}

/// One run of `job` under its locks. Also used by «Run now».
pub async fn run_job(pool: &PgPool, job: &Job) -> Result<(), RunError> {
    let run = (job.run)();
    run_locked(pool, job.locks(), async move { run.await.map_err(|e| e.to_string()) })
        .await?
        .map_err(RunError::Failed)
}

/// First time strictly after `now` at `at` UTC.
pub fn next_run(now: DateTime<Utc>, at: NaiveTime) -> DateTime<Utc> {
    let today = now.date_naive().and_time(at).and_utc();
    if today > now {
        today
    } else {
        today + ChronoDuration::days(1)
    }
}

/// Starts every job's daily loop.
pub struct Scheduler;

impl Scheduler {
    pub fn start(pool: PgPool, jobs: Vec<Job>) -> Vec<JoinHandle<()>> {
        jobs.into_iter()
            .map(|job| {
                let pool = pool.clone();
                tokio::spawn(async move {
                    loop {
                        let now = Utc::now();
                        let wait = (next_run(now, job.at) - now).to_std().unwrap_or_default();
                        tokio::time::sleep(wait).await;
                        match run_job(&pool, &job).await {
                            Ok(()) => tracing::info!(job = job.name, "job done"),
                            Err(e @ RunError::Busy { .. }) => {
                                tracing::warn!(job = job.name, "job skipped: {e}")
                            }
                            Err(e) => tracing::error!(job = job.name, "job failed: {e}"),
                        }
                    }
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn next_run_is_today_or_tomorrow() {
        let at = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
        let before = Utc.with_ymd_and_hms(2026, 10, 19, 5, 59, 0).unwrap();
        assert_eq!(next_run(before, at), Utc.with_ymd_and_hms(2026, 10, 19, 6, 0, 0).unwrap());
        let exact = Utc.with_ymd_and_hms(2026, 10, 19, 6, 0, 0).unwrap();
        assert_eq!(next_run(exact, at), Utc.with_ymd_and_hms(2026, 10, 20, 6, 0, 0).unwrap());
    }

    #[test]
    fn keys_differ_and_busy_texts() {
        assert_ne!(lock_key("job:collect"), lock_key(DATA_LOCK));
        assert_eq!(lock_key("x"), lock_key("x"));
        let busy = RunError::Busy { lock: DATA_LOCK.into() };
        assert_eq!(busy.to_string(), "collection running, try again later");
        let busy = RunError::Busy { lock: "job:collect".into() };
        assert_eq!(busy.to_string(), "collect already running");
    }

    #[test]
    fn job_locks() {
        let at = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
        let job = Job::daily("collect", at, DataLock::Wait(Duration::from_secs(1800)), || async {
            Ok(())
        });
        assert_eq!(
            job.locks(),
            vec![
                LockSpec::try_once("job:collect"),
                LockSpec::wait(DATA_LOCK, Duration::from_secs(1800))
            ]
        );
    }
}
