use api_snapshots::{
    MemberReason, MetricSource, Metrics, SnapshotStatus, SnapshotView, SnapshotsApi, SnapshotsError,
};
use arena_platform::{Person, testing::TestDb};
use chrono::{DateTime, NaiveDate, Utc};
use plugin_snapshots::{MIGRATOR, NAME, Snapshots};

const P: Person = Person::SYSTEM;

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

async fn setup() -> (TestDb, Snapshots) {
    let db = TestDb::with_plugins(&[(NAME, &MIGRATOR)]).await;
    let s = Snapshots::new(db.pool().clone());
    (db, s)
}

fn metrics(d30: f64) -> Metrics {
    Metrics {
        d30: Some(d30),
        wr30: Some(0.5),
        last_min_at_fetch: Some(30),
        ..Metrics::default()
    }
}

#[tokio::test]
async fn seed_is_the_day0_fixture() {
    let (db, s) = setup().await;
    let want: SnapshotView =
        serde_json::from_str(include_str!("fixtures/day0_2026-10-18.json")).unwrap();
    let got = s.snapshot(&P, day(18)).await.unwrap();
    assert_eq!(got.members.len(), 122);
    assert_eq!(got, want);
    assert_eq!(got.snapshot.status, SnapshotStatus::Closed);
    let t = got.members.iter().find(|m| m.kol == "tekkerrss").unwrap();
    assert_eq!(t.reasons, [MemberReason::Network]);
    let row = t.row.as_ref().unwrap();
    assert_eq!(row.source, MetricSource::Solanatracker);
    // Solana Tracker's all-time win rate is not a 30-day one; missing metrics stay null.
    assert_eq!(row.metrics.wr30, None);
    assert_eq!(row.metrics.wr_all, Some(0.442));
    assert_eq!(row.metrics.d1, None);
    assert_eq!(s.latest_date(&P).await.unwrap(), Some(day(18)));
    assert_eq!(s.dates(&P).await.unwrap(), [day(18)]);

    // The seed is idempotent.
    sqlx::raw_sql(include_str!("../../migrations/0003_seed.sql"))
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(s.snapshot(&P, day(18)).await.unwrap(), want);
    db.close().await;
}

#[tokio::test]
async fn writes_need_an_open_snapshot() {
    let (db, s) = setup().await;
    let m = metrics(1.0);
    let now = Utc::now();
    assert_eq!(
        s.put_metrics(&P, day(18), "decu", &m, MetricSource::Kolexplorer, now)
            .await,
        Err(SnapshotsError::Closed { date: day(18) })
    );
    assert_eq!(
        s.set_members(&P, day(18), MemberReason::Top100, &[]).await,
        Err(SnapshotsError::Closed { date: day(18) })
    );
    assert_eq!(
        s.put_metrics(&P, day(25), "decu", &m, MetricSource::Kolexplorer, now)
            .await,
        Err(SnapshotsError::NoSnapshot { date: day(25) })
    );
    assert!(matches!(
        s.snapshot(&P, day(25)).await,
        Err(SnapshotsError::NoSnapshot { .. })
    ));
    assert!(matches!(
        s.close(&P, day(25)).await,
        Err(SnapshotsError::NoSnapshot { .. })
    ));
    db.close().await;
}

#[tokio::test]
async fn close_carries_forward_the_latest_row() {
    let (db, s) = setup().await;
    let opened = s.open(&P, day(19)).await.unwrap();
    assert_eq!(opened.status, SnapshotStatus::Open);
    assert_eq!(opened.closed_at, None);
    s.copy_members(&P, day(19), MemberReason::Top100)
        .await
        .unwrap();
    s.copy_members(&P, day(19), MemberReason::Network)
        .await
        .unwrap();
    let fresh = at("2026-10-19T06:01:00Z");
    s.put_metrics(
        &P,
        day(19),
        "decu",
        &metrics(600_000.0),
        MetricSource::Kolexplorer,
        fresh,
    )
    .await
    .unwrap();
    // Open and not yet closed: only decu has a row.
    let open = s.snapshot(&P, day(19)).await.unwrap();
    assert_eq!(open.members.len(), 122);
    assert_eq!(open.members.iter().filter(|m| m.row.is_some()).count(), 1);
    // Open snapshots are not listed.
    assert_eq!(s.dates(&P).await.unwrap(), [day(18)]);

    let closed = s.close(&P, day(19)).await.unwrap();
    assert_eq!(closed.status, SnapshotStatus::Closed);
    assert!(closed.closed_at.is_some());
    let view = s.snapshot(&P, day(19)).await.unwrap();
    let day0 = s.snapshot(&P, day(18)).await.unwrap();
    for m in &view.members {
        let row = m.row.as_ref().unwrap();
        assert_eq!(row.date, day(19));
        if m.kol == "decu" {
            assert_eq!(row.fetched_at, fresh);
            assert_eq!(row.metrics.d30, Some(600_000.0));
        } else {
            // Old source, fetched_at and metrics: the date shows their age.
            let old = day0.members.iter().find(|o| o.kol == m.kol).unwrap();
            let old = old.row.as_ref().unwrap();
            assert_eq!(row.source, old.source);
            assert_eq!(row.fetched_at, old.fetched_at);
            assert_eq!(row.metrics, old.metrics);
        }
    }
    assert_eq!(s.latest_date(&P).await.unwrap(), Some(day(19)));
    assert_eq!(s.dates(&P).await.unwrap(), [day(18), day(19)]);

    // A day later, a KOL without a row on day 19 or 20 still gets the newest earlier one.
    s.open(&P, day(20)).await.unwrap();
    s.set_members(&P, day(20), MemberReason::Top100, &["decu".into()])
        .await
        .unwrap();
    s.close(&P, day(20)).await.unwrap();
    let h = s.history(&P, "decu", day(1), day(31)).await.unwrap();
    let dates: Vec<NaiveDate> = h.iter().map(|r| r.date).collect();
    assert_eq!(dates, [day(18), day(19), day(20)]);
    assert_eq!(h[2].fetched_at, fresh);
    assert_eq!(h[2].metrics.d30, Some(600_000.0));
    let h = s.history(&P, "decu", day(19), day(19)).await.unwrap();
    assert_eq!(h.len(), 1);
    db.close().await;
}

