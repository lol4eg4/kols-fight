//! Arena Score API (layer 1): Score, tier, badges and rank of every KOL on a snapshot.
//!
//! Formula from the requirements, changed only with the owner's consent:
//! `round(100 × (0.40·Profit + 0.25·Accuracy + 0.20·Form + 0.15·Activity))`.
//! - Profit: `d30`, log scale $1,000..$500,000.
//! - Accuracy: `wr30`, or `wr_all` when `wr30` is `None`; linear 30%..80%; counts only when
//!   `d30 > 0`.
//! - Form: `d7`, log scale $500..$100,000.
//! - Activity: `last_min_at_fetch`; up to 60 minutes full, 30 days zero, log scale between.
//!
//! A part without data scores 0 and is marked «no data». Rank: Score desc, then `d30` desc,
//! then name asc (only for a stable order).

use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

pub use api_snapshots::Metrics;

#[path = "../../stub/mod.rs"]
pub mod stub;

/// KOL id (registry slug).
pub type KolId = String;

/// The four parts of Arena Score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    /// Weight 0.40; raw = `d30`.
    Profit,
    /// Weight 0.25; raw = win rate.
    Accuracy,
    /// Weight 0.20; raw = `d7`.
    Form,
    /// Weight 0.15; raw = minutes since the last trade.
    Activity,
}

impl PartKind {
    /// In formula order.
    pub const ALL: [PartKind; 4] = [
        PartKind::Profit,
        PartKind::Accuracy,
        PartKind::Form,
        PartKind::Activity,
    ];
}

/// Whether a part counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartStatus {
    Counted,
    /// No data: 0 points, shown as «no data».
    NoData,
    /// Accuracy only: `d30` is not above zero, so the win rate does not count (0 points).
    NeedsProfit,
}

/// One part of the breakdown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub kind: PartKind,
    /// Raw value: USD for profit and form, fraction 0..=1 for accuracy, minutes for activity.
    pub raw: Option<f64>,
    /// Accuracy only: `raw` is the all-time win rate (shown with `*`).
    pub all_time: bool,
    /// Share of the scale, 0..=1 (0 when not counted).
    pub share: f64,
    /// Points: `100 × weight × share`.
    pub points: f64,
    pub status: PartStatus,
}

/// Tier by Score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// 70 and up.
    Goat,
    /// 50 to 69.
    Diamond,
    /// 35 to 49.
    Sendoor,
    /// Below 35.
    Degen,
}

/// Badges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Badge {
    /// Sniper: `wr30` at least 65% (never from the all-time win rate).
    Sniper,
    /// On fire: `d7` at least $20K.
    OnFire,
    /// Whale: average position at least $1K.
    Whale,
    /// Scalper: hold up to 15 seconds.
    Scalper,
    /// HODL: hold at least one hour.
    Hodl,
    /// Live: a trade within the last hour.
    Live,
    /// «📡 In top today»: in today's Solana Tracker KOL list.
    InTopToday,
}

/// Score of one set of metrics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoreCard {
    /// 0..=100.
    pub score: u8,
    pub tier: Tier,
    /// The four parts in [`PartKind::ALL`] order.
    pub parts: Vec<Part>,
    pub badges: Vec<Badge>,
}

/// One KOL in a ranking.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankedKol {
    pub kol: KolId,
    /// 1-based place.
    pub rank: u32,
    pub card: ScoreCard,
}

/// Stored result of a closed snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ranking {
    pub date: NaiveDate,
    pub ranked_at: DateTime<Utc>,
    /// By rank.
    pub rows: Vec<RankedKol>,
}

/// Score call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ScoreError {
    /// Score is off for this app or person (stub).
    #[error("score is off")]
    Off,
    /// The score plugin is not built yet.
    #[error("score is not built yet")]
    NotBuilt,
    #[error("no snapshot for {date}")]
    NoSnapshot { date: NaiveDate },
    /// `rank` needs a closed snapshot.
    #[error("snapshot {date} is not closed")]
    NotClosed { date: NaiveDate },
    #[error("snapshot {date} is not ranked")]
    NotRanked { date: NaiveDate },
    #[error("score: {message}")]
    Internal { message: String },
}

/// Arena Score operations.
#[async_trait]
pub trait ScoreApi: Send + Sync {
    /// Parts (raw value, share of scale, points), Score, tier and badges for `metrics`.
    async fn score(&self, person: &Person, metrics: &Metrics) -> Result<ScoreCard, ScoreError>;

    /// Computes and stores the ranking of the closed snapshot for `date`.
    async fn rank(&self, person: &Person, date: NaiveDate) -> Result<(), ScoreError>;

    /// Stored ranking for `date`: KOLs with Score, tier, badges and rank.
    async fn ranking(&self, person: &Person, date: NaiveDate) -> Result<Ranking, ScoreError>;
}
