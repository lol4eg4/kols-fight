//! The radar: today's Solana Tracker KOL list minus the KOLs on the board.

use api_registry::RegistryApi;
use api_snapshots::{SnapshotsApi, SnapshotsError};
use api_solanatracker::{ListedKol, SolanatrackerApi};
use arena_platform::{Api, Person};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashSet;

/// Whether the list is from the requested day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RadarStatus {
    /// The list of the requested day was read.
    Ok,
    /// No list for that day: «Radar not updated today». `list_date` says how old the last one is.
    NotUpdated,
}

/// One KOL on the radar. Public: only what the section shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RadarKol {
    /// Stable key for the pixel avatar and the list.
    pub id: String,
    pub name: String,
    /// X handle without `@`.
    pub x_handle: Option<String>,
    /// Public Solana address, for the Solscan link.
    pub wallet: String,
}

/// The radar section.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RadarView {
    pub date: NaiveDate,
    pub status: RadarStatus,
    /// Date of the newest list on or before `date`; `None` when there is none.
    pub list_date: Option<NaiveDate>,
    /// Empty unless `status` is `ok`.
    pub kols: Vec<RadarKol>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadarError {
    /// The board has no snapshot for that date.
    NoSnapshot(NaiveDate),
    /// A block the radar reads failed; the text is for the log, not the client.
    Upstream(String),
}

#[derive(Clone)]
pub struct Radar {
    solanatracker: Api<dyn SolanatrackerApi>,
    snapshots: Api<dyn SnapshotsApi>,
    registry: Api<dyn RegistryApi>,
}

impl Radar {
    pub fn new(
        solanatracker: Api<dyn SolanatrackerApi>,
        snapshots: Api<dyn SnapshotsApi>,
        registry: Api<dyn RegistryApi>,
    ) -> Self {
        Radar {
            solanatracker,
            snapshots,
            registry,
        }
    }

    /// The snapshot date the radar uses when none is asked: the latest closed one.
    pub async fn latest_date(&self) -> Result<Option<NaiveDate>, RadarError> {
        let p = &Person::NOBODY;
        self.snapshots
            .pick(p)
            .latest_date(p)
            .await
            .map_err(|e| RadarError::Upstream(e.to_string()))
    }

    pub async fn view(&self, date: NaiveDate) -> Result<RadarView, RadarError> {
        let p = &Person::NOBODY;
        let upstream = |e: &dyn std::fmt::Display| RadarError::Upstream(e.to_string());
        let board = match self.snapshots.pick(p).snapshot(p, date).await {
            Ok(v) => v,
            Err(SnapshotsError::NoSnapshot { .. }) => return Err(RadarError::NoSnapshot(date)),
            Err(e) => return Err(upstream(&e)),
        };
        let list = self
            .solanatracker
            .pick(p)
            .kol_list(p, date)
            .await
            .map_err(|e| upstream(&e))?;
        let list = match list {
            Some(l) if l.date == date => l,
            other => {
                return Ok(RadarView {
                    date,
                    status: RadarStatus::NotUpdated,
                    list_date: other.map(|l| l.date),
                    kols: Vec::new(),
                });
            }
        };

        let on_board: HashSet<String> = board.members.into_iter().map(|m| m.kol).collect();
        let registry = self.registry.pick(p);
        let mut kols = Vec::new();
        let mut seen = HashSet::new();
        for listed in list.kols {
            if !is_address(&listed.wallet) {
                tracing::warn!("radar: list row with a bad wallet skipped");
                continue;
            }
            if !seen.insert(listed.wallet.clone()) {
                continue;
            }
            let handle = listed.x_handle.as_deref().and_then(clean_handle);
            let mut owner = registry
                .find_by_wallet(p, &listed.wallet)
                .await
                .map_err(|e| upstream(&e))?;
            if owner.is_none()
                && let Some(h) = &handle
            {
                owner = registry
                    .find_by_handle(p, h)
                    .await
                    .map_err(|e| upstream(&e))?;
            }
            if owner.is_some_and(|id| on_board.contains(&id)) {
                continue;
            }
            kols.push(radar_kol(listed, handle));
        }
        Ok(RadarView {
            date,
            status: RadarStatus::Ok,
            list_date: Some(date),
            kols,
        })
    }
}

fn radar_kol(listed: ListedKol, handle: Option<String>) -> RadarKol {
    let name = listed
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .or_else(|| handle.as_ref().map(|h| format!("@{h}")))
        .unwrap_or_else(|| {
            format!(
                "{}…{}",
                &listed.wallet[..4],
                &listed.wallet[listed.wallet.len() - 4..]
            )
        });
    RadarKol {
        id: handle
            .as_ref()
            .map(|h| h.to_lowercase())
            .unwrap_or_else(|| listed.wallet.clone()),
        name,
        x_handle: handle,
        wallet: listed.wallet,
    }
}

/// X handle without `@`, 1..=15 letters, digits or `_`; anything else is dropped.
fn clean_handle(raw: &str) -> Option<String> {
    let h = raw.trim().trim_start_matches('@');
    let ok =
        (1..=15).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    ok.then(|| h.to_string())
}

/// Base58 text of a plausible address length (a 32-byte key is 32..=44 characters).
fn is_address(s: &str) -> bool {
    const B58: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    (32..=44).contains(&s.len()) && s.chars().all(|c| B58.contains(c))
}
