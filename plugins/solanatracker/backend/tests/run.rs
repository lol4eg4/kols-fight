mod common;
use api_registry::WalletSource;
use api_snapshots::{MemberReason, MetricSource, Metrics};
use api_solanatracker::{KolRunStatus, SolanatrackerApi, StError, StFailure, StRunStatus};
use arena_platform::{Person, testing::TestDb};
use chrono::{Duration as Dur, Utc};
use common::*;
use plugin_solanatracker::{MIGRATOR, Options, Solanatracker};
use std::{sync::Arc, time::Duration};

const ADMIN: Person = Person::SYSTEM;

const CENTED: &str = "CyaE1VxvBrahnPWkqm5VsdCvyS2QmNht2UFrKJHga54o";
const DECU: &str = "4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud9";
const ZERO: &str = "4xY9T1Q7foJzJsJ6YZDSsfp9zkzeZsXnxd45SixduMmr";

struct Rig {
    db: TestDb,
    st: FakeSt,
    registry: Arc<FakeRegistry>,
    snapshots: Arc<FakeSnapshots>,
    svc: Solanatracker,
}

async fn rig_with(key: Option<&str>, interval_ms: u64) -> Rig {
    let db = TestDb::with_plugins(&[("solanatracker", &MIGRATOR)]).await;
    let st = FakeSt::start().await;
    st.set_list(200, LIST_JSON);
    let registry = Arc::new(FakeRegistry::default());
    let snapshots = Arc::new(FakeSnapshots::default());
    snapshots.open_day(day());
    let (r, s) = apis(registry.clone(), snapshots.clone());
    let svc = Solanatracker::new(
        db.pool().clone(),
        r,
        s,
        key.map(str::to_string),
        Options {
            base_url: st.url.clone(),
            min_interval: Duration::from_millis(interval_ms),
            retry_backoff: Duration::from_millis(20),
            ..Options::default()
        },
    )
    .unwrap()
    .with_clock(clock);
    Rig {
        db,
        st,
        registry,
        snapshots,
        svc,
    }
}

async fn rig() -> Rig {
    rig_with(Some(KEY), 5).await
}

impl Rig {
    /// A KOL with a main wallet, a member of today's Top 100.
    fn member(&self, id: &str, wallet: &str) {
        self.registry
            .add(id, id, None, Some((wallet, WalletSource::Kolexplorer)));
        self.snapshots.seed(id, MemberReason::Top100, None);
    }
}

fn wallet_path(a: &str) -> String {
    format!("/v2/pnl/wallets/{a}")
}

fn history_path(a: &str) -> String {
    format!("/v2/pnl/wallets/{a}/history")
}

