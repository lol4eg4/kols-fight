//! Network roster API (layer 3): members of our network and their rates.
//!
//! Reads the owner's sheet daily at 05:45 UTC and on the admin button. The sheet is the main
//! list: a removed row ends membership (history stays); the 9 prototype network KOLs absent
//! from the sheet stay (`keep`). Group, rates, conditions and notes are team-only: only
//! [`RosterApi::rates`] returns rates, and only to staff. Until staff verifies a KOL's X
//! handle, its FOMO handle counts as the X handle.

use arena_platform::{Event, Person};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[path = "../../stub/mod.rs"]
pub mod stub;

/// KOL id (registry slug).
pub type KolId = String;

/// A network member, without rates, group or notes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkMember {
    pub kol: KolId,
    pub name: String,
    /// Verified X handle, or the FOMO handle until verified. Without `@`.
    pub x_handle: Option<String>,
    /// Whether staff verified `x_handle`.
    pub x_verified: bool,
    pub fomo_handle: Option<String>,
    /// pump.fun profile from the sheet (public).
    pub pumpfun: Option<String>,
    /// Base58 Solana addresses from the sheet's Wallet column (and an unnamed column after
    /// it). Team-only wallet candidates for the wallets plugin: never in a public response
    /// until wallets confirms and publishes them.
    pub sheet_wallets: Vec<String>,
    /// Work formats from known keywords (`post`, `buy`, `thread`, `stream`, `space`, `call`).
    pub formats: Vec<String>,
    /// Stays a member without a sheet row (owner's decision for 9 prototype KOLs).
    pub keep: bool,
}

/// A KOL's X handle and whether staff verified it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XHandle {
    /// Without `@`; the FOMO handle while not verified.
    pub handle: String,
    pub verified: bool,
}

/// Rates and formats of one member. Team-only: never in a public response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rates {
    pub kol: KolId,
    /// «Low $» from the sheet, USD.
    pub low_usd: Option<f64>,
    /// «High $» from the sheet, USD.
    pub high_usd: Option<f64>,
    pub formats: Vec<String>,
}

/// «Table read»: the sheet was read on `date` (wallets then matches wallets).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRead {
    pub date: NaiveDate,
}

impl Event for TableRead {
    const NAME: &'static str = "table_read";
}

/// «X handle verified»: staff verified the X handle of `kol`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XHandleVerified {
    pub kol: KolId,
}

impl Event for XHandleVerified {
    const NAME: &'static str = "x_handle_verified";
}

/// Roster call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RosterError {
    /// The roster is off for this app or person (stub).
    #[error("roster is off")]
    Off,
    /// The roster plugin is not built yet.
    #[error("roster is not built yet")]
    NotBuilt,
    /// Rates need a staff person.
    #[error("rates are for staff only")]
    Forbidden,
    #[error("roster: {message}")]
    Internal { message: String },
}

/// Roster operations.
#[async_trait]
pub trait RosterApi: Send + Sync {
    /// Current members, without rates.
    async fn members(&self, person: &Person) -> Result<Vec<NetworkMember>, RosterError>;

    /// Whether `kol` is a current member.
    async fn is_member(&self, person: &Person, kol: &str) -> Result<bool, RosterError>;

    /// The member's X handle and whether it is verified; `None` for non-members.
    async fn verified_handle(&self, person: &Person, kol: &str)
    -> Result<Option<XHandle>, RosterError>;

    /// Rates and formats of `kols` (members only). [`RosterError::Forbidden`] unless
    /// `person` is staff: only for plugins serving `#staff` routes.
    async fn rates(&self, person: &Person, kols: &[KolId]) -> Result<Vec<Rates>, RosterError>;
}
