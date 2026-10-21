//! The plugin's own tables (schema `kolexplorer`).

use crate::parse::BoardRow;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

/// How long a profile read is reused: reruns on the same day, never the next daily run.
pub const PROFILE_TTL_HOURS: i64 = 20;

/// A cached profile read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CachedProfile {
    pub avg_pos: Option<f64>,
    pub hold_s: Option<f64>,
}

/// The cached read of `slug`, if it is younger than 24 hours at `now`.
pub async fn cached_profile(
    pool: &PgPool,
    slug: &str,
    now: DateTime<Utc>,
) -> Result<Option<CachedProfile>, sqlx::Error> {
    let row: Option<(Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT avg_pos, hold_s FROM kolexplorer.profile_cache WHERE slug = $1 AND fetched_at > $2",
    )
    .bind(slug)
    .bind(now - Duration::hours(PROFILE_TTL_HOURS))
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(avg_pos, hold_s)| CachedProfile { avg_pos, hold_s }))
}

pub async fn cache_profile(
    pool: &PgPool,
    slug: &str,
    at: DateTime<Utc>,
    p: CachedProfile,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO kolexplorer.profile_cache (slug, fetched_at, avg_pos, hold_s)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (slug) DO UPDATE
         SET fetched_at = EXCLUDED.fetched_at, avg_pos = EXCLUDED.avg_pos, hold_s = EXCLUDED.hold_s",
    )
    .bind(slug)
    .bind(at)
    .bind(p.avg_pos)
    .bind(p.hold_s)
    .execute(pool)
    .await?;
    Ok(())
}

/// Replaces the «wallet - X handle» list with the rows of the last read.
pub async fn replace_board(
    pool: &PgPool,
    rows: &[BoardRow],
    read_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM kolexplorer.board_row")
        .execute(&mut *tx)
        .await?;
    for r in rows {
        sqlx::query(
            "INSERT INTO kolexplorer.board_row (wallet, rank, slug, name, x_handle, read_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (wallet) DO NOTHING",
        )
        .bind(&r.address)
        .bind(r.rank as i32)
        .bind(&r.slug)
        .bind(&r.name)
        .bind(&r.x_handle)
        .bind(read_at)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

/// Wallet of the best-ranked row with X handle `x` (case-insensitive, `@` ignored).
pub async fn wallet_by_handle(pool: &PgPool, x: &str) -> Result<Option<String>, sqlx::Error> {
    let x = x.trim().trim_start_matches('@');
    if x.is_empty() {
        return Ok(None);
    }
    sqlx::query_scalar(
        "SELECT wallet FROM kolexplorer.board_row WHERE lower(x_handle) = lower($1)
         ORDER BY rank LIMIT 1",
    )
    .bind(x)
    .fetch_optional(pool)
    .await
}
