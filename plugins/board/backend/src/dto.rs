//! Public DTOs of the board routes. Only these types reach the client; each field is on the
//! leak test's allowlist. Never serialize an API type of another plugin here directly.

use api_score::{Badge, PartKind, PartStatus, Tier};
use api_snapshots::{MetricSource, Metrics};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

/// Public metric fields of one KOL. `None` = no data («—»). USD; win rates 0..=1.
#[derive(Clone, Debug, Serialize)]
pub struct PublicMetrics {
    pub d1: Option<f64>,
    pub d7: Option<f64>,
    pub d30: Option<f64>,
    pub wr7: Option<f64>,
    pub wr30: Option<f64>,
    /// All-time win rate; shown with `*` when `wr30` is `None`.
    pub wr_all: Option<f64>,
    pub avg_pos: Option<f64>,
    /// Median hold, seconds.
    pub hold_s: Option<f64>,
    /// Minutes since the last trade at fetch.
    pub last_min_at_fetch: Option<i64>,
    pub unreal: Option<f64>,
    pub total: Option<f64>,
    pub vol30: Option<f64>,
    pub days30: Option<i32>,
}

impl From<&Metrics> for PublicMetrics {
    fn from(m: &Metrics) -> Self {
        PublicMetrics {
            d1: m.d1,
            d7: m.d7,
            d30: m.d30,
            wr7: m.wr7,
            wr30: m.wr30,
            wr_all: m.wr_all,
            avg_pos: m.avg_pos,
            hold_s: m.hold_s,
            last_min_at_fetch: m.last_min_at_fetch,
            unreal: m.unreal,
            total: m.total,
            vol30: m.vol30,
            days30: m.days30,
        }
    }
}

/// One leaderboard row.
#[derive(Clone, Debug, Serialize)]
pub struct BoardRow {
    pub kol: String,
    pub name: String,
    /// Without `@`.
    pub x_handle: Option<String>,
    pub rank: u32,
    pub score: u8,
    pub tier: Tier,
    pub badges: Vec<Badge>,
    /// In the day's KOL Explorer top 100 (the «Top-100» tab).
    pub top100: bool,
    /// API slot `board/in_network`.
    pub in_network: bool,
    /// `None` when the KOL has no metric row on this snapshot.
    pub source: Option<MetricSource>,
    pub fetched_at: Option<DateTime<Utc>>,
    /// `now - fetched_at > 48h`: «data stale».
    pub stale: bool,
    pub metrics: Option<PublicMetrics>,
}

/// Hero numbers.
#[derive(Clone, Debug, Serialize)]
pub struct Stats {
    /// KOLs on the board (M in «Only N of M tracked KOLs are up over 30 days»).
    pub tracked: u32,
    /// KOLs with `d30 > 0` (N).
    pub up_30d: u32,
    /// KOLs whose last trade was at most 60 minutes before fetch.
    pub traded_last_hour: u32,
    /// Sum of `d30` over the board, USD.
    pub total_d30: f64,
}

/// Latest fetch time of one source on the snapshot (the line under the table).
#[derive(Clone, Debug, Serialize)]
pub struct SourceDate {
    pub source: MetricSource,
    pub fetched_at: DateTime<Utc>,
    /// KOLs with rows from this source.
    pub kols: u32,
}

/// Kind of a ticker item; the page words it from its dictionary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TickerKind {
    /// #1 by Score: `kol`, `name`, `value` = Score.
    TopScore,
    /// Best `d7`: `kol`, `name`, `value` = USD.
    BestWeek,
    /// `value` = KOLs that traded in the last hour.
    TradedLastHour,
    /// `value` = KOLs up over 30 days.
    #[serde(rename = "up_30d")]
    Up30d,
}

#[derive(Clone, Debug, Serialize)]
pub struct TickerItem {
    pub kind: TickerKind,
    pub kol: Option<String>,
    pub name: Option<String>,
    pub value: f64,
}

/// `GET /api/board?date=`.
#[derive(Clone, Debug, Serialize)]
pub struct Board {
    pub date: NaiveDate,
    /// Snapshot close time.
    pub closed_at: Option<DateTime<Utc>>,
    /// Every closed snapshot date, oldest first.
    pub dates: Vec<NaiveDate>,
    pub stats: Stats,
    pub sources: Vec<SourceDate>,
    pub ticker: Vec<TickerItem>,
    /// By rank.
    pub rows: Vec<BoardRow>,
}

/// One part of the Score breakdown.
#[derive(Clone, Debug, Serialize)]
pub struct PartDto {
    pub kind: PartKind,
    pub raw: Option<f64>,
    pub all_time: bool,
    pub share: f64,
    pub points: f64,
    pub status: PartStatus,
}

/// Source handles (validated, no `@`); the page builds the links.
#[derive(Clone, Debug, Serialize)]
pub struct Links {
    pub kolexplorer: Option<String>,
    pub solanatracker: bool,
    pub kolscan: bool,
    pub fomo: Option<String>,
    pub pumpfun: Option<String>,
    pub telegram: Option<String>,
}

/// A published wallet.
#[derive(Clone, Debug, Serialize)]
pub struct Wallet {
    pub address: String,
    /// The wallet Score is computed from.
    pub main: bool,
    /// `https://solscan.io/account/<address>`: a link only, never fetched.
    pub solscan_url: String,
}

/// `GET /api/kol/:id?date=`.
#[derive(Clone, Debug, Serialize)]
pub struct KolCard {
    pub kol: String,
    pub name: String,
    pub x_handle: Option<String>,
    pub date: NaiveDate,
    pub rank: u32,
    /// KOLs ranked on that snapshot.
    pub ranked: u32,
    pub top100: bool,
    pub in_network: bool,
    pub score: u8,
    pub tier: Tier,
    pub parts: Vec<PartDto>,
    pub badges: Vec<Badge>,
    pub source: Option<MetricSource>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub stale: bool,
    pub metrics: Option<PublicMetrics>,
    pub formats: Vec<String>,
    pub links: Links,
    /// Main wallet first.
    pub wallets: Vec<Wallet>,
}
