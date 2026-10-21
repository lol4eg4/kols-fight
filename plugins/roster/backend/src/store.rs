//! Reads of the roster tables.

use api_roster::{NetworkMember, Rates, RosterError, XHandle};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

pub(crate) fn db(e: sqlx::Error) -> RosterError {
    RosterError::Internal {
        message: e.to_string(),
    }
}

/// A member joined with its team-only terms.
#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct MemberRow {
    pub kol: String,
    pub name: String,
    pub fomo_handle: Option<String>,
    pub fomo_key: Option<String>,
    pub x_handle: Option<String>,
    pub x_verified: bool,
    pub pumpfun: Option<String>,
    pub formats: Vec<String>,
    pub keep: bool,
    pub active: bool,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub grp: Option<String>,
    pub low_usd: Option<f64>,
    pub high_usd: Option<f64>,
    pub note: Option<String>,
    pub wallet_note: Option<String>,
    pub sheet_wallets: Vec<String>,
}

pub(crate) const SELECT: &str = "SELECT m.kol, m.name, m.fomo_handle, m.fomo_key, m.x_handle, m.x_verified, \
     m.pumpfun, m.formats, m.keep, m.active, m.joined_at, m.left_at, m.last_seen_at, \
     t.grp, t.low_usd, t.high_usd, t.note, t.wallet_note, \
     COALESCE(t.sheet_wallets, '{}') AS sheet_wallets \
     FROM roster.member m LEFT JOIN roster.terms t ON t.kol = m.kol";

impl MemberRow {
    /// Public-safe view. `team` also hands out the sheet's wallet candidates.
    pub fn member(&self, team: bool) -> NetworkMember {
        NetworkMember {
            kol: self.kol.clone(),
            name: self.name.clone(),
            x_handle: self.handle().map(|h| h.handle),
            x_verified: self.x_verified,
            fomo_handle: self.fomo_handle.clone(),
            pumpfun: self.pumpfun.clone(),
            sheet_wallets: if team {
                self.sheet_wallets.clone()
            } else {
                Vec::new()
            },
            formats: self.formats.clone(),
            keep: self.keep,
        }
    }

    /// The X handle, or the FOMO handle until staff verified one.
    pub fn handle(&self) -> Option<XHandle> {
        self.x_handle
            .clone()
            .or_else(|| self.fomo_handle.clone())
            .map(|handle| XHandle {
                handle,
                verified: self.x_verified,
            })
    }

    pub fn rates(&self) -> Rates {
        Rates {
            kol: self.kol.clone(),
            low_usd: self.low_usd,
            high_usd: self.high_usd,
            formats: self.formats.clone(),
        }
    }
}

/// A member with everything the team sees. Only the admin routes return it.
#[derive(Serialize)]
pub struct AdminMember {
    #[serde(flatten)]
    pub member: NetworkMember,
    pub active: bool,
    pub group: Option<String>,
    pub low_usd: Option<f64>,
    pub high_usd: Option<f64>,
    /// «Deliverable / note» cell.
    pub note: Option<String>,
    /// A Wallet cell that is not a Solana address.
    pub wallet_note: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

impl From<&MemberRow> for AdminMember {
    fn from(r: &MemberRow) -> Self {
        AdminMember {
            member: r.member(true),
            active: r.active,
            group: r.grp.clone(),
            low_usd: r.low_usd,
            high_usd: r.high_usd,
            note: r.note.clone(),
            wallet_note: r.wallet_note.clone(),
            joined_at: r.joined_at,
            left_at: r.left_at,
            last_seen_at: r.last_seen_at,
        }
    }
}

pub(crate) async fn members(
    pool: &PgPool,
    active_only: bool,
) -> Result<Vec<MemberRow>, RosterError> {
    let sql = format!(
        "{SELECT} {} ORDER BY m.name, m.kol",
        if active_only { "WHERE m.active" } else { "" }
    );
    sqlx::query_as(&sql).fetch_all(pool).await.map_err(db)
}

pub(crate) async fn member(pool: &PgPool, kol: &str) -> Result<Option<MemberRow>, RosterError> {
    let sql = format!("{SELECT} WHERE m.kol = $1");
    sqlx::query_as(&sql)
        .bind(kol)
        .fetch_optional(pool)
        .await
        .map_err(db)
}

/// Result of one sheet read, as stored.
#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct SyncRun {
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    /// `ok` or `failed`.
    pub status: String,
    pub error: Option<String>,
    pub rows: i32,
    pub added: i32,
    pub updated: i32,
    pub removed: i32,
    pub skipped: serde_json::Value,
    pub note: Option<String>,
}

pub(crate) async fn last_sync(pool: &PgPool) -> Result<Option<SyncRun>, RosterError> {
    sqlx::query_as(
        "SELECT started_at, finished_at, status, error, rows, added, updated, removed, skipped, note \
         FROM roster.sync_run ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(db)
}
