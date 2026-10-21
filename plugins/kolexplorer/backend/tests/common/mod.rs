//! Fakes for the tests: in-memory registry and snapshots, and a local KOL Explorer.
#![allow(dead_code)]

use api_registry::*;
use api_snapshots::{
    Member, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotStatus, SnapshotView,
    SnapshotsApi, SnapshotsError,
};
use arena_platform::{Api, Apis, Person, Registry};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

pub const BOARD_HTML: &str =
    include_str!("../../../../../data/prototype/kolexplorer-2026-10-19.html");
pub const PROFILE_HTML: &str = include_str!("../../../tests/fixtures/profile_decu.html");

pub fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 20).unwrap()
}

// ---------------------------------------------------------------- registry

#[derive(Default)]
pub struct FakeRegistry {
    pub kols: Mutex<BTreeMap<String, KolProfile>>,
}

impl FakeRegistry {
    pub fn add(&self, id: &str, name: &str, x: Option<&str>, main: Option<(&str, WalletSource)>) {
        self.kols.lock().unwrap().insert(
            id.to_string(),
            KolProfile {
                id: id.to_string(),
                name: name.to_string(),
                x_handle: x.map(str::to_string),
                links: KolLinks::default(),
                main_wallet: main.map(|(a, s)| PublishedWallet {
                    address: a.to_string(),
                    source: s,
                    published_at: Utc::now(),
                }),
                other_wallets: vec![],
                reach: None,
                formats: vec![],
            },
        );
    }

    pub fn get(&self, id: &str) -> Option<KolProfile> {
        self.kols.lock().unwrap().get(id).cloned()
    }
}

#[async_trait]
impl RegistryApi for FakeRegistry {
    async fn upsert_kol(&self, _: &Person, p: &KolUpsert) -> Result<KolId, RegistryError> {
        let mut kols = self.kols.lock().unwrap();
        let id = match &p.id {
            Some(id) => id.clone(),
            None => {
                let base: String = p
                    .name
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect();
                let mut id = base.clone();
                let mut n = 2;
                while kols.contains_key(&id) {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                id
            }
        };
        let old = kols.get(&id).cloned();
        kols.insert(
            id.clone(),
            KolProfile {
                id: id.clone(),
                name: p.name.clone(),
                x_handle: p.x_handle.clone(),
                links: p.links.clone(),
                main_wallet: old.as_ref().and_then(|o| o.main_wallet.clone()),
                other_wallets: old.map(|o| o.other_wallets).unwrap_or_default(),
                reach: None,
                formats: vec![],
            },
        );
        Ok(id)
    }

    async fn get_kol(&self, _: &Person, id: &str) -> Result<Option<KolProfile>, RegistryError> {
        Ok(self.get(id))
    }

    async fn list_kols(&self, _: &Person, ids: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        Ok(ids.iter().filter_map(|i| self.get(i)).collect())
    }

    async fn find_by_handle(&self, _: &Person, x: &str) -> Result<Option<KolId>, RegistryError> {
        let x = x.trim_start_matches('@').to_lowercase();
        Ok(self
            .kols
            .lock()
            .unwrap()
            .values()
            .find(|k| k.x_handle.as_deref().map(str::to_lowercase).as_deref() == Some(&x))
            .map(|k| k.id.clone()))
    }

    async fn find_by_wallet(&self, _: &Person, a: &str) -> Result<Option<KolId>, RegistryError> {
        Ok(self
            .kols
            .lock()
            .unwrap()
            .values()
            .find(|k| {
                k.main_wallet
                    .iter()
                    .chain(k.other_wallets.iter())
                    .any(|w| w.address == a)
            })
            .map(|k| k.id.clone()))
    }

    async fn publish_wallets(
        &self,
        _: &Person,
        kol: &str,
        main: Option<(&str, WalletSource)>,
        others: &[(String, WalletSource)],
    ) -> Result<(), RegistryError> {
        let mut kols = self.kols.lock().unwrap();
        for (a, _) in main.into_iter().chain(others.iter().map(|(a, s)| (a.as_str(), *s))) {
            if let Some(owner) = kols.values().find(|k| {
                k.id != kol
                    && k.main_wallet
                        .iter()
                        .chain(k.other_wallets.iter())
                        .any(|w| w.address == a)
            }) {
                return Err(RegistryError::WalletTaken {
                    address: a.to_string(),
                    owner: owner.id.clone(),
                });
            }
        }
        let k = kols.get_mut(kol).ok_or(RegistryError::UnknownKol {
            kol: kol.to_string(),
        })?;
        let w = |a: &str, source| PublishedWallet {
            address: a.to_string(),
            source,
            published_at: Utc::now(),
        };
        k.main_wallet = main.map(|(a, s)| w(a, s));
        k.other_wallets = others.iter().map(|(a, s)| w(a, *s)).collect();
        Ok(())
    }

    async fn set_reach(
        &self,
        _: &Person,
        _: &str,
        _: &Reach,
        _: &str,
        _: NaiveDate,
    ) -> Result<(), RegistryError> {
        Ok(())
    }

    async fn set_formats(&self, _: &Person, _: &str, _: &[String]) -> Result<(), RegistryError> {
        Ok(())
    }
}

// --------------------------------------------------------------- snapshots

#[derive(Default)]
pub struct SnapState {
    pub snapshot: Option<Snapshot>,
    pub members: BTreeMap<String, Vec<MemberReason>>,
    pub rows: BTreeMap<String, MetricRow>,
    /// Writes made through the API (`set_members`, `put_metrics`).
    pub writes: u32,
}

#[derive(Default)]
pub struct FakeSnapshots {
    pub state: Mutex<SnapState>,
}

impl FakeSnapshots {
    pub fn open_day(&self, date: NaiveDate) {
        let mut s = self.state.lock().unwrap();
        s.snapshot = Some(Snapshot {
            date,
            status: SnapshotStatus::Open,
            opened_at: Utc::now() - chrono::Duration::seconds(1),
            closed_at: None,
        });
    }

