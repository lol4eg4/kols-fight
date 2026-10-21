mod common;
use api_kolexplorer::{KeError, KeRunStatus, KolexplorerApi};
use api_registry::WalletSource;
use api_snapshots::{MemberReason, MetricSource};
use arena_platform::{Person, testing::TestDb};
use common::*;
use plugin_kolexplorer::{Kolexplorer, MIGRATOR, Options};
use std::{sync::Arc, time::Duration};

const ADMIN: Person = Person::SYSTEM;

struct Rig {
    db: TestDb,
    site: FakeSite,
    registry: Arc<FakeRegistry>,
    snapshots: Arc<FakeSnapshots>,
    ke: Kolexplorer,
}

async fn rig(board: &str) -> Rig {
    let db = TestDb::with_plugins(&[("kolexplorer", &MIGRATOR)]).await;
    let site = FakeSite::start(board).await;
    let registry = Arc::new(FakeRegistry::default());
    let snapshots = Arc::new(FakeSnapshots::default());
    snapshots.open_day(day());
    let (r, s) = apis(registry.clone(), snapshots.clone());
    let ke = Kolexplorer::new(
        db.pool().clone(),
        r,
        s,
        Options {
            base_url: site.url.clone(),
            profile_gap: Duration::from_millis(20),
            ..Options::default()
        },
    )
    .unwrap();
    Rig {
        db,
        site,
        registry,
        snapshots,
        ke,
    }
}

#[tokio::test]
async fn success_writes_top_100() {
    let r = rig(BOARD_HTML).await;
    r.site.state.profiles.lock().unwrap().insert(
        "cented".into(),
        (
            200,
            PROFILE_HTML
                .replace("$342", "$1.5K")
                .replace("<b>15 s</b>", "<b>2 h</b>"),
        ),
    );
    r.registry.add(
        "decu-old",
        "Decu",
        Some("NotDecu"),
        Some((
            "OldWallet11111111111111111111111111111111",
            WalletSource::Prototype,
        )),
    );

    let run = r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, KeRunStatus::Ok, "{:?}", run.error);
    assert_eq!((run.rows, run.profiles_failed, run.error), (100, 0, None));

    // 100 members, 100 rows from KE, one leaderboard request, one request per profile.
    assert_eq!(r.snapshots.members_with(MemberReason::Top100).len(), 100);
    assert_eq!(r.site.board_hits(), 1);
    assert_eq!(r.site.profile_hits(), 100);
    let ua = r.site.state.user_agents.lock().unwrap().clone();
    assert!(ua[0].starts_with("KOLArena/"), "{ua:?}");

    // Decu is found by X handle (case-insensitive), keeps the id, gets the KE wallet and link.
    let row = r.snapshots.row("decu-old").expect("row for decu");
    assert_eq!(row.source, MetricSource::Kolexplorer);
    let m = &row.metrics;
    assert_eq!(m.d1, Some(15953.375041653));
    assert_eq!(m.d30, Some(553928.02266837));
    assert_eq!(m.avg_pos, Some(342.0));
    assert_eq!(m.hold_s, Some(15.0));
    assert_eq!(m.pnl_rank, Some(1));
    assert!(!m.in_top_today);
    assert!((m.wr30.unwrap() - 0.70669533169533).abs() < 1e-12);
    assert_eq!(
        m.last_min_at_fetch,
        Some((row.fetched_at.timestamp() - 1792381024) / 60)
    );
    assert_eq!(m.traded_24h, Some(m.last_min_at_fetch.unwrap() <= 1440));
    let decu = r.registry.get("decu-old").unwrap();
    assert_eq!(
        decu.main_wallet.unwrap().address,
        "4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud9"
    );
    assert_eq!(decu.links.kolexplorer.as_deref(), Some("decu"));

    // Slug-specific profile.
    let cented = r.snapshots.row("cented").unwrap();
    assert_eq!(
        (cented.metrics.avg_pos, cented.metrics.hold_s),
        (Some(1500.0), Some(7200.0))
    );
    assert_eq!(
        r.registry
            .get("cented")
            .unwrap()
            .links
            .kolexplorer
            .as_deref(),
        Some("cented")
    );

    // Wallet - handle list holds all 150 rows of the read.
    assert_eq!(
        r.ke.wallet_by_handle(&ADMIN, "@NOTDECU")
            .await
            .unwrap()
            .as_deref(),
        Some("4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud9")
    );
    assert_eq!(
        r.ke.wallet_by_handle(&ADMIN, "nobody_here").await.unwrap(),
        None
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM kolexplorer.board_row")
        .fetch_one(r.db.pool())
        .await
        .unwrap();
    assert_eq!(n, 150);
    r.db.close().await;
}

#[tokio::test]
async fn profiles_are_cached_for_24_hours() {
    let r = rig(BOARD_HTML).await;
    r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(r.site.profile_hits(), 100);

    // Same day again: leaderboard is read again, profiles come from the cache.
    r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(r.site.board_hits(), 2);
    assert_eq!(r.site.profile_hits(), 100);
    assert_eq!(
        r.snapshots.row("decu").unwrap().metrics.avg_pos,
        Some(342.0)
    );

    // Older than 24 hours: read again.
    sqlx::query("UPDATE kolexplorer.profile_cache SET fetched_at = now() - interval '25 hours' WHERE slug = 'decu'")
        .execute(r.db.pool())
        .await
        .unwrap();
    r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(r.site.profile_hits(), 101);
    r.db.close().await;
}