#[tokio::test]
async fn success_writes_metrics_and_the_list() {
    let r = rig().await;
    r.member("cented", CENTED);
    r.member("decu", DECU);
    // A member with no wallet is left out.
    r.registry.add("nowallet", "nowallet", None, None);
    r.snapshots.seed("nowallet", MemberReason::Top100, None);
    r.st.set_traded(CENTED);
    r.st.set_traded(DECU);

    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Ok, "{:?}", run.failure);
    assert_eq!(run.failure, None);
    assert_eq!(run.list_rows, 100);
    assert_eq!(run.kols.len(), 2);
    assert!(run.kols.iter().all(|k| k.status == KolRunStatus::Ok));
    // 1 list + (summary + history) for each of 2 KOLs.
    assert_eq!(run.calls, 5);
    assert_eq!(r.st.total_hits(), 5);
    // Every request carried the key.
    let keys = r.st.state.keys.lock().unwrap().clone();
    assert!(keys.iter().all(|k| k.as_deref() == Some(KEY)), "{keys:?}");

    let row = r.snapshots.row("decu").unwrap();
    assert_eq!(row.source, MetricSource::Solanatracker);
    let m = &row.metrics;
    // From the Tekkerrss fixtures: 7 and 30 days ending on the fetch date.
    assert!((m.d7.unwrap() - 17670.53).abs() < 0.01, "{:?}", m.d7);
    assert!((m.d30.unwrap() - 182676.78).abs() < 0.01, "{:?}", m.d30);
    assert!((m.vol30.unwrap() - 1231796.02).abs() < 0.01);
    assert_eq!(m.days30, Some(30));
    assert_eq!(m.traded_24h, Some(true));
    assert_eq!(m.unreal, Some(-284357.74));
    assert_eq!(m.total, Some(475791.44));
    assert_eq!(m.avg_pos, Some(464.48));
    assert_eq!((m.wins, m.losses), (Some(415), Some(519)));
    assert!((m.wr_all.unwrap() - 0.4434).abs() < 1e-12);
    assert_eq!(
        m.last_min_at_fetch,
        Some((clock().timestamp_millis() - 1792392188561) / 60_000)
    );
    assert_eq!(m.pnl_rank, Some(2));
    assert!(m.in_top_today);
    // KE-only fields stay empty.
    assert_eq!((m.d1, m.wr7, m.wr30, m.hold_s), (None, None, None, None));
    assert_eq!(r.snapshots.row("cented").unwrap().metrics.pnl_rank, Some(1));
    assert!(r.snapshots.row("nowallet").is_none());

    // The list is stored for the date.
    let list = r.svc.kol_list(&ADMIN, day()).await.unwrap().unwrap();
    assert_eq!((list.date, list.kols.len()), (day(), 100));
    assert_eq!(list.fetched_at, clock());
    assert_eq!(list.kols[0].wallet, CENTED);
    assert_eq!(list.kols[0].x_handle.as_deref(), Some("Cented7"));
    assert_eq!(list.kols[0].name.as_deref(), Some("Cented"));
    // A listed trader with identity null is kept, without a name or handle.
    assert_eq!(list.kols[59].x_handle, None);
    assert_eq!(list.kols[59].name, None);
    r.db.close().await;
}

#[tokio::test]
async fn missing_key_is_not_configured() {
    let r = rig_with(None, 5).await;
    r.member("decu", DECU);
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Failed);
    assert_eq!(run.failure, Some(StFailure::NotConfigured));
    assert_eq!(
        (run.calls, r.st.total_hits(), r.snapshots.writes()),
        (0, 0, 0)
    );
    assert_eq!(
        r.svc.wallet_summary(&ADMIN, DECU).await,
        Err(StError::Source {
            failure: StFailure::NotConfigured
        })
    );
    r.db.close().await;
}

#[tokio::test]
async fn key_rejected_401_fails_the_run() {
    let r = rig().await;
    r.member("decu", DECU);
    r.st.set_list(401, r#"{"error":"bad key"}"#);
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Failed);
    assert_eq!(run.failure, Some(StFailure::KeyRejected));
    assert_eq!(run.failure.unwrap().to_string(), "key rejected");
    // Not retried, nothing else asked, nothing written.
    assert_eq!((run.calls, r.st.total_hits()), (1, 1));
    assert_eq!(r.snapshots.writes(), 0);
    assert_eq!(run.kols[0].status, KolRunStatus::Failed);
    assert_eq!(run.kols[0].failure, Some(StFailure::KeyRejected));
    assert!(r.svc.kol_list(&ADMIN, day()).await.unwrap().is_none());
    r.db.close().await;
}

#[tokio::test]
async fn key_rejected_on_a_wallet_call() {
    let r = rig().await;
    r.member("decu", DECU);
    r.st.set_wallet(DECU, 401, "{}");
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Failed);
    assert_eq!(run.failure, Some(StFailure::KeyRejected));
    assert_eq!(r.st.hits_on(&wallet_path(DECU)), 1);
    assert_eq!(r.snapshots.writes(), 0);
    r.db.close().await;
}

