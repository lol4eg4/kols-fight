//! The «Our network» section: network KOLs on the board and the waiting list.
//!
//! Public DTOs only. The roster's rates, group and notes never reach this module (it calls
//! no staff API), and no address leaves it: the waiting list says only «wallet not confirmed
//! yet» or «known wallet quiet for N days», so a client cannot tell whether a hypothesis
//! exists.

use api_registry::RegistryApi;
use api_roster::RosterApi;
use api_score::{ScoreApi, ScoreError, Tier};
use api_snapshots::{SnapshotStatus, SnapshotsApi, SnapshotsError};
use api_wallets::{KolWalletStatus, WalletsApi, WalletsError};
use arena_platform::{Api, Person};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// A network KOL on the board.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OnBoard {
    pub kol: String,
    pub name: String,
    /// Place on the board; `None` while the snapshot is not ranked.
    pub rank: Option<u32>,
    pub score: Option<u8>,
    pub tier: Option<Tier>,
    /// Realized PnL over 30 days, USD.
    pub pnl_30d: Option<f64>,
    /// Volume over 30 days, USD.
    pub volume_30d: Option<f64>,
    /// Days with a trade in the last 30.
    pub days_in_market: Option<i32>,
}

/// Why a network KOL is not on the board. Only these two, by design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitReason {
    /// No confirmed wallet: covers a hypothesis, nothing found and a conflict alike.
    WalletNotConfirmed,
    /// The confirmed wallet has no recent trades.
    Quiet,
}

/// A network KOL on the waiting list: name and reason, no numbers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Waiting {
    pub kol: String,
    pub name: String,
    pub reason: WaitReason,
    /// For `quiet`: whole days since the last known trade, when known.
    pub quiet_days: Option<i64>,
}

/// Response of `GET /api/network`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NetworkView {
    pub date: NaiveDate,
    /// By rank, unranked last by name.
    pub members: Vec<OnBoard>,
    /// By name.
    pub waiting: Vec<Waiting>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkError {
    /// No closed snapshot for that date.
    NoSnapshot(NaiveDate),
    /// A block this section reads failed; the text is for the log, not the client.
    Upstream(String),
}

fn upstream(e: impl std::fmt::Display) -> NetworkError {
    NetworkError::Upstream(e.to_string())
}

#[derive(Clone)]
pub struct Network {
    pub(crate) roster: Api<dyn RosterApi>,
    pub(crate) wallets: Api<dyn WalletsApi>,
    pub(crate) score: Api<dyn ScoreApi>,
    pub(crate) snapshots: Api<dyn SnapshotsApi>,
    pub(crate) registry: Api<dyn RegistryApi>,
}

impl Network {
    pub fn new(
        roster: Api<dyn RosterApi>,
        wallets: Api<dyn WalletsApi>,
        score: Api<dyn ScoreApi>,
        snapshots: Api<dyn SnapshotsApi>,
        registry: Api<dyn RegistryApi>,
    ) -> Self {
        Network {
            roster,
            wallets,
            score,
            snapshots,
            registry,
        }
    }

    /// Latest closed snapshot date; `None` when there is none.
    pub async fn latest_date(&self) -> Result<Option<NaiveDate>, NetworkError> {
        let p = &Person::NOBODY;
        self.snapshots
            .pick(p)
            .latest_date(p)
            .await
            .map_err(upstream)
    }

    /// The section for the closed snapshot of `date`. Every call runs as a visitor
    /// ([`Person::NOBODY`]): this view must never hold what a visitor may not see.
    pub async fn view(&self, date: NaiveDate) -> Result<NetworkView, NetworkError> {
        let p = &Person::NOBODY;
        let board = match self.snapshots.pick(p).snapshot(p, date).await {
            Ok(v) if v.snapshot.status == SnapshotStatus::Closed => v,
            Ok(_) | Err(SnapshotsError::NoSnapshot { .. }) => {
                return Err(NetworkError::NoSnapshot(date));
            }
            Err(e) => return Err(upstream(e)),
        };
        let members = self.roster.pick(p).members(p).await.map_err(upstream)?;
        if members.is_empty() {
            return Ok(NetworkView {
                date,
                members: Vec::new(),
                waiting: Vec::new(),
            });
        }
        let ranking = match self.score.pick(p).ranking(p, date).await {
            Ok(r) => Some(r),
            Err(ScoreError::NotRanked { .. } | ScoreError::NoSnapshot { .. }) => None,
            Err(e) => return Err(upstream(e)),
        };

        let ids: Vec<String> = members.iter().map(|m| m.kol.clone()).collect();
        let names: HashMap<String, String> = self
            .registry
            .pick(p)
            .list_kols(p, &ids)
            .await
            .map_err(upstream)?
            .into_iter()
            .map(|k| (k.id, k.name))
            .collect();
        let rows: HashMap<&str, &api_snapshots::Metrics> = board
            .members
            .iter()
            .filter_map(|m| m.row.as_ref().map(|r| (m.kol.as_str(), &r.metrics)))
            .collect();
        let ranked: HashMap<&str, &api_score::RankedKol> = ranking
            .as_ref()
            .map(|r| r.rows.iter().map(|k| (k.kol.as_str(), k)).collect())
            .unwrap_or_default();
        // On the board: ranked, or (before ranking) a member of the snapshot with a row.
        let on_board: HashSet<&str> = match &ranking {
            Some(_) => ranked.keys().copied().collect(),
            None => rows.keys().copied().collect(),
        };

        let mut out = NetworkView {
            date,
            members: Vec::new(),
            waiting: Vec::new(),
        };
        let wallets = self.wallets.pick(p);
        for m in &members {
            let name = names.get(&m.kol).cloned().unwrap_or_else(|| m.name.clone());
            if on_board.contains(m.kol.as_str()) {
                let metrics = rows.get(m.kol.as_str());
                let card = ranked.get(m.kol.as_str());
                out.members.push(OnBoard {
                    kol: m.kol.clone(),
                    name,
                    rank: card.map(|c| c.rank),
                    score: card.map(|c| c.card.score),
                    tier: card.map(|c| c.card.tier),
                    pnl_30d: metrics.and_then(|x| x.d30),
                    volume_30d: metrics.and_then(|x| x.vol30),
                    days_in_market: metrics.and_then(|x| x.days30),
                });
                continue;
            }
            let (reason, quiet_days) = match wallets.status(p, &m.kol).await {
                Ok(s) if s.status == KolWalletStatus::Confirmed => (
                    WaitReason::Quiet,
                    s.last_trade_at
                        .map(|t| (date - t.date_naive()).num_days().max(0)),
                ),
                Ok(_) => (WaitReason::WalletNotConfirmed, None),
                // Left the network between the two reads: nothing confirmed to say.
                Err(WalletsError::NotMember { .. }) => (WaitReason::WalletNotConfirmed, None),
                Err(e) => return Err(upstream(e)),
            };
            out.waiting.push(Waiting {
                kol: m.kol.clone(),
                name,
                reason,
                quiet_days,
            });
        }
        out.members.sort_by(|a, b| match (a.rank, b.rank) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
        out.waiting.sort_by_key(|w| w.name.to_lowercase());
        Ok(out)
    }
}
