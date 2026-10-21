//! KOL Explorer collection API (layer 2).
//!
//! Once a day reads the public KOL Explorer leaderboard (150 KOLs), then the profile page of
//! each top-100 KOL for average position and median hold. Writes the top 100 into the
//! snapshot, publishes their KE wallets in the registry, and keeps the «wallet - X handle»
//! list of the last read. A changed page (under 50 rows or missing fields) fails the run and
//! writes nothing. A failed profile leaves that KOL's `avg_pos` and `hold_s` empty.

use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[path = "../../stub/mod.rs"]
pub mod stub;

/// Result of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeRunStatus {
    /// Top 100 written.
    Ok,
    /// Nothing written; `error` says why.
    Failed,
}

/// Summary of one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeRun {
    pub date: NaiveDate,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub status: KeRunStatus,
    /// Metric rows written to the snapshot (0 when failed).
    pub rows: u32,
    /// Top-100 profiles that could not be read (their position and hold stay empty).
    pub profiles_failed: u32,
    /// Error text when failed.
    pub error: Option<String>,
}

/// KOL Explorer call failed (a failed run is an `Ok` [`KeRun`] with status `failed`).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeError {
    /// KOL Explorer collection is off for this app or person (stub).
    #[error("kolexplorer is off")]
    Off,
    /// The kolexplorer plugin is not built yet.
    #[error("kolexplorer is not built yet")]
    NotBuilt,
    #[error("kolexplorer: {message}")]
    Internal { message: String },
}

/// KOL Explorer collection operations.
#[async_trait]
pub trait KolexplorerApi: Send + Sync {
    /// Reads the leaderboard and top-100 profiles and writes the snapshot for `date`
    /// (which must be open).
    async fn run(&self, person: &Person, date: NaiveDate) -> Result<KeRun, KeError>;

    /// Wallet labelled with X handle `x` in the last leaderboard read (case-insensitive,
    /// without `@`).
    async fn wallet_by_handle(&self, person: &Person, x: &str) -> Result<Option<String>, KeError>;
}
