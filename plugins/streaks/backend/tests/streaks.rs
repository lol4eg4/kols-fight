//! Streaks tests: a badge only with the whole history, fresh rows only, public route.
//! Snapshots is a stand-in that serves fixed data.

use api_snapshots::{
    KolId, Member, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotStatus,
    SnapshotView, SnapshotsApi, SnapshotsError,
};
use arena_platform::{Config, Ctx, Person};
use async_trait::async_trait;
use axum::{Router, body::Body, http::Request};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use plugin_streaks::{ENTRY, Streak, streaks};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use tower::ServiceExt;

/// The last day of the fixtures.
fn last() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 14).unwrap()
}

fn back(n: i64) -> NaiveDate {
    last() - Duration::days(n)
}

fn morning(date: NaiveDate) -> DateTime<Utc> {
    Utc.from_utc_datetime(&date.and_hms_opt(6, 0, 0).unwrap())
}

#[derive(Clone)]
struct Cell {
    traded: Option<bool>,
    d7: Option<f64>,
    /// Row carried forward from the day before.
    carried: bool,
}

fn cell(traded: bool, d7: f64) -> Cell {
    Cell {
        traded: Some(traded),
        d7: Some(d7),
        carried: false,
    }
}

#[derive(Default)]
struct Fake {
    status: BTreeMap<NaiveDate, SnapshotStatus>,
    rows: BTreeMap<(NaiveDate, &'static str), Cell>,
}

impl Fake {
    /// Closed snapshots for the `days` last days (0 = none) with one row per KOL per day from `f`.
    fn with(days: i64, kols: &[&'static str], f: impl Fn(&str, i64) -> Option<Cell>) -> Fake {
        let mut fake = Fake::default();
        for n in 0..days {
            fake.status.insert(back(n), SnapshotStatus::Closed);
            for k in kols {
                if let Some(c) = f(k, n) {
                    fake.rows.insert((back(n), *k), c);
                }
            }
        }
        fake
    }
}

fn unused<T>() -> Result<T, SnapshotsError> {
    Err(SnapshotsError::Internal {
        message: "not used".into(),
    })
}

#[async_trait]
impl SnapshotsApi for Fake {
    async fn open(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        unused()
    }
    async fn close(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        unused()
    }
    async fn put_metrics(
        &self,
        _: &Person,
        _: NaiveDate,
        _: &str,
        _: &Metrics,
        _: MetricSource,
        _: DateTime<Utc>,
    ) -> Result<(), SnapshotsError> {
        unused()
    }
    async fn set_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
        _: &[KolId],
    ) -> Result<(), SnapshotsError> {
        unused()
    }
    async fn copy_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
    ) -> Result<(), SnapshotsError> {
        unused()
    }
    async fn snapshot(&self, _: &Person, date: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        let status = *self
            .status
            .get(&date)
            .ok_or(SnapshotsError::NoSnapshot { date })?;
        let members = self
            .rows
            .iter()
            .filter(|((d, _), _)| *d == date)
            .map(|((_, kol), c)| {
                let fetched = if c.carried {
                    morning(date - Duration::days(1))
                } else {
                    morning(date)
                };
                Member {
                    kol: kol.to_string(),
                    reasons: vec![MemberReason::Top100],
                    row: Some(MetricRow {
                        date,
                        kol: kol.to_string(),
                        source: MetricSource::Kolexplorer,
                        fetched_at: fetched,
                        metrics: Metrics {
                            traded_24h: c.traded,
                            d7: c.d7,
                            ..Metrics::default()
                        },
                    }),
                }
            })
            .collect();
        Ok(SnapshotView {
            snapshot: Snapshot {
                date,
                status,
                opened_at: morning(date),
                closed_at: Some(morning(date)),
            },
            members,
        })
    }
    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Ok(self
            .status
            .iter()
            .filter(|(_, s)| **s == SnapshotStatus::Closed)
            .map(|(d, _)| *d)
            .max())
    }
    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Ok(self
            .status
            .iter()
            .filter(|(_, s)| **s == SnapshotStatus::Closed)
            .map(|(d, _)| *d)
            .collect())
    }
    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        unused()
    }
}

async fn badges_of(fake: &Fake, kol: &str) -> Vec<Streak> {
    let view = streaks(fake, &Person::NOBODY, last()).await.unwrap();
    view.streaks
        .into_iter()
        .find(|s| s.kol == kol)
        .map(|s| s.badges)
        .unwrap_or_default()
}

#[tokio::test]
async fn seven_days_with_a_trade_needs_seven_days_of_history() {
    let traded = |_: &str, _: i64| Some(cell(true, -1.0));
    // 7 closed days, traded each: the badge.
    let fake = Fake::with(7, &["ann"], traded);
    assert_eq!(badges_of(&fake, "ann").await, [Streak::Trade7d]);
    // 6 closed days: not enough history.
    let fake = Fake::with(6, &["ann"], traded);
    assert_eq!(badges_of(&fake, "ann").await, []);
}

