//! KOL registry API (layer 1): KOLs and their published wallets.
//!
//! A plain store: API only. Only published wallets live here; hypotheses never do.
//! One wallet belongs to one KOL. The first KOLs come from the prototype data.

use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[path = "../../stub/mod.rs"]
pub mod stub;

/// KOL id: a stable slug (prototype slug; new KOLs get one from the name, deduplicated).
pub type KolId = String;

/// Ids and handles the page builds source links from. Handles have no `@`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KolLinks {
    /// KOL Explorer profile slug: `https://kolexplorer.com/kol/<slug>`.
    pub kolexplorer: Option<String>,
    /// Solana Tracker has a page for the main wallet.
    pub solanatracker: bool,
    /// Kolscan has a page for the main wallet.
    pub kolscan: bool,
    /// FOMO handle.
    pub fomo: Option<String>,
    /// pump.fun profile: username or address.
    pub pumpfun: Option<String>,
    /// Public Telegram channel handle (a public field).
    pub telegram: Option<String>,
}

/// Audience reach. `None` = unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reach {
    pub x_followers: Option<i64>,
    pub telegram_subscribers: Option<i64>,
    pub pumpfun_followers: Option<i64>,
}

/// Reach with where and when it was read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReachRecord {
    pub reach: Reach,
    /// Where the numbers come from, e.g. `prototype`, `X profile`.
    pub source: String,
    pub date: NaiveDate,
}

/// Who published a wallet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletSource {
    /// Day-0 seed from the prototype data.
    Prototype,
    /// The KOL's row on the KOL Explorer leaderboard (public profile; published at once).
    Kolexplorer,
    /// Confirmed by the wallets plugin for a network KOL.
    Network,
}

/// A published wallet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedWallet {
    /// Base58 Solana address (32 bytes).
    pub address: String,
    pub source: WalletSource,
    pub published_at: DateTime<Utc>,
}

/// Everything the registry keeps about one KOL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KolProfile {
    pub id: KolId,
    pub name: String,
    /// X handle without `@`.
    pub x_handle: Option<String>,
    pub links: KolLinks,
    /// Main wallet: the one Score is computed from. Solscan link:
    /// `https://solscan.io/account/<address>`.
    pub main_wallet: Option<PublishedWallet>,
    /// Other published wallets of this KOL.
    pub other_wallets: Vec<PublishedWallet>,
    pub reach: Option<ReachRecord>,
    /// Work formats, e.g. `post`, `buy`, `thread`, `stream`, `space`, `call`.
    pub formats: Vec<String>,
}

/// Input of [`RegistryApi::upsert_kol`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KolUpsert {
    /// Existing KOL to update; `None` creates a KOL with a new slug from `name`.
    pub id: Option<KolId>,
    pub name: String,
    pub x_handle: Option<String>,
    pub links: KolLinks,
}

/// Registry call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// The registry is off for this app or person (stub).
    #[error("registry is off")]
    Off,
    /// The registry plugin is not built yet.
    #[error("registry is not built yet")]
    NotBuilt,
    #[error("unknown KOL {kol}")]
    UnknownKol { kol: KolId },
    /// `publish_wallets` refused: the wallet is published for another KOL.
    #[error("wallet {address} already belongs to {owner}")]
    WalletTaken { address: String, owner: KolId },
    #[error("not a Solana address: {address}")]
    InvalidAddress { address: String },
    #[error("registry: {message}")]
    Internal { message: String },
}

/// Registry operations.
#[async_trait]
pub trait RegistryApi: Send + Sync {
    /// Creates or updates a KOL (name, X handle, links). Returns its id.
    async fn upsert_kol(&self, person: &Person, profile: &KolUpsert)
    -> Result<KolId, RegistryError>;

    /// One KOL's profile; `None` when unknown.
    async fn get_kol(&self, person: &Person, id: &str)
    -> Result<Option<KolProfile>, RegistryError>;

    /// Profiles for `ids`, in the same order; unknown ids are left out.
    async fn list_kols(
        &self,
        person: &Person,
        ids: &[KolId],
    ) -> Result<Vec<KolProfile>, RegistryError>;

    /// KOL with X handle `x` (case-insensitive, without `@`).
    async fn find_by_handle(&self, person: &Person, x: &str)
    -> Result<Option<KolId>, RegistryError>;

    /// KOL that holds published wallet `address`.
    async fn find_by_wallet(
        &self,
        person: &Person,
        address: &str,
    ) -> Result<Option<KolId>, RegistryError>;

    /// Replaces the KOL's published wallets with `main` and `others`, each with its source.
    /// `None` and no others unpublishes them. Refuses with [`RegistryError::WalletTaken`]
    /// when any address is published for another KOL; then nothing changes.
    async fn publish_wallets(
        &self,
        person: &Person,
        kol: &str,
        main: Option<(&str, WalletSource)>,
        others: &[(String, WalletSource)],
    ) -> Result<(), RegistryError>;

    /// Sets the KOL's reach, read from `source` on `date`.
    async fn set_reach(
        &self,
        person: &Person,
        kol: &str,
        reach: &Reach,
        source: &str,
        date: NaiveDate,
    ) -> Result<(), RegistryError>;

    /// Replaces the KOL's work formats.
    async fn set_formats(
        &self,
        person: &Person,
        kol: &str,
        formats: &[String],
    ) -> Result<(), RegistryError>;
}