#[tokio::test]
async fn plan_without_pnl_403_fails_the_run() {
    let r = rig().await;
    r.member("cented", CENTED);
    r.member("decu", DECU);
    r.st.set_wallet(CENTED, 403, r#"{"error":"plan"}"#);
    r.st.set_traded(DECU);
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Failed);
    assert_eq!(run.failure, Some(StFailure::PlanLacksPnl));
    assert_eq!(run.failure.unwrap().to_string(), "plan lacks PnL");
    // The run stopped at the first 403: no retry, the second KOL was not asked.
    assert_eq!(r.st.hits_on(&wallet_path(CENTED)), 1);
    assert_eq!(r.st.hits_on(&wallet_path(DECU)), 0);
    assert!(
        run.kols
            .iter()
            .all(|k| k.status == KolRunStatus::Failed
                && k.failure == Some(StFailure::PlanLacksPnl))
    );
    assert_eq!(r.snapshots.writes(), 0);
    r.db.close().await;
}

#[tokio::test]
async fn rate_limit_429_stops_the_run_as_partial() {
    let r = rig().await;
    r.member("cented", CENTED);
    r.member("decu", DECU);
    r.member("zero", ZERO);
    r.st.set_traded(CENTED);
    r.st.set_wallet(DECU, 429, "{}");
    r.st.set_traded(ZERO);
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Partial);
    assert_eq!(run.failure, Some(StFailure::RateLimited));
    let by = |id: &str| run.kols.iter().find(|k| k.kol == id).unwrap();
    assert_eq!(by("cented").status, KolRunStatus::Ok);
    assert_eq!(by("decu").failure, Some(StFailure::RateLimited));
    assert_eq!(by("zero").failure, Some(StFailure::RateLimited));
    // 429 is not retried and the rest of the run is not asked.
    assert_eq!(r.st.hits_on(&wallet_path(DECU)), 1);
    assert_eq!(r.st.hits_on(&wallet_path(ZERO)), 0);
    assert!(r.snapshots.row("cented").is_some());
    assert!(r.snapshots.row("decu").is_none());
    r.db.close().await;
}

#[tokio::test]
async fn server_error_5xx_is_tried_twice() {
    let r = rig().await;
    r.member("cented", CENTED);
    r.member("decu", DECU);
    r.member("zero", ZERO);
    // One 503, then fine: the retry saves the KOL.
    r.st.set_reply(
        &wallet_path(CENTED),
        Reply::FailFirst(1, 503, WALLET_JSON.into()),
    );
    r.st.set_history(CENTED, 200, HISTORY_JSON);
    // Always 500: two tries, then the KOL fails and the run goes on.
    r.st.set_wallet(DECU, 500, "{}");
    r.st.set_traded(ZERO);

    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Partial);
    assert_eq!(run.failure, None);
    let by = |id: &str| run.kols.iter().find(|k| k.kol == id).unwrap();
    assert_eq!(by("cented").status, KolRunStatus::Ok);
    assert_eq!(r.st.hits_on(&wallet_path(CENTED)), 2);
    assert_eq!(by("decu").status, KolRunStatus::Failed);
    assert_eq!(
        by("decu").failure,
        Some(StFailure::Upstream { status: 500 })
    );
    assert_eq!(r.st.hits_on(&wallet_path(DECU)), 2);
    assert_eq!(by("zero").status, KolRunStatus::Ok);
    assert!(r.snapshots.row("decu").is_none());
    r.db.close().await;
}

#[tokio::test]
async fn client_error_4xx_is_not_retried() {
    let r = rig().await;
    r.member("decu", DECU);
    r.member("zero", ZERO);
    r.st.set_wallet(DECU, 404, r#"{"error":"nope"}"#);
    r.st.set_traded(ZERO);
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Partial);
    let decu = run.kols.iter().find(|k| k.kol == "decu").unwrap();
    assert_eq!(
        decu.failure,
        Some(StFailure::BadResponse {
            message: "HTTP 404".into()
        })
    );
    assert_eq!(r.st.hits_on(&wallet_path(DECU)), 1);
    assert_eq!(r.st.hits_on(&history_path(DECU)), 0);
    assert!(r.snapshots.row("zero").is_some());
    r.db.close().await;
}

