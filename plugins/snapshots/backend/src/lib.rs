//! Backend of the `snapshots` plugin (layer 1): KOL metrics per day.
//!
//! A plain store with no routes. Tables `snapshots.snapshot`, `snapshots.member` and
//! `snapshots.metric`. Day 0 (2026-10-18, closed) comes from the seed migration written by
//! `scripts/gen-seed.ts`.

use api_snapshots::{
    KolId, Member, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotStatus,
    SnapshotView, SnapshotsApi, SnapshotsError,
};
use arena_platform::{Ctx, Person, Plugin, PluginEntry, StartError};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgConnection, PgPool, migrate::Migrator};
use std::{collections::BTreeMap, sync::Arc};

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "snapshots";

/// Migrations from `plugins/snapshots/migrations`, run in schema `snapshots`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct SnapshotsPlugin;

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    ctx.apis.provide::<dyn SnapshotsApi>(
        NAME,
        vec![("default", Arc::new(Snapshots::new(ctx.pool.clone())))],
        Arc::new(api_snapshots::stub::Stub),
    )?;
    Ok(Arc::new(SnapshotsPlugin))
}

impl Plugin for SnapshotsPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}

/// Snapshots over their Postgres tables.
pub struct Snapshots {
    pool: PgPool,
}

impl Snapshots {
    pub fn new(pool: PgPool) -> Self {
        Snapshots { pool }
    }
}

fn internal(e: impl std::fmt::Display) -> SnapshotsError {
    SnapshotsError::Internal {
        message: e.to_string(),
    }
}

fn reason_str(r: MemberReason) -> &'static str {
    match r {
        MemberReason::Top100 => "top100",
        MemberReason::Network => "network",
    }
}

fn reason_of(s: &str) -> Result<MemberReason, SnapshotsError> {
    match s {
        "top100" => Ok(MemberReason::Top100),
        "network" => Ok(MemberReason::Network),
        other => Err(internal(format!("unknown member reason {other}"))),
    }
}

fn source_str(s: MetricSource) -> &'static str {
    match s {
        MetricSource::Kolexplorer => "kolexplorer",
        MetricSource::Solanatracker => "solanatracker",
    }
}

fn source_of(s: &str) -> Result<MetricSource, SnapshotsError> {
    match s {
        "kolexplorer" => Ok(MetricSource::Kolexplorer),
        "solanatracker" => Ok(MetricSource::Solanatracker),
        other => Err(internal(format!("unknown metric source {other}"))),
    }
}

#[derive(sqlx::FromRow)]
struct SnapshotRecord {
    date: NaiveDate,
    status: String,
    opened_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
}

impl SnapshotRecord {
    fn into_snapshot(self) -> Result<Snapshot, SnapshotsError> {
        let status = match self.status.as_str() {
            "open" => SnapshotStatus::Open,
            "closed" => SnapshotStatus::Closed,
            other => return Err(internal(format!("unknown snapshot status {other}"))),
        };
        Ok(Snapshot {
            date: self.date,
            status,
            opened_at: self.opened_at,
            closed_at: self.closed_at,
        })
    }
}

/// Metric columns, in table order after (date, kol).
const METRIC_COLUMNS: &str = "source, fetched_at, d1, d7, d30, wr7, wr30, wr_all, avg_pos, \
    hold_s, last_min_at_fetch, traded_24h, unreal, total, vol30, days30, wins, losses, \
    pnl_rank, in_top_today";

