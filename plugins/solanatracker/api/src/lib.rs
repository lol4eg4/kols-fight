//! Solana Tracker collection API (layer 2).
//!
//! Once a day, through the official Data API with the company key, reads PnL for KOLs with a
//! published wallet that KOL Explorer did not write for that date, and the day's KOL list with
//! X handles (marks «in top today»). At most 3 requests per second; retries only on 5xx. A
//! rejected key, limit or plan fails that KOL; its earlier numbers stay. The key comes from
//! `SOLANA_TRACKER_API_KEY`; without it every call reports [`StFailure::NotConfigured`].

use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[path = "../../stub/mod.rs"]
pub mod stub;

/// KOL id (registry slug).
pub type KolId = String;

/// Why Solana Tracker did not answer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StFailure {
    /// `SOLANA_TRACKER_API_KEY` is not set.
    #[error("SOLANA_TRACKER_API_KEY not configured")]
    NotConfigured,
    /// HTTP 401.
    #[error("key rejected")]
    KeyRejected,
    /// HTTP 403: the plan does not include PnL.
    #[error("plan lacks PnL")]
    PlanLacksPnl,
    /// HTTP 429: the run stops and is marked partial.
    #[error("rate limited")]
    RateLimited,
    /// HTTP 5xx after retries.
    #[error("Solana Tracker error {status}")]
    Upstream { status: u16 },
    /// Connection or timeout.
    #[error("network: {message}")]
    Network { message: String },
    /// Unexpected response shape.
    #[error("bad response: {message}")]
    BadResponse { message: String },
}

/// Result of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StRunStatus {
    /// Every KOL and the list read.
    Ok,
    /// Some KOLs failed or the run stopped on 429.
    Partial,
    /// Nothing read (no key, key rejected, plan).
    Failed,
}

/// Result for one KOL.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KolRunStatus {
    /// Metrics written.
    Ok,
    /// KOL Explorer wrote this KOL for the date.
    Skipped,
    /// Not read; earlier numbers stay.
    Failed,
}

/// One KOL in a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KolRun {
    pub kol: KolId,
    /// Main published wallet that was read.
    pub wallet: String,
    pub status: KolRunStatus,
    pub failure: Option<StFailure>,
}

/// Summary of one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StRun {
    pub date: NaiveDate,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub status: StRunStatus,
    /// Why the run failed or stopped.
    pub failure: Option<StFailure>,
    pub kols: Vec<KolRun>,
    /// KOLs in the day's list (0 when it was not read).
    pub list_rows: u32,
    /// HTTP requests made.
    pub calls: u32,
}

/// One KOL in the day's list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedKol {
    pub name: Option<String>,
    /// X handle without `@` (`identity.twitter`).
    pub x_handle: Option<String>,
    pub wallet: String,
}

/// The day's KOL list (30-day KOL leaderboard).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KolList {
    /// Date of the list; earlier than asked when that day's list is missing.
    pub date: NaiveDate,
    pub fetched_at: DateTime<Utc>,
    pub kols: Vec<ListedKol>,
}

/// A wallet Solana Tracker labels with an X handle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StWallet {
    pub address: String,
    pub name: Option<String>,
    pub x_handle: Option<String>,
    /// FOMO handle from the wallet's identity, when known.
    pub fomo_handle: Option<String>,
}

/// Whether a wallet trades.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSummary {
    pub address: String,
    pub has_trades: bool,
    pub last_trade_at: Option<DateTime<Utc>>,
}

/// Solana Tracker call failed (a failed run is an `Ok` [`StRun`] with status `failed`).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StError {
    /// Solana Tracker collection is off for this app or person (stub).
    #[error("solanatracker is off")]
    Off,
    /// The solanatracker plugin is not built yet.
    #[error("solanatracker is not built yet")]
    NotBuilt,
    /// The source did not answer.
    #[error("{failure}")]
    Source { failure: StFailure },
    #[error("solanatracker: {message}")]
    Internal { message: String },
}

/// Solana Tracker collection operations.
#[async_trait]
pub trait SolanatrackerApi: Send + Sync {
    /// Reads the day's list and PnL of every member KOL that KOL Explorer did not write for
    /// `date` (which must be open); writes metrics and the «in top today» marks.
    async fn run(&self, person: &Person, date: NaiveDate) -> Result<StRun, StError>;

    /// Latest stored KOL list on or before `date`; `None` when there is none.
    async fn kol_list(&self, person: &Person, date: NaiveDate) -> Result<Option<KolList>, StError>;

    /// Wallet whose `identity.twitter` equals `x` (case-insensitive, without `@`) in the
    /// latest stored list.
    async fn wallet_by_handle(&self, person: &Person, x: &str)
    -> Result<Option<StWallet>, StError>;

    /// Whether `address` has trades and when the last one was (one live call).
    async fn wallet_summary(
        &self,
        person: &Person,
        address: &str,
    ) -> Result<WalletSummary, StError>;
}
