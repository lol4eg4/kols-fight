//! The run: leaderboard, profiles, registry, snapshot.

use crate::{
    http::{FetchError, Http, Options},
    parse::{self, BoardRow},
    store::{self, CachedProfile},
};
use api_kolexplorer::{KeError, KeRun, KeRunStatus, KolexplorerApi};
use api_registry::{KolId, KolUpsert, RegistryApi, WalletSource};
use api_snapshots::{MemberReason, MetricSource, Metrics, SnapshotStatus, SnapshotsApi};
use arena_platform::{Api, Person};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use std::{collections::HashSet, fmt::Display};

/// KOLs written per run: the top of the leaderboard.
pub const TOP: usize = 100;

pub struct Kolexplorer {
    pool: PgPool,
    registry: Api<dyn RegistryApi>,
    snapshots: Api<dyn SnapshotsApi>,
    http: Http,
}

fn internal(e: impl Display) -> KeError {
    KeError::Internal {
        message: e.to_string(),
    }
}

impl Kolexplorer {
    pub fn new(
        pool: PgPool,
        registry: Api<dyn RegistryApi>,
        snapshots: Api<dyn SnapshotsApi>,
        opts: Options,
    ) -> Result<Self, String> {
        Ok(Kolexplorer {
            pool,
            registry,
            snapshots,
            http: Http::new(opts)?,
        })
    }

    fn failed(date: NaiveDate, started_at: DateTime<Utc>, error: String) -> KeRun {
        KeRun {
            date,
            started_at,
            finished_at: Utc::now(),
            status: KeRunStatus::Failed,
            rows: 0,
            profiles_failed: 0,
            error: Some(error),
        }
    }

    /// Registry KOL for a leaderboard row: by wallet, else by X handle, else a new one.
    async fn resolve(
        registry: &dyn RegistryApi,
        p: &Person,
        row: &BoardRow,
    ) -> Result<KolId, KeError> {
        if let Some(id) = registry
            .find_by_wallet(p, &row.address)
            .await
            .map_err(internal)?
        {
            return Ok(id);
        }
        if let Some(h) = &row.x_handle
            && let Some(id) = registry.find_by_handle(p, h).await.map_err(internal)?
        {
            return Ok(id);
        }
        registry
            .upsert_kol(
                p,
                &KolUpsert {
                    id: None,
                    name: row.name.clone(),
                    x_handle: row.x_handle.clone(),
                    links: api_registry::KolLinks {
                        kolexplorer: Some(row.slug.clone()),
                        ..Default::default()
                    },
                },
            )
            .await
            .map_err(internal)
    }

    /// Links the KOL to its KE profile and publishes the KE wallet as its main wallet.
    /// Returns the KOLs whose main wallet stays another one: KE's numbers are not theirs.
    async fn sync_registry(
        registry: &dyn RegistryApi,
        p: &Person,
        kols: &[(KolId, &BoardRow)],
    ) -> Result<HashSet<KolId>, KeError> {
        let mut other_main = HashSet::new();
        let ids: Vec<KolId> = kols.iter().map(|(id, _)| id.clone()).collect();
        let profiles = registry.list_kols(p, &ids).await.map_err(internal)?;
        for (id, row) in kols {
            let Some(prof) = profiles.iter().find(|k| &k.id == id) else {
                continue;
            };
            if prof.links.kolexplorer.is_none() {
                let mut links = prof.links.clone();
                links.kolexplorer = Some(row.slug.clone());
                registry
                    .upsert_kol(
                        p,
                        &KolUpsert {
                            id: Some(id.clone()),
                            name: prof.name.clone(),
                            x_handle: prof.x_handle.clone().or_else(|| row.x_handle.clone()),
                            links,
                        },
                    )
                    .await
                    .map_err(internal)?;
            }
            match &prof.main_wallet {
                Some(w) if w.address == row.address => continue,
                // A network wallet that differs is the wallets plugin's conflict to resolve.
                Some(w) if w.source == WalletSource::Network => {
                    tracing::warn!(kol = %id, "KE wallet differs from the network wallet; left to wallets");
                    other_main.insert(id.clone());
                    continue;
                }
                _ => {}
            }
            // Other wallets keep their own source: a network wallet stays the network's.
            let others: Vec<(String, WalletSource)> = prof
                .other_wallets
                .iter()
                .filter(|w| w.address != row.address)
                .map(|w| (w.address.clone(), w.source))
                .collect();
            match registry
                .publish_wallets(
                    p,
                    id,
                    Some((&row.address, WalletSource::Kolexplorer)),
                    &others,
                )
                .await
            {
                Ok(()) => {}
                Err(api_registry::RegistryError::WalletTaken { address, owner }) => {
                    tracing::warn!(kol = %id, %owner, %address, "KE wallet belongs to another KOL; not published");
                    other_main.insert(id.clone());
                }
                Err(e) => return Err(internal(e)),
            }
        }
        Ok(other_main)
    }

