//! The run and the read calls.

use crate::{
    client::{Client, Options},
    metrics,
    models::{HistoryResp, KolListResp, WalletResp},
    store::{self, StoredKol},
};
use api_registry::{KolId, RegistryApi};
use api_snapshots::{MetricSource, SnapshotStatus, SnapshotsApi};
use api_solanatracker::{
    KolList, KolRun, KolRunStatus, ListedKol, SolanatrackerApi, StError, StFailure, StRun,
    StRunStatus, StWallet, WalletSummary,
};
use arena_platform::{Api, Person};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use sqlx::PgPool;
use std::{collections::HashMap, fmt::Display};

pub struct Solanatracker {
    pool: PgPool,
    registry: Api<dyn RegistryApi>,
    snapshots: Api<dyn SnapshotsApi>,
    /// `None`: `SOLANA_TRACKER_API_KEY` is not set.
    client: Option<Client>,
    /// The time source; tests set a fixed one.
    clock: fn() -> DateTime<Utc>,
}

fn internal(e: impl Display) -> StError {
    StError::Internal {
        message: e.to_string(),
    }
}

/// The KOL list endpoint: 30-day realized PnL, first page (100 KOLs).
const LIST_PATH: &str = "/v2/pnl/leaderboard/kols/period?period=30d";

/// Failures that end the run: nothing more can be read with this key.
fn ends_run(f: &StFailure) -> bool {
    matches!(
        f,
        StFailure::KeyRejected | StFailure::PlanLacksPnl | StFailure::RateLimited
    )
}

impl Solanatracker {
    pub fn new(
        pool: PgPool,
        registry: Api<dyn RegistryApi>,
        snapshots: Api<dyn SnapshotsApi>,
        key: Option<String>,
        opts: Options,
    ) -> Result<Self, String> {
        let client = key.map(|k| Client::new(k, opts)).transpose()?;
        Ok(Solanatracker {
            pool,
            registry,
            snapshots,
            client,
            clock: Utc::now,
        })
    }

    /// Replaces the time source (tests).
    pub fn with_clock(mut self, clock: fn() -> DateTime<Utc>) -> Self {
        self.clock = clock;
        self
    }

    fn client(&self) -> Result<&Client, StFailure> {
        self.client.as_ref().ok_or(StFailure::NotConfigured)
    }

    async fn read_wallet(&self, address: &str) -> Result<WalletResp, StFailure> {
        self.client()?
            .get(&format!("/v2/pnl/wallets/{address}"))
            .await
    }

    async fn read_history(&self, address: &str) -> Result<HistoryResp, StFailure> {
        self.client()?
            .get(&format!("/v2/pnl/wallets/{address}/history?period=30d"))
            .await
    }

    /// One KOL: summary, then history, then the snapshot row.
    #[allow(clippy::too_many_arguments)]
    async fn read_kol(
        &self,
        snapshots: &dyn SnapshotsApi,
        person: &Person,
        date: NaiveDate,
        kol: &KolId,
        wallet: &str,
        pnl_rank: Option<i32>,
        in_top: bool,
    ) -> Result<(), StFailure> {
        let summary = self.read_wallet(wallet).await?;
        if summary.indexed == Some(false) || summary.summary.is_none() {
            return Err(StFailure::BadResponse {
                message: "wallet not indexed yet".into(),
            });
        }
        let history = self.read_history(wallet).await?;
        let now = (self.clock)();
        let m = metrics::build(&summary, &history, now, pnl_rank, in_top)?;
        snapshots
            .put_metrics(person, date, kol, &m, MetricSource::Solanatracker, now)
            .await
            .map_err(|e| StFailure::BadResponse {
                message: format!("snapshot: {e}"),
            })
    }
}

