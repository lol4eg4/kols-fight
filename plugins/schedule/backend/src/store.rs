//! The plugin's own tables (schema `schedule`).

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgPool;

/// One step of a run, as stored.
#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct StepRow {
    pub step: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub rows: Option<i32>,
    pub error: Option<String>,
    pub note: Option<String>,
}

/// One run, as stored.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct RunRow {
    pub id: i64,
    pub date: NaiveDate,
    pub trigger: String,
    pub person_id: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub error: Option<String>,
}

/// The latest step of one source with a given outcome, with the run's date.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct SourceStep {
    pub run_id: i64,
    pub date: NaiveDate,
    pub status: String,
    pub finished_at: DateTime<Utc>,
    pub rows: Option<i32>,
    pub error: Option<String>,
    pub note: Option<String>,
}

pub async fn insert_run(
    pool: &PgPool,
    date: NaiveDate,
    trigger: &str,
    person_id: Option<i64>,
    now: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO schedule.run (date, trigger, person_id, started_at, status)
         VALUES ($1, $2, $3, $4, 'running') RETURNING id",
    )
    .bind(date)
    .bind(trigger)
    .bind(person_id)
    .bind(now)
    .fetch_one(pool)
    .await
}

/// A run that never started: stored finished and failed at once.
pub async fn insert_failed_run(
    pool: &PgPool,
    date: NaiveDate,
    trigger: &str,
    now: DateTime<Utc>,
    error: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO schedule.run (date, trigger, started_at, finished_at, status, error)
         VALUES ($1, $2, $3, $3, 'failed', $4) RETURNING id",
    )
    .bind(date)
    .bind(trigger)
    .bind(now)
    .bind(error)
    .fetch_one(pool)
    .await
}

pub async fn finish_run(
    pool: &PgPool,
    id: i64,
    status: &str,
    error: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE schedule.run SET status = $2, error = $3, finished_at = $4 WHERE id = $1")
        .bind(id)
        .bind(status)
        .bind(error)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub struct NewStep<'a> {
    pub run_id: i64,
    pub step: &'a str,
    pub seq: i32,
    pub status: &'a str,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub rows: Option<i32>,
    pub error: Option<&'a str>,
    pub note: Option<&'a str>,
}

pub async fn insert_step(pool: &PgPool, s: &NewStep<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO schedule.step (run_id, step, seq, status, started_at, finished_at, rows, error, note)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(s.run_id)
    .bind(s.step)
    .bind(s.seq)
    .bind(s.status)
    .bind(s.started_at)
    .bind(s.finished_at)
    .bind(s.rows)
    .bind(s.error)
    .bind(s.note)
    .execute(pool)
    .await?;
    Ok(())
}

/// Marks every run still `running` as interrupted and returns their dates.
pub async fn mark_interrupted(
    pool: &PgPool,
    now: DateTime<Utc>,
) -> Result<Vec<NaiveDate>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE schedule.run
         SET status = 'interrupted', finished_at = $1, error = 'the server stopped during the run'
         WHERE status = 'running' RETURNING date",
    )
    .bind(now)
    .fetch_all(pool)
    .await
}

pub async fn any_running(pool: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM schedule.run WHERE status = 'running')")
        .fetch_one(pool)
        .await
}

pub async fn get_run(pool: &PgPool, id: i64) -> Result<Option<RunRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, date, trigger, person_id, started_at, finished_at, status, error
         FROM schedule.run WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// The newest runs, newest first.
pub async fn list_runs(pool: &PgPool, limit: i64) -> Result<Vec<RunRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, date, trigger, person_id, started_at, finished_at, status, error
         FROM schedule.run ORDER BY started_at DESC, id DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Steps of a run in order.
pub async fn steps_of(pool: &PgPool, run_id: i64) -> Result<Vec<StepRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT step, status, started_at, finished_at, rows, error, note
         FROM schedule.step WHERE run_id = $1 ORDER BY seq",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
}

/// The latest step of `step` whose status is one of `statuses`.
pub async fn latest_step(
    pool: &PgPool,
    step: &str,
    statuses: &[&str],
) -> Result<Option<SourceStep>, sqlx::Error> {
    let statuses: Vec<String> = statuses.iter().map(|s| s.to_string()).collect();
    sqlx::query_as(
        "SELECT s.run_id, r.date, s.status, s.finished_at, s.rows, s.error, s.note
         FROM schedule.step s JOIN schedule.run r ON r.id = s.run_id
         WHERE s.step = $1 AND s.status = ANY($2)
         ORDER BY s.finished_at DESC, s.run_id DESC LIMIT 1",
    )
    .bind(step)
    .bind(statuses)
    .fetch_optional(pool)
    .await
}