#[tokio::test]
async fn profile_requests_are_spaced() {
    let r = rig(&board_with_rows(60)).await; // 60 rows: top 60 only
    let run = r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.rows, 60);
    let hits = r.site.state.hits.lock().unwrap().clone();
    let times: Vec<_> = hits
        .iter()
        .filter(|(p, _)| p.starts_with("/kol/"))
        .map(|(_, t)| *t)
        .collect();
    assert_eq!(times.len(), 60);
    for w in times.windows(2) {
        assert!(
            w[1] - w[0] >= Duration::from_millis(18),
            "gap {:?}",
            w[1] - w[0]
        );
    }
    r.db.close().await;
}

#[tokio::test]
async fn under_50_rows_fails_and_writes_nothing() {
    let r = rig(&board_with_rows(49)).await;
    let run = r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, KeRunStatus::Failed);
    assert_eq!((run.rows, run.profiles_failed), (0, 0));
    assert!(run.error.unwrap().contains("49 rows"));
    assert_nothing_written(&r).await;
    r.db.close().await;
}

#[tokio::test]
async fn missing_attribute_fails_and_writes_nothing() {
    let html = BOARD_HTML.replacen("data-address=", "data-addr=", 1);
    let r = rig(&html).await;
    let run = r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, KeRunStatus::Failed);
    assert!(
        run.error
            .as_deref()
            .unwrap()
            .contains("data-address is missing"),
        "{:?}",
        run.error
    );
    assert_nothing_written(&r).await;
    r.db.close().await;
}

#[tokio::test]
async fn leaderboard_http_error_fails_and_writes_nothing() {
    let r = rig(BOARD_HTML).await;
    *r.site.state.board.lock().unwrap() = (503, "down".into());
    let run = r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, KeRunStatus::Failed);
    assert!(run.error.unwrap().contains("HTTP 503"));
    assert_nothing_written(&r).await;
    r.db.close().await;
}

async fn assert_nothing_written(r: &Rig) {
    assert_eq!(r.snapshots.writes(), 0);
    assert!(r.registry.kols.lock().unwrap().is_empty());
    assert_eq!(r.site.profile_hits(), 0);
    let n: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM kolexplorer.board_row) + (SELECT count(*) FROM kolexplorer.profile_cache)")
        .fetch_one(r.db.pool())
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn failed_profiles_leave_two_values_empty_and_are_retried() {
    let r = rig(BOARD_HTML).await;
    {
        let mut p = r.site.state.profiles.lock().unwrap();
        p.insert("decu".into(), (500, "boom".into()));
        p.insert("cented".into(), (200, "<html>page moved</html>".into()));
    }
    let run = r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, KeRunStatus::Ok);
    assert_eq!((run.rows, run.profiles_failed), (100, 2));
    for slug in ["decu", "cented"] {
        let m = r.snapshots.row(slug).unwrap().metrics;
        assert_eq!((m.avg_pos, m.hold_s), (None, None), "{slug}");
        assert!(m.d30.is_some(), "the rest of the row is written");
    }
    assert_eq!(
        r.snapshots.row("trunoest").unwrap().metrics.avg_pos,
        Some(342.0)
    );

    // Failures are not cached: the next run asks for those two again, and only those.
    r.site.state.profiles.lock().unwrap().clear();
    let before = r.site.profile_hits();
    let run = r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.profiles_failed, 0);
    assert_eq!(r.site.profile_hits() - before, 2);
    assert_eq!(
        r.snapshots.row("decu").unwrap().metrics.avg_pos,
        Some(342.0)
    );
    r.db.close().await;
}

#[tokio::test]
async fn closed_snapshot_is_refused_before_anything_happens() {
    let r = rig(BOARD_HTML).await;
    r.snapshots.close_day();
    let err = r.ke.run(&ADMIN, day()).await.unwrap_err();
    assert!(matches!(err, KeError::Internal { .. }), "{err:?}");
    assert_eq!(r.site.board_hits(), 0);
    assert!(r.registry.kols.lock().unwrap().is_empty());
    r.db.close().await;
}

#[tokio::test]
async fn wallets_in_the_registry() {
    let r = rig(&board_with_rows(60)).await;
    // Prototype wallet differs from KE: KE replaces it. Network wallet differs: left alone.
    r.registry.add(
        "decu",
        "Decu",
        Some("notdecu"),
        Some((
            "OldWallet11111111111111111111111111111111",
            WalletSource::Prototype,
        )),
    );
    r.registry.add(
        "cented",
        "Cented",
        Some("flipski77"),
        Some((
            "NetWallet111111111111111111111111111111111",
            WalletSource::Network,
        )),
    );
    // Row 3 wallet held by someone else: not published, run goes on.
    let third =
        plugin_kolexplorer::parse::parse_leaderboard(&board_with_rows(60)).unwrap()[2].clone();
    r.registry.add(
        "squatter",
        "Squatter",
        None,
        Some((&third.address, WalletSource::Network)),
    );

    let run = r.ke.run(&ADMIN, day()).await.unwrap();
    assert_eq!(run.status, KeRunStatus::Ok);
    assert_eq!(
        r.registry.get("decu").unwrap().main_wallet.unwrap().address,
        "4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud9"
    );
    let cented = r.registry.get("cented").unwrap().main_wallet.unwrap();
    assert_eq!(
        (cented.address.as_str(), cented.source),
        (
            "NetWallet111111111111111111111111111111111",
            WalletSource::Network
        )
    );
    // The KOL of row 3 is the squatter (found by wallet): its wallet is already right.
    assert!(r.snapshots.row("squatter").is_some());
    // Every written KOL has one KE row.
    assert_eq!(r.snapshots.members_with(MemberReason::Top100).len(), 60);
    r.db.close().await;
}