    fn metrics(row: &BoardRow, fetched_at: DateTime<Utc>, prof: Option<CachedProfile>) -> Metrics {
        let last_min = row
            .last_tx_ts
            .map(|ts| ((fetched_at.timestamp() - ts) / 60).max(0));
        Metrics {
            d1: row.d1,
            d7: row.d7,
            d30: row.d30,
            wr7: row.wr7,
            wr30: row.wr30,
            avg_pos: prof.and_then(|p| p.avg_pos),
            hold_s: prof.and_then(|p| p.hold_s),
            last_min_at_fetch: last_min,
            traded_24h: last_min.map(|m| m <= 1440),
            unreal: row.unreal,
            total: row.total,
            pnl_rank: Some(row.rank as i32),
            ..Default::default()
        }
    }
}

#[async_trait]
impl KolexplorerApi for Kolexplorer {
    async fn run(&self, person: &Person, date: NaiveDate) -> Result<KeRun, KeError> {
        let started_at = Utc::now();
        let registry = self.registry.pick(person);
        let snapshots = self.snapshots.pick(person);

        // The date must be open: check before reading or writing anything.
        let view = snapshots.snapshot(person, date).await.map_err(internal)?;
        if view.snapshot.status != SnapshotStatus::Open {
            return Err(internal(format!("snapshot {date} is closed")));
        }

        let html = match self.http.leaderboard().await {
            Ok(h) => h,
            Err(e) => {
                return Ok(Self::failed(date, started_at, format!("leaderboard: {e}")));
            }
        };
        let fetched_at = Utc::now();
        let board = match parse::parse_leaderboard(&html) {
            Ok(b) => b,
            Err(e) => return Ok(Self::failed(date, started_at, format!("leaderboard: {e}"))),
        };

        // From here the page is good. Top rows -> registry KOLs (one row per KOL).
        let mut seen = HashSet::new();
        let mut top: Vec<(KolId, &BoardRow)> = Vec::new();
        for row in board.iter().take(TOP) {
            let id = Self::resolve(&*registry, person, row).await?;
            if seen.insert(id.clone()) {
                top.push((id, row));
            }
        }
        let other_main = Self::sync_registry(&*registry, person, &top).await?;
        let ids: Vec<KolId> = top.iter().map(|(id, _)| id.clone()).collect();
        snapshots
            .set_members(person, date, MemberReason::Top100, &ids)
            .await
            .map_err(internal)?;

        // Profiles: cache first, then one request at a time.
        let mut profiles_failed = 0u32;
        let mut written = 0u32;
        for (id, row) in &top {
            // KE's numbers are of its wallet; the main wallet's come from Solana Tracker.
            if other_main.contains(id) {
                continue;
            }
            let now = Utc::now();
            let prof = match store::cached_profile(&self.pool, &row.slug, now)
                .await
                .map_err(internal)?
            {
                Some(p) => Some(p),
                None => match self.read_profile(&row.slug).await {
                    Ok(p) => {
                        store::cache_profile(&self.pool, &row.slug, now, p)
                            .await
                            .map_err(internal)?;
                        Some(p)
                    }
                    Err(e) => {
                        tracing::warn!(slug = %row.slug, "profile not read: {e}");
                        profiles_failed += 1;
                        None
                    }
                },
            };
            snapshots
                .put_metrics(
                    person,
                    date,
                    id,
                    &Self::metrics(row, fetched_at, prof),
                    MetricSource::Kolexplorer,
                    fetched_at,
                )
                .await
                .map_err(internal)?;
            written += 1;
        }
        store::replace_board(&self.pool, &board, fetched_at)
            .await
            .map_err(internal)?;

        Ok(KeRun {
            date,
            started_at,
            finished_at: Utc::now(),
            status: KeRunStatus::Ok,
            rows: written,
            profiles_failed,
            error: None,
        })
    }

    async fn wallet_by_handle(&self, _: &Person, x: &str) -> Result<Option<String>, KeError> {
        store::wallet_by_handle(&self.pool, x)
            .await
            .map_err(internal)
    }
}

impl Kolexplorer {
    async fn read_profile(&self, slug: &str) -> Result<CachedProfile, String> {
        let html = self
            .http
            .profile(slug)
            .await
            .map_err(|e: FetchError| e.to_string())?;
        let p = parse::parse_profile(&html).map_err(|e| e.to_string())?;
        Ok(CachedProfile {
            avg_pos: p.avg_pos,
            hold_s: p.hold_s,
        })
    }
}
