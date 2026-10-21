//! Journal API (layer 0): records who changed what, when, in which block.
//!
//! A plain store: API only, no screens, no permission checks. `before` and `after` are JSON
//! and never hold secrets.

use arena_platform::{Person, Role};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[path = "../../stub/mod.rs"]
pub mod stub;

/// Id of a journal entry.
pub type EntryId = i64;

/// One recorded change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    /// When it was recorded.
    pub at: DateTime<Utc>,
    /// Staff id of who made the change; `None` for nobody and for the server itself.
    pub person_id: Option<i64>,
    /// Role of who made it; `admin` with no id is the server ([`Person::SYSTEM`]).
    pub role: Role,
    /// Plugin that owns the object, e.g. `wallets`.
    pub block: String,
    /// Object id inside that block, e.g. a KOL id or a wallet candidate id.
    pub object: String,
    /// What happened, e.g. `login`, `confirm`, `revoke`, `sync`, `run`.
    pub action: String,
    /// State before the change; `None` when the object was created.
    pub before: Option<Value>,
    /// State after the change; `None` when the object was removed.
    pub after: Option<Value>,
}

/// Journal call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum JournalError {
    /// The journal is off for this app or person (stub).
    #[error("journal is off")]
    Off,
    /// The journal plugin is not built yet.
    #[error("journal is not built yet")]
    NotBuilt,
    #[error("journal: {message}")]
    Internal { message: String },
}

/// Journal operations.
#[async_trait]
pub trait JournalApi: Send + Sync {
    /// Records a change made by `person` to `object` of `block`. Returns the entry id.
    async fn record(
        &self,
        person: &Person,
        block: &str,
        object: &str,
        action: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) -> Result<EntryId, JournalError>;

    /// Every entry for `object` of `block`, oldest first.
    async fn history(
        &self,
        person: &Person,
        block: &str,
        object: &str,
    ) -> Result<Vec<Entry>, JournalError>;
}