    pub fn close_day(&self) {
        let mut s = self.state.lock().unwrap();
        let snap = s.snapshot.as_mut().unwrap();
        snap.status = SnapshotStatus::Closed;
        snap.closed_at = Some(Utc::now());
    }

    pub fn row(&self, kol: &str) -> Option<MetricRow> {
        self.state.lock().unwrap().rows.get(kol).cloned()
    }

    pub fn writes(&self) -> u32 {
        self.state.lock().unwrap().writes
    }

    pub fn members_with(&self, r: MemberReason) -> Vec<String> {
        let s = self.state.lock().unwrap();
        s.members
            .iter()
            .filter(|(_, v)| v.contains(&r))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Adds a member and optionally its row (as an earlier step of the day would).
    pub fn seed(
        &self,
        kol: &str,
        reason: MemberReason,
        row: Option<(MetricSource, DateTime<Utc>)>,
    ) {
        let mut s = self.state.lock().unwrap();
        let date = s.snapshot.as_ref().unwrap().date;
        s.members.entry(kol.to_string()).or_default().push(reason);
        if let Some((source, fetched_at)) = row {
            s.rows.insert(
                kol.to_string(),
                MetricRow {
                    date,
                    kol: kol.to_string(),
                    source,
                    fetched_at,
                    metrics: Metrics::default(),
                },
            );
        }
    }
}

fn open_snapshot(s: &SnapState, date: NaiveDate) -> Result<(), SnapshotsError> {
    match &s.snapshot {
        Some(sn) if sn.date == date && sn.status == SnapshotStatus::Open => Ok(()),
        Some(sn) if sn.date == date => Err(SnapshotsError::Closed { date }),
        _ => Err(SnapshotsError::NoSnapshot { date }),
    }
}

#[async_trait]
impl SnapshotsApi for FakeSnapshots {
    async fn open(&self, _: &Person, date: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        self.open_day(date);
        Ok(self.state.lock().unwrap().snapshot.clone().unwrap())
    }