#[tokio::test]
async fn failing_list_still_reads_member_wallets() {
    let r = rig().await;
    r.member("decu", DECU);
    r.st.set_list(500, "{}");
    r.st.set_traded(DECU);
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Partial);
    assert_eq!(run.failure, Some(StFailure::Upstream { status: 500 }));
    assert_eq!(run.list_rows, 0);
    assert_eq!(r.st.hits_on(LIST_PATH), 2);
    let m = r.snapshots.row("decu").unwrap().metrics;
    assert_eq!((m.pnl_rank, m.in_top_today), (None, false));
    assert!(m.d30.is_some());
    r.db.close().await;
}

#[tokio::test]
async fn unindexed_wallet_fails_that_kol_only() {
    let r = rig().await;
    r.member("decu", DECU);
    r.member("zero", ZERO);
    r.st.set_wallet(DECU, 200, UNINDEXED_JSON);
    r.st.set_traded(ZERO);
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Partial);
    let decu = run.kols.iter().find(|k| k.kol == "decu").unwrap();
    assert_eq!(decu.status, KolRunStatus::Failed);
    assert_eq!(
        decu.failure,
        Some(StFailure::BadResponse {
            message: "wallet not indexed yet".into()
        })
    );
    assert_eq!(r.st.hits_on(&history_path(DECU)), 0);
    assert!(r.snapshots.row("decu").is_none());
    assert!(r.snapshots.row("zero").is_some());
    r.db.close().await;
}

#[tokio::test]
async fn incomplete_history_gives_no_history_numbers() {
    let r = rig().await;
    r.member("decu", DECU);
    r.st.set_wallet(DECU, 200, WALLET_JSON);
    r.st.set_history(
        DECU,
        200,
        &HISTORY_JSON.replace("\"complete\":true", "\"complete\":false"),
    );
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Ok);
    let m = r.snapshots.row("decu").unwrap().metrics;
    assert_eq!(
        (m.d7, m.d30, m.vol30, m.days30, m.traded_24h),
        (None, None, None, None, None)
    );
    assert_eq!(m.unreal, Some(-284357.74));
    r.db.close().await;
}

#[tokio::test]
async fn few_trades_and_empty_history() {
    let r = rig().await;
    r.member("decu", DECU);
    r.st.set_wallet(DECU, 200, FEW_TRADES_JSON);
    r.st.set_history(DECU, 200, EMPTY_HISTORY_JSON);
    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Ok, "{run:?}");
    let m = r.snapshots.row("decu").unwrap().metrics;
    // No closed tokens: no win rate. Empty history: zero, not missing.
    assert_eq!(m.wr_all, None);
    assert_eq!(
        (m.d7, m.d30, m.days30, m.traded_24h),
        (Some(0.0), Some(0.0), Some(0), Some(false))
    );
    r.db.close().await;
}

