//! Snapshots API (layer 1): KOL metrics per day.
//!
//! A snapshot per date: who is on the board and why, one metric row per KOL with its source
//! and fetch time. A snapshot opens, fills and closes; closed past dates do not change, except
//! that a re-run opens the same date again. A missing metric is `None`, never zero. Closing
//! gives a member without a new row its latest earlier row, with the old source and
//! `fetched_at`, so the date shows its age. Day 0 (2026-10-18) is the prototype data.

use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[path = "../../stub/mod.rs"]
pub mod stub;

/// KOL id (registry slug).
pub type KolId = String;

/// State of a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    /// Being filled.
    Open,
    /// Closed: carried forward, ready to rank.
    Closed,
}

/// One snapshot date.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub date: NaiveDate,
    pub status: SnapshotStatus,
    /// Last time it was opened (a re-run opens it again).
    pub opened_at: DateTime<Utc>,
    /// Last time it was closed; `None` while open for the first time.
    pub closed_at: Option<DateTime<Utc>>,
}

/// Where a metric row comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSource {
    /// KOL Explorer leaderboard and profile pages (today's top 100).
    Kolexplorer,
    /// Solana Tracker Data API (KOLs with a published wallet outside the KE top).
    Solanatracker,
}

/// Metrics of one KOL on one date, by the requirements' schema. `None` = no data.
/// Money is USD; win rates are fractions 0..=1.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Realized PnL over 24 hours. KOL Explorer only.
    pub d1: Option<f64>,
    /// Realized PnL over 7 days.
    pub d7: Option<f64>,
    /// Realized PnL over 30 days.
    pub d30: Option<f64>,
    /// Win rate over 7 days. KOL Explorer only.
    pub wr7: Option<f64>,
    /// Win rate over 30 days. KOL Explorer only.
    pub wr30: Option<f64>,
    /// All-time win rate (Solana Tracker); used when `wr30` is `None`, shown with `*`.
    pub wr_all: Option<f64>,
    /// Average position (buy) size.
    pub avg_pos: Option<f64>,
    /// Median hold time, seconds. KOL Explorer profile only.
    pub hold_s: Option<f64>,
    /// Minutes since the last trade, at fetch time.
    pub last_min_at_fetch: Option<i64>,
    /// Traded in the 24 hours before fetch (KE: last trade within 1,440 minutes; ST: the
    /// day's history entry has buys or sells). Feeds streaks.
    pub traded_24h: Option<bool>,
    /// Unrealized PnL.
    pub unreal: Option<f64>,
    /// Total PnL (realized + unrealized).
    pub total: Option<f64>,
    /// Volume over 30 days.
    pub vol30: Option<f64>,
    /// Days with at least one trade in the last 30.
    pub days30: Option<i32>,
    /// Winning trades (tokens).
    pub wins: Option<i32>,
    /// Losing trades (tokens).
    pub losses: Option<i32>,
    /// Place on the source's PnL leaderboard.
    pub pnl_rank: Option<i32>,
    /// In today's Solana Tracker KOL list: the «📡 In top today» badge.
    pub in_top_today: bool,
}

/// One KOL's metric row on one date.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricRow {
    pub date: NaiveDate,
    pub kol: KolId,
    pub source: MetricSource,
    /// When the source was read. Older than the date for carried-forward rows.
    pub fetched_at: DateTime<Utc>,
    pub metrics: Metrics,
}

/// Why a KOL is on a snapshot's board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberReason {
    /// In the day's KOL Explorer top 100.
    Top100,
    /// In our network with a published wallet.
    Network,
}

/// A KOL on a snapshot's board.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub kol: KolId,
    /// Every reason it is a member (one or both).
    pub reasons: Vec<MemberReason>,
    /// Its metric row; `None` while open and not yet written.
    pub row: Option<MetricRow>,
}

/// A snapshot with its members and their metrics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotView {
    pub snapshot: Snapshot,
    pub members: Vec<Member>,
}

/// Snapshots call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotsError {
    /// Snapshots are off for this app or person (stub).
    #[error("snapshots are off")]
    Off,
    /// The snapshots plugin is not built yet.
    #[error("snapshots is not built yet")]
    NotBuilt,
    #[error("no snapshot for {date}")]
    NoSnapshot { date: NaiveDate },
    /// A write to a closed snapshot; open it first.
    #[error("snapshot {date} is closed")]
    Closed { date: NaiveDate },
    #[error("snapshots: {message}")]
    Internal { message: String },
}

/// Snapshots operations.
#[async_trait]
pub trait SnapshotsApi: Send + Sync {
    /// Opens the snapshot for `date`, creating it; a closed one opens again.
    async fn open(&self, person: &Person, date: NaiveDate) -> Result<Snapshot, SnapshotsError>;

    /// Closes the snapshot for `date`. Members without a row get their latest earlier row
    /// with its source and `fetched_at`.
    async fn close(&self, person: &Person, date: NaiveDate) -> Result<Snapshot, SnapshotsError>;

    /// Upserts the row for (`date`, `kol`). The snapshot must be open.
    async fn put_metrics(
        &self,
        person: &Person,
        date: NaiveDate,
        kol: &str,
        metrics: &Metrics,
        source: MetricSource,
        fetched_at: DateTime<Utc>,
    ) -> Result<(), SnapshotsError>;

    /// Replaces every member with `reason` on `date` by `kols`, so dropped KOLs leave also on
    /// a re-run. Other reasons stay. The snapshot must be open.
    async fn set_members(
        &self,
        person: &Person,
        date: NaiveDate,
        reason: MemberReason,
        kols: &[KolId],
    ) -> Result<(), SnapshotsError>;

    /// Copies the members with `reason` from the latest earlier snapshot to `date`.
    /// The snapshot must be open.
    async fn copy_members(
        &self,
        person: &Person,
        date: NaiveDate,
        reason: MemberReason,
    ) -> Result<(), SnapshotsError>;

    /// The snapshot for `date` (open or closed) with members and their rows.
    async fn snapshot(
        &self,
        person: &Person,
        date: NaiveDate,
    ) -> Result<SnapshotView, SnapshotsError>;

    /// Latest closed snapshot date; `None` when there is none.
    async fn latest_date(&self, person: &Person) -> Result<Option<NaiveDate>, SnapshotsError>;

    /// Every closed snapshot date, oldest first.
    async fn dates(&self, person: &Person) -> Result<Vec<NaiveDate>, SnapshotsError>;

    /// The KOL's rows on closed snapshots from `from` to `to` (inclusive), oldest first.
    async fn history(
        &self,
        person: &Person,
        kol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError>;
}