    async fn close(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        self.close_day();
        Ok(self.state.lock().unwrap().snapshot.clone().unwrap())
    }

    async fn put_metrics(
        &self,
        _: &Person,
        date: NaiveDate,
        kol: &str,
        metrics: &Metrics,
        source: MetricSource,
        fetched_at: DateTime<Utc>,
    ) -> Result<(), SnapshotsError> {
        let mut s = self.state.lock().unwrap();
        open_snapshot(&s, date)?;
        s.writes += 1;
        s.rows.insert(
            kol.to_string(),
            MetricRow {
                date,
                kol: kol.to_string(),
                source,
                fetched_at,
                metrics: metrics.clone(),
            },
        );
        Ok(())
    }

    async fn set_members(
        &self,
        _: &Person,
        date: NaiveDate,
        reason: MemberReason,
        kols: &[KolId],
    ) -> Result<(), SnapshotsError> {
        let mut s = self.state.lock().unwrap();
        open_snapshot(&s, date)?;
        s.writes += 1;
        for v in s.members.values_mut() {
            v.retain(|r| *r != reason);
        }
        for k in kols {
            s.members.entry(k.clone()).or_default().push(reason);
        }
        s.members.retain(|_, v| !v.is_empty());
        Ok(())
    }

    async fn copy_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
    ) -> Result<(), SnapshotsError> {
        Ok(())
    }

    async fn snapshot(&self, _: &Person, date: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        let s = self.state.lock().unwrap();
        match &s.snapshot {
            Some(sn) if sn.date == date => Ok(SnapshotView {
                snapshot: sn.clone(),
                members: s
                    .members
                    .iter()
                    .map(|(k, r)| Member {
                        kol: k.clone(),
                        reasons: r.clone(),
                        row: s.rows.get(k).cloned(),
                    })
                    .collect(),
            }),
            _ => Err(SnapshotsError::NoSnapshot { date }),
        }
    }

    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Ok(None)
    }

    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Ok(vec![])
    }

    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        Ok(vec![])
    }
}

/// `Api` handles for the fakes, built the way the server builds them.
pub fn apis(
    registry: Arc<FakeRegistry>,
    snapshots: Arc<FakeSnapshots>,
) -> (Api<dyn RegistryApi>, Api<dyn SnapshotsApi>) {
    let apis = Apis::new(Registry::new("test", &[]));
    apis.provide::<dyn RegistryApi>(
        "registry",
        vec![("default", registry as Arc<dyn RegistryApi>)],
        Arc::new(api_registry::stub::Stub),
    )
    .unwrap();
    apis.provide::<dyn SnapshotsApi>(
        "snapshots",
        vec![("default", snapshots as Arc<dyn SnapshotsApi>)],
        Arc::new(api_snapshots::stub::Stub),
    )
    .unwrap();
    (apis.get().unwrap(), apis.get().unwrap())
}

// ------------------------------------------------------------ fake KE site

#[path = "../../../tests/fake_site.rs"]
mod fake_site;
#[allow(unused_imports)]
pub use fake_site::*;

/// The saved board with every row after the first `keep` turned into a non-row.
pub fn board_with_rows(keep: usize) -> String {
    let mut n = 0;
    let marker = "<tr class=\"board-row\"";
    let mut out = String::new();
    let mut rest = BOARD_HTML;
    while let Some(at) = rest.find(marker) {
        n += 1;
        out.push_str(&rest[..at]);
        out.push_str(if n <= keep {
            marker
        } else {
            "<tr class=\"gone\""
        });
        rest = &rest[at + marker.len()..];
    }
    out.push_str(rest);
    out
}