#[async_trait]
impl SolanatrackerApi for Solanatracker {
    async fn run(&self, person: &Person, date: NaiveDate) -> Result<StRun, StError> {
        let started_at = (self.clock)();
        let mut run = StRun {
            date,
            started_at,
            finished_at: started_at,
            status: StRunStatus::Ok,
            failure: None,
            kols: vec![],
            list_rows: 0,
            calls: 0,
        };
        let Some(client) = &self.client else {
            run.status = StRunStatus::Failed;
            run.failure = Some(StFailure::NotConfigured);
            return Ok(run);
        };
        let calls_before = client.calls();
        let registry = self.registry.pick(person);
        let snapshots = self.snapshots.pick(person);

        // The date must be open; its members tell whom to read.
        let view = snapshots.snapshot(person, date).await.map_err(internal)?;
        if view.snapshot.status != SnapshotStatus::Open {
            return Err(internal(format!("snapshot {date} is closed")));
        }

        // 1. The day's list.
        let mut stop: Option<StFailure> = None;
        let mut some_failed = false;
        let mut list: Vec<StoredKol> = vec![];
        match client.get::<KolListResp>(LIST_PATH).await {
            Ok(resp) => {
                let fetched_at = (self.clock)();
                list = resp
                    .traders
                    .into_iter()
                    .map(|t| {
                        let id = t.identity.unwrap_or_default();
                        StoredKol {
                            wallet: t.wallet,
                            x_handle: id.x_handle(),
                            fomo_handle: id.fomo_handle(),
                            name: id.name.filter(|n| !n.trim().is_empty()),
                        }
                    })
                    .collect();
                store::replace_list(&self.pool, date, fetched_at, &list)
                    .await
                    .map_err(internal)?;
                run.list_rows = list.len() as u32;
            }
            Err(f) => {
                if ends_run(&f) {
                    stop = Some(f.clone());
                } else {
                    some_failed = true;
                }
                run.failure = Some(f);
            }
        }
        let by_wallet: HashMap<&str, usize> = list
            .iter()
            .enumerate()
            .map(|(i, k)| (k.wallet.as_str(), i))
            .collect();
        let by_handle: HashMap<String, usize> = list
            .iter()
            .enumerate()
            .filter_map(|(i, k)| Some((k.x_handle.as_ref()?.to_lowercase(), i)))
            .collect();

        // 2. Members, their wallets and handles.
        let ids: Vec<KolId> = view.members.iter().map(|m| m.kol.clone()).collect();
        let profiles = registry.list_kols(person, &ids).await.map_err(internal)?;
        let mut ok = 0;
        for member in &view.members {
            let Some(profile) = profiles.iter().find(|p| p.id == member.kol) else {
                continue;
            };
            let Some(main) = &profile.main_wallet else {
                continue;
            };
            let wallet = main.address.clone();
            let place = by_wallet.get(wallet.as_str()).copied().or_else(|| {
                let h = profile
                    .x_handle
                    .as_ref()?
                    .trim_start_matches('@')
                    .to_lowercase();
                by_handle.get(&h).copied()
            });
            let in_top = place.is_some();
            let pnl_rank = place.map(|i| i as i32 + 1);

            // Metric precedence: KE wrote this KOL for this date (since the snapshot was
            // last opened): keep its row, only set the «in top today» mark.
            let ke_wrote = member.row.as_ref().filter(|r| {
                r.source == MetricSource::Kolexplorer && r.fetched_at >= view.snapshot.opened_at
            });
            if let Some(row) = ke_wrote {
                if in_top && !row.metrics.in_top_today {
                    let mut m = row.metrics.clone();
                    m.in_top_today = true;
                    snapshots
                        .put_metrics(person, date, &member.kol, &m, row.source, row.fetched_at)
                        .await
                        .map_err(internal)?;
                }
                run.kols.push(KolRun {
                    kol: member.kol.clone(),
                    wallet,
                    status: KolRunStatus::Skipped,
                    failure: None,
                });
                continue;
            }

            if let Some(f) = &stop {
                run.kols.push(KolRun {
                    kol: member.kol.clone(),
                    wallet,
                    status: KolRunStatus::Failed,
                    failure: Some(f.clone()),
                });
                continue;
            }
            match self
                .read_kol(
                    &*snapshots,
                    person,
                    date,
                    &member.kol,
                    &wallet,
                    pnl_rank,
                    in_top,
                )
                .await
            {
                Ok(()) => {
                    ok += 1;
                    run.kols.push(KolRun {
                        kol: member.kol.clone(),
                        wallet,
                        status: KolRunStatus::Ok,
                        failure: None,
                    });
                }
                Err(f) => {
                    tracing::warn!(kol = %member.kol, "Solana Tracker read failed: {f}");
                    if ends_run(&f) {
                        stop = Some(f.clone());
                        run.failure = Some(f.clone());
                    }
                    some_failed = true;
                    run.kols.push(KolRun {
                        kol: member.kol.clone(),
                        wallet,
                        status: KolRunStatus::Failed,
                        failure: Some(f),
                    });
                }
            }
        }

        run.status = match &stop {
            // 429 stops the run as partial. A rejected key or a plan without PnL is a failed
            // run, unless some KOLs were already written.
            Some(StFailure::RateLimited) => StRunStatus::Partial,
            Some(_) if ok == 0 => StRunStatus::Failed,
            Some(_) => StRunStatus::Partial,
            None if some_failed => StRunStatus::Partial,
            None => StRunStatus::Ok,
        };
        run.calls = client.calls() - calls_before;
        run.finished_at = (self.clock)();
        Ok(run)
    }

    async fn kol_list(&self, _: &Person, date: NaiveDate) -> Result<Option<KolList>, StError> {
        let Some((list_date, fetched_at, kols)) = store::list_on_or_before(&self.pool, date)
            .await
            .map_err(internal)?
        else {
            return Ok(None);
        };
        Ok(Some(KolList {
            date: list_date,
            fetched_at,
            kols: kols
                .into_iter()
                .map(|k| ListedKol {
                    name: k.name,
                    x_handle: k.x_handle,
                    wallet: k.wallet,
                })
                .collect(),
        }))
    }

    async fn wallet_by_handle(&self, _: &Person, x: &str) -> Result<Option<StWallet>, StError> {
        Ok(store::wallet_by_handle(&self.pool, x)
            .await
            .map_err(internal)?
            .map(|k| StWallet {
                address: k.wallet,
                name: k.name,
                x_handle: k.x_handle,
                fomo_handle: k.fomo_handle,
            }))
    }

    async fn wallet_summary(&self, _: &Person, address: &str) -> Result<WalletSummary, StError> {
        if !is_address(address) {
            return Err(internal(format!("not a Solana address: {address}")));
        }
        let resp = self
            .read_wallet(address)
            .await
            .map_err(|failure| StError::Source { failure })?;
        // Not indexed yet (queued): no trades known.
        let Some(summary) = resp.summary.filter(|_| resp.indexed != Some(false)) else {
            return Ok(WalletSummary {
                address: address.to_string(),
                has_trades: false,
                last_trade_at: None,
            });
        };
        let trades = summary
            .counts
            .as_ref()
            .and_then(|c| c.trades)
            .unwrap_or(0.0);
        let last_trade_at: Option<DateTime<Utc>> = summary
            .timing
            .and_then(|t| t.last_trade)
            .and_then(|ms| Utc.timestamp_millis_opt(ms).single());
        Ok(WalletSummary {
            address: address.to_string(),
            has_trades: trades > 0.0,
            last_trade_at,
        })
    }
}

/// 32..=44 base58 characters.
fn is_address(s: &str) -> bool {
    (32..=44).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() && !matches!(b, b'0' | b'O' | b'I' | b'l'))
}
