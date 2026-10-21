//! Network wallets API (layer 3): wallet candidates of network KOLs and their status.
//!
//! Candidate states (scope review, «Wallet state machine»):
//!
//! ```text
//!   (new) --name-only or unverified handle match, FOMO unique match--> hypothesis
//!   (new) --verified X handle match | address from KOL or sheet with trades--> confirmed
//!   (new) --address from KOL or sheet without trades--> not_found (with reason)
//!   hypothesis --KOL confirms, staff records where and when--> confirmed
//!   hypothesis --handle verified and equals the wallet label--> confirmed
//!   hypothesis --staff rejects--> rejected
//!   confirmed --staff revokes--> revoked
//!   sheet candidate --sheet shows another address--> retired
//!   any --address published for another KOL--> conflict
//! ```
//!
//! A hypothesis is never published. Only confirmed candidates go to the registry. Every
//! status change goes to the journal.

use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[path = "../../stub/mod.rs"]
pub mod stub;

/// KOL id (registry slug).
pub type KolId = String;

/// Wallet status of a network KOL.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KolWalletStatus {
    /// Has a confirmed main wallet.
    Confirmed,
    /// Has a hypothesis, nothing confirmed.
    Hypothesis,
    /// No candidate.
    NotFound,
    /// A candidate conflicts with another published wallet; nothing is published until
    /// staff resolves it.
    Conflict,
}

/// State of one candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    /// Not confirmed; never published, never shown to a client.
    Hypothesis,
    /// Published to the registry (main or other wallet).
    Confirmed,
    /// Checked and not usable, e.g. an address without trades; `reason` says why.
    NotFound,
    /// Staff rejected the hypothesis.
    Rejected,
    /// Staff revoked a confirmed wallet.
    Revoked,
    /// A sheet address replaced by another address in a later sheet read.
    Retired,
    /// The address is published for another KOL (or differs from the KOL's KE wallet);
    /// nothing is published until staff resolves it.
    Conflict,
}

/// Where a candidate comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateSource {
    /// KOL Explorer label by X handle.
    Kolexplorer,
    /// Solana Tracker identity by X handle or FOMO handle.
    Solanatracker,
    /// FOMO match: positions from the KOL's FOMO profile.
    Fomo,
    /// Address sent by the KOL.
    FromKol,
    /// Base58 address in the network sheet's Wallet column (or an unnamed column after it),
    /// confirmed once the trades check passes.
    Sheet,
    /// Address in the sheet's pump.fun column.
    Pumpfun,
    /// Confirmed in the prototype data.
    Prototype,
}

/// One wallet candidate. Team-only: holds hypothesis addresses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: i64,
    pub kol: KolId,
    pub address: String,
    pub status: CandidateStatus,
    pub source: CandidateSource,
    /// Proof, e.g. which positions matched.
    pub evidence: Option<String>,
    /// Who sent the address (for `from_kol`).
    pub sent_by: Option<String>,
    /// Where and when the KOL confirmed it (manual confirm of a hypothesis).
    pub confirmation: Option<String>,
    /// Why the candidate is `not_found`, `conflict` or `retired` (e.g. «no trades»).
    pub reason: Option<String>,
    /// The KOL's main wallet.
    pub main: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A KOL's wallet status without any hypothesis address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletStatus {
    pub kol: KolId,
    pub status: KolWalletStatus,
    /// Confirmed main wallet (published), when there is one.
    pub main_address: Option<String>,
    /// Last trade of the confirmed main wallet at the last check
    /// («known wallet quiet for N days»).
    pub last_trade_at: Option<DateTime<Utc>>,
}

/// Wallets call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WalletsError {
    /// Wallets are off for this app or person (stub).
    #[error("wallets are off")]
    Off,
    /// The wallets plugin is not built yet.
    #[error("wallets is not built yet")]
    NotBuilt,
    #[error("{kol} is not in the network")]
    NotMember { kol: KolId },
    #[error("not a Solana address: {address}")]
    InvalidAddress { address: String },
    /// Wallet matching holds the shared `arena_data` lock.
    #[error("collection running, try again later")]
    Busy,
    #[error("wallets: {message}")]
    Internal { message: String },
}

/// Network wallets operations.
#[async_trait]
pub trait WalletsApi: Send + Sync {
    /// Adds a hypothesis for `kol` (source `fomo`) with `evidence`. Returns the candidate.
    async fn propose(
        &self,
        person: &Person,
        kol: &str,
        address: &str,
        evidence: &str,
    ) -> Result<Candidate, WalletsError>;

    /// The KOL's status, without a hypothesis address.
    async fn status(&self, person: &Person, kol: &str) -> Result<WalletStatus, WalletsError>;
}