/// [`METRIC_COLUMNS`] with each name prefixed (`m.`).
fn metric_columns(prefix: &str) -> String {
    METRIC_COLUMNS
        .split(", ")
        .map(|c| format!("{prefix}{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(sqlx::FromRow)]
struct MetricRecord {
    date: NaiveDate,
    kol: String,
    source: String,
    fetched_at: DateTime<Utc>,
    d1: Option<f64>,
    d7: Option<f64>,
    d30: Option<f64>,
    wr7: Option<f64>,
    wr30: Option<f64>,
    wr_all: Option<f64>,
    avg_pos: Option<f64>,
    hold_s: Option<f64>,
    last_min_at_fetch: Option<i64>,
    traded_24h: Option<bool>,
    unreal: Option<f64>,
    total: Option<f64>,
    vol30: Option<f64>,
    days30: Option<i32>,
    wins: Option<i32>,
    losses: Option<i32>,
    pnl_rank: Option<i32>,
    in_top_today: bool,
}

impl MetricRecord {
    fn into_row(self) -> Result<MetricRow, SnapshotsError> {
        Ok(MetricRow {
            date: self.date,
            kol: self.kol,
            source: source_of(&self.source)?,
            fetched_at: self.fetched_at,
            metrics: Metrics {
                d1: self.d1,
                d7: self.d7,
                d30: self.d30,
                wr7: self.wr7,
                wr30: self.wr30,
                wr_all: self.wr_all,
                avg_pos: self.avg_pos,
                hold_s: self.hold_s,
                last_min_at_fetch: self.last_min_at_fetch,
                traded_24h: self.traded_24h,
                unreal: self.unreal,
                total: self.total,
                vol30: self.vol30,
                days30: self.days30,
                wins: self.wins,
                losses: self.losses,
                pnl_rank: self.pnl_rank,
                in_top_today: self.in_top_today,
            },
        })
    }
}

/// The snapshot row for `date`, locked for the rest of the transaction.
async fn lock(conn: &mut PgConnection, date: NaiveDate) -> Result<Snapshot, SnapshotsError> {
    let rec: Option<SnapshotRecord> = sqlx::query_as(
        "SELECT date, status, opened_at, closed_at FROM snapshots.snapshot \
         WHERE date = $1 FOR UPDATE",
    )
    .bind(date)
    .fetch_optional(conn)
    .await
    .map_err(internal)?;
    rec.ok_or(SnapshotsError::NoSnapshot { date })?
        .into_snapshot()
}

/// Locks the snapshot for `date` and checks it is open.
async fn lock_open(conn: &mut PgConnection, date: NaiveDate) -> Result<(), SnapshotsError> {
    match lock(conn, date).await?.status {
        SnapshotStatus::Open => Ok(()),
        SnapshotStatus::Closed => Err(SnapshotsError::Closed { date }),
    }
}

#[async_trait]
impl SnapshotsApi for Snapshots {
    async fn open(&self, _: &Person, date: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        // A new date opens; an open one stays as it is; a closed one opens again.
        let rec: Option<SnapshotRecord> = sqlx::query_as(
            "INSERT INTO snapshots.snapshot (date, status, opened_at) VALUES ($1, 'open', now()) \
             ON CONFLICT (date) DO UPDATE SET status = 'open', opened_at = now() \
             WHERE snapshots.snapshot.status = 'closed' \
             RETURNING date, status, opened_at, closed_at",
        )
        .bind(date)
        .fetch_optional(&self.pool)
        .await
        .map_err(internal)?;
        let rec = match rec {
            Some(rec) => rec,
            None => sqlx::query_as(
                "SELECT date, status, opened_at, closed_at FROM snapshots.snapshot WHERE date = $1",
            )
            .bind(date)
            .fetch_one(&self.pool)
            .await
            .map_err(internal)?,
        };
        rec.into_snapshot()
    }

    async fn close(&self, _: &Person, date: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        let mut tx = self.pool.begin().await.map_err(internal)?;
        lock(&mut tx, date).await?;
        // Members without a row get their latest earlier row, source and fetched_at kept.
        sqlx::query(&format!(
            "INSERT INTO snapshots.metric (date, kol, {METRIC_COLUMNS}) \
             SELECT DISTINCT ON (m.kol) $1, m.kol, {cols} \
             FROM snapshots.metric m \
             WHERE m.date < $1 \
               AND m.kol IN (SELECT kol FROM snapshots.member WHERE date = $1) \
               AND NOT EXISTS (SELECT 1 FROM snapshots.metric x WHERE x.date = $1 AND x.kol = m.kol) \
             ORDER BY m.kol, m.date DESC",
            cols = metric_columns("m.")
        ))
        .bind(date)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        let rec: SnapshotRecord = sqlx::query_as(
            "UPDATE snapshots.snapshot SET status = 'closed', closed_at = now() WHERE date = $1 \
             RETURNING date, status, opened_at, closed_at",
        )
        .bind(date)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        rec.into_snapshot()
    }

    async fn put_metrics(
        &self,
        _: &Person,
        date: NaiveDate,
        kol: &str,
        m: &Metrics,
        source: MetricSource,
        fetched_at: DateTime<Utc>,
    ) -> Result<(), SnapshotsError> {
        let mut tx = self.pool.begin().await.map_err(internal)?;
        lock_open(&mut tx, date).await?;
        let updates = METRIC_COLUMNS
            .split(", ")
            .map(|c| format!("{0} = EXCLUDED.{0}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        sqlx::query(&format!(
            "INSERT INTO snapshots.metric (date, kol, {METRIC_COLUMNS}) VALUES ($1, $2, $3, $4, \
             $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22) \
             ON CONFLICT (date, kol) DO UPDATE SET {updates}"
        ))
        .bind(date)
        .bind(kol)
        .bind(source_str(source))
        .bind(fetched_at)
        .bind(m.d1)
        .bind(m.d7)
        .bind(m.d30)
        .bind(m.wr7)
        .bind(m.wr30)
        .bind(m.wr_all)
        .bind(m.avg_pos)
        .bind(m.hold_s)
        .bind(m.last_min_at_fetch)
        .bind(m.traded_24h)
        .bind(m.unreal)
        .bind(m.total)
        .bind(m.vol30)
        .bind(m.days30)
        .bind(m.wins)
        .bind(m.losses)
        .bind(m.pnl_rank)
        .bind(m.in_top_today)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        tx.commit().await.map_err(internal)
    }

    async fn set_members(
        &self,
        _: &Person,
        date: NaiveDate,
        reason: MemberReason,
        kols: &[KolId],
    ) -> Result<(), SnapshotsError> {
        let mut tx = self.pool.begin().await.map_err(internal)?;
        lock_open(&mut tx, date).await?;
        sqlx::query("DELETE FROM snapshots.member WHERE date = $1 AND reason = $2")
            .bind(date)
            .bind(reason_str(reason))
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        sqlx::query(
            "INSERT INTO snapshots.member (date, kol, reason) \
             SELECT $1, k, $2 FROM unnest($3::text[]) AS k ON CONFLICT DO NOTHING",
        )
        .bind(date)
        .bind(reason_str(reason))
        .bind(kols)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        tx.commit().await.map_err(internal)
    }

    async fn copy_members(
        &self,
        _: &Person,
        date: NaiveDate,
        reason: MemberReason,
    ) -> Result<(), SnapshotsError> {
        let mut tx = self.pool.begin().await.map_err(internal)?;
        lock_open(&mut tx, date).await?;
        let reason = reason_str(reason);
        sqlx::query("DELETE FROM snapshots.member WHERE date = $1 AND reason = $2")
            .bind(date)
            .bind(reason)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        sqlx::query(
            "INSERT INTO snapshots.member (date, kol, reason) \
             SELECT $1, kol, reason FROM snapshots.member \
             WHERE reason = $2 AND date = \
               (SELECT max(date) FROM snapshots.snapshot WHERE date < $1)",
        )
        .bind(date)
        .bind(reason)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        tx.commit().await.map_err(internal)
    }

    async fn snapshot(&self, _: &Person, date: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        let rec: Option<SnapshotRecord> = sqlx::query_as(
            "SELECT date, status, opened_at, closed_at FROM snapshots.snapshot WHERE date = $1",
        )
        .bind(date)
        .fetch_optional(&self.pool)
        .await
        .map_err(internal)?;
        let snapshot = rec
            .ok_or(SnapshotsError::NoSnapshot { date })?
            .into_snapshot()?;
        let members: Vec<(String, String)> = sqlx::query_as(
            "SELECT kol, reason FROM snapshots.member WHERE date = $1 \
             ORDER BY kol, CASE reason WHEN 'top100' THEN 0 ELSE 1 END",
        )
        .bind(date)
        .fetch_all(&self.pool)
        .await
        .map_err(internal)?;
        let rows: Vec<MetricRecord> = sqlx::query_as(&format!(
            "SELECT date, kol, {METRIC_COLUMNS} FROM snapshots.metric m WHERE date = $1 \
             AND EXISTS (SELECT 1 FROM snapshots.member x WHERE x.date = $1 AND x.kol = m.kol)"
        ))
        .bind(date)
        .fetch_all(&self.pool)
        .await
        .map_err(internal)?;
        let mut by_kol: BTreeMap<String, Member> = BTreeMap::new();
        for (kol, reason) in members {
            by_kol
                .entry(kol.clone())
                .or_insert_with(|| Member {
                    kol,
                    reasons: Vec::new(),
                    row: None,
                })
                .reasons
                .push(reason_of(&reason)?);
        }
        for r in rows {
            let row = r.into_row()?;
            if let Some(m) = by_kol.get_mut(&row.kol) {
                m.row = Some(row);
            }
        }
        Ok(SnapshotView {
            snapshot,
            members: by_kol.into_values().collect(),
        })
    }

    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        sqlx::query_scalar("SELECT max(date) FROM snapshots.snapshot WHERE status = 'closed'")
            .fetch_one(&self.pool)
            .await
            .map_err(internal)
    }

    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        sqlx::query_scalar(
            "SELECT date FROM snapshots.snapshot WHERE status = 'closed' ORDER BY date",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(internal)
    }

    async fn history(
        &self,
        _: &Person,
        kol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        let rows: Vec<MetricRecord> = sqlx::query_as(&format!(
            "SELECT m.date, m.kol, {cols} FROM snapshots.metric m \
             JOIN snapshots.snapshot s ON s.date = m.date AND s.status = 'closed' \
             WHERE m.kol = $1 AND m.date BETWEEN $2 AND $3 ORDER BY m.date",
            cols = metric_columns("m.")
        ))
        .bind(kol)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(internal)?;
        rows.into_iter().map(MetricRecord::into_row).collect()
    }
}