#[tokio::test]
async fn kolexplorer_rows_win_and_get_the_top_today_mark() {
    let r = rig().await;
    let now = Utc::now();
    // Cented: KE wrote it today and it is in the list -> kept, marked.
    r.registry.add(
        "cented",
        "cented",
        None,
        Some((CENTED, WalletSource::Kolexplorer)),
    );
    r.snapshots.seed(
        "cented",
        MemberReason::Top100,
        Some((MetricSource::Kolexplorer, now)),
    );
    r.snapshots
        .state
        .lock()
        .unwrap()
        .rows
        .get_mut("cented")
        .unwrap()
        .metrics = Metrics {
        d1: Some(5.0),
        d30: Some(99.0),
        ..Metrics::default()
    };
    // Decu: a KE row from before the day was opened -> not today's, ST reads it.
    r.registry.add(
        "decu",
        "decu",
        None,
        Some((DECU, WalletSource::Kolexplorer)),
    );
    r.snapshots.seed(
        "decu",
        MemberReason::Top100,
        Some((MetricSource::Kolexplorer, now - Dur::days(2))),
    );
    // Zero: nothing written yet.
    r.member("zero", ZERO);
    r.st.set_traded(DECU);
    r.st.set_traded(ZERO);

    let run = r.svc.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, StRunStatus::Ok);
    let by = |id: &str| run.kols.iter().find(|k| k.kol == id).unwrap();
    assert_eq!(by("cented").status, KolRunStatus::Skipped);
    assert_eq!(by("decu").status, KolRunStatus::Ok);
    assert_eq!(by("zero").status, KolRunStatus::Ok);
    // The skipped KOL cost no request.
    assert_eq!(r.st.hits_on(&wallet_path(CENTED)), 0);
    assert_eq!(run.calls, 1 + 2 * 2);

    let cented = r.snapshots.row("cented").unwrap();
    assert_eq!(cented.source, MetricSource::Kolexplorer);
    assert_eq!(
        (cented.metrics.d1, cented.metrics.d30),
        (Some(5.0), Some(99.0))
    );
    assert!(cented.metrics.in_top_today);
    assert_eq!(
        r.snapshots.row("decu").unwrap().source,
        MetricSource::Solanatracker
    );
    r.db.close().await;
}

#[tokio::test]
async fn closed_snapshot_is_an_error() {
    let r = rig().await;
    r.member("decu", DECU);
    r.snapshots.close_day();
    let err = r.svc.run(&ADMIN, day()).await.unwrap_err();
    assert!(matches!(err, StError::Internal { .. }), "{err:?}");
    assert_eq!(r.st.total_hits(), 0);
    r.db.close().await;
}

#[tokio::test]
async fn requests_are_spaced_by_the_limiter() {
    let r = rig_with(Some(KEY), 60).await;
    r.member("cented", CENTED);
    r.member("decu", DECU);
    r.st.set_traded(CENTED);
    r.st.set_traded(DECU);
    r.svc.run(&ADMIN, day()).await.unwrap();
    let hits = r.st.state.hits.lock().unwrap().clone();
    assert_eq!(hits.len(), 5);
    for w in hits.windows(2) {
        let gap = w[1].1 - w[0].1;
        assert!(
            gap >= Duration::from_millis(50),
            "gap {gap:?} between {w:?}"
        );
    }
    r.db.close().await;
}

#[tokio::test]
async fn default_limiter_stays_under_the_free_plans_three_per_second() {
    let o = Options::default();
    assert!(o.min_interval > Duration::from_millis(334));
    assert!(o.min_interval <= Duration::from_millis(500));
    assert_eq!(o.base_url, "https://data.solanatracker.io");
}

#[tokio::test]
async fn lists_are_stored_per_date() {
    let r = rig().await;
    let d1 = day();
    let d2 = d1 + Dur::days(2);
    r.svc.run(&ADMIN, d1).await.unwrap();

    // The next date has a shorter list.
    let short = {
        let mut v: serde_json::Value = serde_json::from_str(LIST_JSON).unwrap();
        v["traders"].as_array_mut().unwrap().truncate(3);
        v.to_string()
    };
    r.st.set_list(200, &short);
    r.snapshots.open_day(d2);
    r.svc.run(&ADMIN, d2).await.unwrap();

    let len = |l: Option<api_solanatracker::KolList>| l.map(|l| (l.date, l.kols.len()));
    assert_eq!(
        len(r.svc.kol_list(&ADMIN, d1).await.unwrap()),
        Some((d1, 100))
    );
    assert_eq!(
        len(r.svc.kol_list(&ADMIN, d2).await.unwrap()),
        Some((d2, 3))
    );
    // A date with no list gets the latest one before it; before the first, none.
    assert_eq!(
        len(r.svc.kol_list(&ADMIN, d1 + Dur::days(1)).await.unwrap()),
        Some((d1, 100))
    );
    assert_eq!(
        len(r.svc.kol_list(&ADMIN, d2 + Dur::days(9)).await.unwrap()),
        Some((d2, 3))
    );
    assert_eq!(
        r.svc.kol_list(&ADMIN, d1 - Dur::days(1)).await.unwrap(),
        None
    );

    // Running the same date again replaces its list.
    r.st.set_list(200, LIST_JSON);
    r.snapshots.open_day(d2);
    r.svc.run(&ADMIN, d2).await.unwrap();
    assert_eq!(
        len(r.svc.kol_list(&ADMIN, d2).await.unwrap()),
        Some((d2, 100))
    );
    r.db.close().await;
}