#[tokio::test]
async fn reopen_same_date() {
    let (db, s) = setup().await;
    s.open(&P, day(19)).await.unwrap();
    s.set_members(
        &P,
        day(19),
        MemberReason::Top100,
        &["decu".into(), "cented".into()],
    )
    .await
    .unwrap();
    let first_fetch = at("2026-10-19T06:01:00Z");
    s.put_metrics(
        &P,
        day(19),
        "decu",
        &metrics(1.0),
        MetricSource::Kolexplorer,
        first_fetch,
    )
    .await
    .unwrap();
    let first = s.close(&P, day(19)).await.unwrap();

    // Opening a still-open snapshot changes nothing; opening a closed one opens it again.
    let again = s.open(&P, day(19)).await.unwrap();
    assert_eq!(again.status, SnapshotStatus::Open);
    assert_eq!(again.closed_at, first.closed_at);
    assert!(again.opened_at >= first.opened_at);
    let same = s.open(&P, day(19)).await.unwrap();
    assert_eq!(same, again);
    // Closed date leaves the list while it is open again.
    assert_eq!(s.dates(&P).await.unwrap(), [day(18)]);

    // A re-run replaces the source's row and closes again.
    let second_fetch = at("2026-10-19T09:00:00Z");
    s.put_metrics(
        &P,
        day(19),
        "decu",
        &metrics(2.0),
        MetricSource::Kolexplorer,
        second_fetch,
    )
    .await
    .unwrap();
    let second = s.close(&P, day(19)).await.unwrap();
    assert!(second.closed_at >= first.closed_at);
    let view = s.snapshot(&P, day(19)).await.unwrap();
    let decu = view.members.iter().find(|m| m.kol == "decu").unwrap();
    let row = decu.row.as_ref().unwrap();
    assert_eq!((row.fetched_at, row.metrics.d30), (second_fetch, Some(2.0)));
    // Cented was carried forward on the first close and keeps that row.
    let cented = view.members.iter().find(|m| m.kol == "cented").unwrap();
    assert_eq!(
        cented.row.as_ref().unwrap().fetched_at,
        at("2026-10-17T12:00:00Z")
    );
    db.close().await;
}

#[tokio::test]
async fn set_members_replaces_one_reason() {
    let (db, s) = setup().await;
    s.open(&P, day(19)).await.unwrap();
    s.set_members(
        &P,
        day(19),
        MemberReason::Top100,
        &["decu".into(), "tekkerrss".into()],
    )
    .await
    .unwrap();
    s.set_members(
        &P,
        day(19),
        MemberReason::Network,
        &["tekkerrss".into(), "peter".into()],
    )
    .await
    .unwrap();
    let view = s.snapshot(&P, day(19)).await.unwrap();
    let got: Vec<(&str, &[MemberReason])> = view
        .members
        .iter()
        .map(|m| (m.kol.as_str(), m.reasons.as_slice()))
        .collect();
    assert_eq!(
        got,
        [
            ("decu", &[MemberReason::Top100][..]),
            ("peter", &[MemberReason::Network][..]),
            (
                "tekkerrss",
                &[MemberReason::Top100, MemberReason::Network][..]
            ),
        ]
    );

    // A re-run drops KOLs that left the top; the network members stay.
    s.set_members(&P, day(19), MemberReason::Top100, &["cented".into()])
        .await
        .unwrap();
    let view = s.snapshot(&P, day(19)).await.unwrap();
    let got: Vec<(&str, &[MemberReason])> = view
        .members
        .iter()
        .map(|m| (m.kol.as_str(), m.reasons.as_slice()))
        .collect();
    assert_eq!(
        got,
        [
            ("cented", &[MemberReason::Top100][..]),
            ("peter", &[MemberReason::Network][..]),
            ("tekkerrss", &[MemberReason::Network][..]),
        ]
    );
    db.close().await;
}

#[tokio::test]
async fn copy_members_from_the_previous_snapshot() {
    let (db, s) = setup().await;
    s.open(&P, day(19)).await.unwrap();
    s.set_members(&P, day(19), MemberReason::Network, &["someone-new".into()])
        .await
        .unwrap();
    s.copy_members(&P, day(19), MemberReason::Top100)
        .await
        .unwrap();
    let view = s.snapshot(&P, day(19)).await.unwrap();
    let top: Vec<&str> = view
        .members
        .iter()
        .filter(|m| m.reasons.contains(&MemberReason::Top100))
        .map(|m| m.kol.as_str())
        .collect();
    assert_eq!(top.len(), 100);
    assert!(top.contains(&"decu") && !top.contains(&"tekkerrss"));
    // Other reasons stay.
    assert!(view.members.iter().any(|m| m.kol == "someone-new"));

    // The day after copies from day 19, not day 18.
    s.close(&P, day(19)).await.unwrap();
    s.open(&P, day(20)).await.unwrap();
    s.copy_members(&P, day(20), MemberReason::Network)
        .await
        .unwrap();
    let view = s.snapshot(&P, day(20)).await.unwrap();
    let kols: Vec<&str> = view.members.iter().map(|m| m.kol.as_str()).collect();
    assert_eq!(kols, ["someone-new"]);
    db.close().await;
}