#[tokio::test]
async fn one_idle_or_missing_day_breaks_the_trade_streak() {
    let idle = Fake::with(10, &["ann"], |_, n| Some(cell(n != 3, -1.0)));
    assert_eq!(badges_of(&idle, "ann").await, []);

    let unknown = Fake::with(10, &["ann"], |_, n| {
        Some(Cell {
            traded: (n != 3).then_some(true),
            d7: None,
            carried: false,
        })
    });
    assert_eq!(badges_of(&unknown, "ann").await, []);

    // a day without a snapshot in the middle
    let mut gap = Fake::with(10, &["ann"], |_, _| Some(cell(true, -1.0)));
    gap.status.remove(&back(2));
    assert_eq!(badges_of(&gap, "ann").await, []);

    // an old trade streak ended before the window
    let old = Fake::with(10, &["ann"], |_, n| Some(cell(n > 7, -1.0)));
    assert_eq!(badges_of(&old, "ann").await, []);
}

#[tokio::test]
async fn a_carried_forward_row_is_not_a_trade() {
    let fake = Fake::with(10, &["ann", "bob"], |k, n| {
        Some(Cell {
            traded: Some(true),
            d7: Some(-1.0),
            carried: k == "bob" && n == 4,
        })
    });
    assert_eq!(badges_of(&fake, "ann").await, [Streak::Trade7d]);
    assert_eq!(badges_of(&fake, "bob").await, []);
}

#[tokio::test]
async fn three_weeks_in_profit_needs_fifteen_days_and_three_green_weeks() {
    // d7 above zero on the last day, 7 and 14 days back.
    let green = |_: &str, n: i64| Some(cell(false, if n % 7 == 0 { 500.0 } else { -50.0 }));
    let fake = Fake::with(15, &["ann"], green);
    assert_eq!(badges_of(&fake, "ann").await, [Streak::Profit3w]);
    // one day short: no row 14 days back.
    let fake = Fake::with(14, &["ann"], green);
    assert_eq!(badges_of(&fake, "ann").await, []);
    // the middle week is red.
    let red = Fake::with(15, &["ann"], |_, n| {
        Some(cell(false, if n == 7 { -10.0 } else { 500.0 }))
    });
    assert_eq!(badges_of(&red, "ann").await, []);
    // exactly zero is not profit.
    let zero = Fake::with(15, &["ann"], |_, n| {
        Some(cell(false, if n == 14 { 0.0 } else { 500.0 }))
    });
    assert_eq!(badges_of(&zero, "ann").await, []);
}

#[tokio::test]
async fn both_badges_and_only_kols_with_a_badge_are_listed() {
    let fake = Fake::with(15, &["ann", "bob", "cy"], |k, n| match k {
        "ann" => Some(cell(true, 800.0)),
        "bob" => Some(cell(true, if n == 0 { -1.0 } else { 800.0 })),
        _ => Some(cell(false, -5.0)),
    });
    let view = streaks(&fake, &Person::NOBODY, last()).await.unwrap();
    let names: Vec<_> = view.streaks.iter().map(|s| s.kol.as_str()).collect();
    assert_eq!(names, ["ann", "bob"]);
    assert_eq!(view.streaks[0].badges, [Streak::Trade7d, Streak::Profit3w]);
    assert_eq!(view.streaks[1].badges, [Streak::Trade7d]);
}

#[tokio::test]
async fn an_open_or_missing_date_has_no_answer() {
    let mut fake = Fake::with(8, &["ann"], |_, _| Some(cell(true, 1.0)));
    fake.status.insert(last(), SnapshotStatus::Open);
    let err = streaks(&fake, &Person::NOBODY, last()).await.unwrap_err();
    assert_eq!(err, plugin_streaks::StreaksError::NoData(last()));
    let err = streaks(&fake, &Person::NOBODY, back(30)).await.unwrap_err();
    assert_eq!(err, plugin_streaks::StreaksError::NoData(back(30)));
}

// ---- the public route ----

fn router(fake: Fake) -> Router {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://x@127.0.0.1:1/x")
        .unwrap();
    let ctx = Ctx::new(pool, Config::default(), "test").unwrap();
    let fake = Arc::new(fake);
    ctx.apis
        .provide::<dyn SnapshotsApi>("snapshots", vec![("default", fake.clone())], fake)
        .unwrap();
    let plugin = (ENTRY.build)(&ctx).unwrap();
    plugin
        .routes(&ctx)
        .into_iter()
        .fold(Router::default(), |r, route| {
            r.route(&route.path, route.handler)
        })
}

async fn get(router: &Router, uri: &str) -> (u16, Value) {
    let res = router
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

#[tokio::test]
async fn route_returns_only_public_keys() {
    let r = router(Fake::with(15, &["ann"], |_, _| Some(cell(true, 800.0))));
    for uri in ["/api/streaks?date=2026-11-14", "/api/streaks"] {
        let (status, body) = get(&r, uri).await;
        assert_eq!(status, 200);
        assert_eq!(keys(&body), ["date", "streaks"]);
        assert_eq!(keys(&body["streaks"][0]), ["badges", "kol"]);
        assert_eq!(
            body["streaks"][0]["badges"],
            serde_json::json!(["trade_7d", "profit_3w"])
        );
        assert_eq!(body["date"], "2026-11-14");
    }
}

#[tokio::test]
async fn route_errors_have_a_code() {
    let r = router(Fake::with(15, &["ann"], |_, _| Some(cell(true, 800.0))));
    let (status, body) = get(&r, "/api/streaks?date=soon").await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (400, Some("bad_date"))
    );
    let (status, body) = get(&r, "/api/streaks?date=2026-01-01").await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (404, Some("no_snapshot"))
    );
    let (status, body) = get(&router(Fake::default()), "/api/streaks").await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (404, Some("no_snapshot"))
    );
}