#[tokio::test]
async fn wallet_by_handle_matches_identity() {
    let r = rig().await;
    let list = serde_json::json!({"traders": [
        {"wallet": CENTED, "identity": {"name": "Cented", "twitter": "@Cented7"}},
        {"wallet": DECU, "identity": null},
        {"wallet": ZERO, "identity": {
            "name": "Zero", "twitter": "ZeroPnl",
            "platformProfiles": {"fomo": {"handle": "zero_on_fomo"}}}},
    ]})
    .to_string();
    r.st.set_list(200, &list);
    r.svc.run(&ADMIN, day()).await.unwrap();

    // Case-insensitive, with or without @, and the stored handle has no @.
    for q in ["cented7", "@CENTED7", "Cented7", " @cented7 "] {
        let w = r.svc.wallet_by_handle(&ADMIN, q).await.unwrap();
        let w = w.unwrap_or_else(|| panic!("{q}"));
        assert_eq!(w.address, CENTED);
        assert_eq!(w.x_handle.as_deref(), Some("Cented7"));
        assert_eq!(w.fomo_handle, None);
    }
    // The FOMO handle comes along.
    let z = r
        .svc
        .wallet_by_handle(&ADMIN, "@zeropnl")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (z.address.as_str(), z.name.as_deref()),
        (ZERO, Some("Zero"))
    );
    assert_eq!(z.fomo_handle.as_deref(), Some("zero_on_fomo"));
    // Unknown, empty, and the trader with no identity.
    assert_eq!(
        r.svc.wallet_by_handle(&ADMIN, "nobody").await.unwrap(),
        None
    );
    assert_eq!(r.svc.wallet_by_handle(&ADMIN, "@").await.unwrap(), None);
    r.db.close().await;
}

#[tokio::test]
async fn wallet_by_handle_without_a_list_is_none() {
    let r = rig().await;
    assert_eq!(
        r.svc.wallet_by_handle(&ADMIN, "decu0x").await.unwrap(),
        None
    );
    r.db.close().await;
}

#[tokio::test]
async fn wallet_summary_reads_one_wallet() {
    let r = rig().await;
    r.st.set_wallet(DECU, 200, FEW_TRADES_JSON);
    let s = r.svc.wallet_summary(&ADMIN, DECU).await.unwrap();
    assert_eq!(s.address, DECU);
    assert!(s.has_trades);
    assert_eq!(s.last_trade_at.unwrap().timestamp(), 1709135514);

    // Not indexed yet: no trades known.
    r.st.set_wallet(ZERO, 200, UNINDEXED_JSON);
    let s = r.svc.wallet_summary(&ADMIN, ZERO).await.unwrap();
    assert_eq!((s.has_trades, s.last_trade_at), (false, None));

    // Source failures keep their kind; bad input is rejected before any request.
    r.st.set_wallet(CENTED, 401, "{}");
    assert_eq!(
        r.svc.wallet_summary(&ADMIN, CENTED).await,
        Err(StError::Source {
            failure: StFailure::KeyRejected
        })
    );
    let before = r.st.total_hits();
    let err = r
        .svc
        .wallet_summary(&ADMIN, "not an address")
        .await
        .unwrap_err();
    assert!(matches!(err, StError::Internal { .. }), "{err:?}");
    assert_eq!(r.st.total_hits(), before);
    r.db.close().await;
}
