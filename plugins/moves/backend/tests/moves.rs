//! Moves tests: a change only when both days are fresh for the KOL, week climb, public route.
//! Score, snapshots and the registry are stand-ins that serve fixed data.

use api_registry::{
    KolId, KolLinks, KolProfile, KolUpsert, Reach, RegistryApi, RegistryError, WalletSource,
};
use api_score::{RankedKol, Ranking, ScoreApi, ScoreCard, ScoreError, Tier};
use api_snapshots::{
    Member, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotStatus, SnapshotView,
    SnapshotsApi, SnapshotsError,
};
use arena_platform::{Config, Ctx, Person};
use async_trait::async_trait;
use axum::{Router, body::Body, http::Request};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use plugin_moves::{Day, ENTRY, changes, is_fresh, moves};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
}

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()
}

/// One KOL on one day: rank and the time its row was fetched.
type Entry = (&'static str, u32, DateTime<Utc>);

#[derive(Default)]
struct World {
    days: HashMap<NaiveDate, Vec<Entry>>,
}

fn card() -> ScoreCard {
    ScoreCard {
        score: 50,
        tier: Tier::Diamond,
        parts: vec![],
        badges: vec![],
    }
}

#[async_trait]
impl ScoreApi for World {
    async fn score(&self, _: &Person, _: &Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::Internal {
            message: "not used".into(),
        })
    }
    async fn rank(&self, _: &Person, _: NaiveDate) -> Result<(), ScoreError> {
        Err(ScoreError::Internal {
            message: "not used".into(),
        })
    }
    async fn ranking(&self, _: &Person, date: NaiveDate) -> Result<Ranking, ScoreError> {
        let rows = self
            .days
            .get(&date)
            .ok_or(ScoreError::NoSnapshot { date })?;
        let mut rows: Vec<RankedKol> = rows
            .iter()
            .map(|(k, r, _)| RankedKol {
                kol: k.to_string(),
                rank: *r,
                card: card(),
            })
            .collect();
        rows.sort_by_key(|r| r.rank);
        Ok(Ranking {
            date,
            ranked_at: at(date.format("%d").to_string().parse().unwrap(), 7),
            rows,
        })
    }
}

#[async_trait]
impl SnapshotsApi for World {
    async fn open(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(SnapshotsError::Internal {
            message: "not used".into(),
        })
    }
    async fn close(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(SnapshotsError::Internal {
            message: "not used".into(),
        })
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
        Err(SnapshotsError::Internal {
            message: "not used".into(),
        })
    }
    async fn set_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
        _: &[KolId],
    ) -> Result<(), SnapshotsError> {
        Err(SnapshotsError::Internal {
            message: "not used".into(),
        })
    }
    async fn copy_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
    ) -> Result<(), SnapshotsError> {
        Err(SnapshotsError::Internal {
            message: "not used".into(),
        })
    }
    async fn snapshot(&self, _: &Person, date: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        let rows = self
            .days
            .get(&date)
            .ok_or(SnapshotsError::NoSnapshot { date })?;
        Ok(SnapshotView {
            snapshot: Snapshot {
                date,
                status: SnapshotStatus::Closed,
                opened_at: at(1, 6),
                closed_at: Some(at(1, 7)),
            },
            members: rows
                .iter()
                .map(|(k, _, fetched)| Member {
                    kol: k.to_string(),
                    reasons: vec![MemberReason::Top100],
                    row: Some(MetricRow {
                        date,
                        kol: k.to_string(),
                        source: MetricSource::Kolexplorer,
                        fetched_at: *fetched,
                        metrics: Metrics::default(),
                    }),
                })
                .collect(),
        })
    }
    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Ok(self.days.keys().max().copied())
    }
    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        let mut v: Vec<_> = self.days.keys().copied().collect();
        v.sort();
        Ok(v)
    }
    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        Err(SnapshotsError::Internal {
            message: "not used".into(),
        })
    }
}

struct Names;

#[async_trait]
impl RegistryApi for Names {
    async fn upsert_kol(&self, _: &Person, _: &KolUpsert) -> Result<KolId, RegistryError> {
        Err(RegistryError::Internal {
            message: "not used".into(),
        })
    }
    async fn get_kol(&self, _: &Person, _: &str) -> Result<Option<KolProfile>, RegistryError> {
        Err(RegistryError::Internal {
            message: "not used".into(),
        })
    }
    async fn list_kols(&self, _: &Person, ids: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        Ok(ids
            .iter()
            .filter(|i| *i != "ghost")
            .map(|i| KolProfile {
                id: i.clone(),
                name: i.to_uppercase(),
                x_handle: Some("secret_handle".into()),
                links: KolLinks::default(),
                main_wallet: None,
                other_wallets: vec![],
                reach: None,
                formats: vec![],
            })
            .collect())
    }
    async fn find_by_handle(&self, _: &Person, _: &str) -> Result<Option<KolId>, RegistryError> {
        Err(RegistryError::Internal {
            message: "not used".into(),
        })
    }
    async fn find_by_wallet(&self, _: &Person, _: &str) -> Result<Option<KolId>, RegistryError> {
        Err(RegistryError::Internal {
            message: "not used".into(),
        })
    }
    async fn publish_wallets(
        &self,
        _: &Person,
        _: &str,
        _: Option<&str>,
        _: &[String],
        _: WalletSource,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::Internal {
            message: "not used".into(),
        })
    }
    async fn set_reach(
        &self,
        _: &Person,
        _: &str,
        _: &Reach,
        _: &str,
        _: NaiveDate,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::Internal {
            message: "not used".into(),
        })
    }
    async fn set_formats(&self, _: &Person, _: &str, _: &[String]) -> Result<(), RegistryError> {
        Err(RegistryError::Internal {
            message: "not used".into(),
        })
    }
}

/// Day 20 is today; 19 is the day before; 13 is 7 days back.
/// - ann:  rank 3 now, 5 yesterday, 9 last week, fresh every day.
/// - bob:  rank 1 now, 2 yesterday, 2 last week, fresh every day.
/// - cy:   rank 2 now, 1 yesterday; today's row is carried forward (fetched on the 19th).
/// - dee:  rank 4 now, 4 yesterday, 8 last week; last week's row is carried forward.
/// - eve:  rank 5 now; not on the board yesterday or last week.
fn world() -> World {
    let mut w = World::default();
    w.days.insert(
        d(20),
        vec![
            ("bob", 1, at(20, 6)),
            ("cy", 2, at(19, 6)),
            ("ann", 3, at(20, 6)),
            ("dee", 4, at(20, 6)),
            ("eve", 5, at(20, 6)),
        ],
    );
    w.days.insert(
        d(19),
        vec![
            ("cy", 1, at(19, 6)),
            ("bob", 2, at(19, 6)),
            ("dee", 4, at(19, 6)),
            ("ann", 5, at(19, 6)),
        ],
    );
    w.days.insert(
        d(13),
        vec![
            ("bob", 2, at(13, 6)),
            ("dee", 8, at(12, 6)),
            ("ann", 9, at(13, 6)),
        ],
    );
    w
}

async fn run(w: &World, date: NaiveDate) -> plugin_moves::MovesView {
    moves(w, w, &Names, &Person::NOBODY, date).await.unwrap()
}

fn change<'a>(v: &'a plugin_moves::MovesView, kol: &str) -> Option<&'a plugin_moves::Change> {
    v.changes.iter().find(|c| c.kol == kol)
}

#[tokio::test]
async fn change_only_when_both_days_are_fresh() {
    let v = run(&world(), d(20)).await;
    // ann: fresh every day.
    assert_eq!(
        (
            change(&v, "ann").unwrap().day,
            change(&v, "ann").unwrap().week
        ),
        (Some(2), Some(6))
    );
    // bob: climbed one place on the day, none on the week.
    assert_eq!(
        (
            change(&v, "bob").unwrap().day,
            change(&v, "bob").unwrap().week
        ),
        (Some(1), Some(1))
    );
    // cy: today's row is old, so no arrow at all even though the rank fell.
    assert!(change(&v, "cy").is_none());
    // dee: fresh yesterday (same place), stale last week.
    assert_eq!(
        (
            change(&v, "dee").unwrap().day,
            change(&v, "dee").unwrap().week
        ),
        (Some(0), None)
    );
    // eve: not on the other days.
    assert!(change(&v, "eve").is_none());
    // by current rank
    let order: Vec<_> = v.changes.iter().map(|c| c.kol.as_str()).collect();
    assert_eq!(order, ["bob", "ann", "dee"]);
}

#[tokio::test]
async fn week_climb_is_the_best_seven_day_climb() {
    let v = run(&world(), d(20)).await;
    let week = v.week_climb.unwrap();
    assert_eq!(
        (week.kol.as_str(), week.places, week.name.as_deref()),
        ("ann", 6, Some("ANN"))
    );
    let day = v.day_climb.unwrap();
    assert_eq!((day.kol.as_str(), day.places), ("ann", 2));
}

#[tokio::test]
async fn no_week_back_snapshot_means_no_week_change() {
    let mut w = world();
    w.days.remove(&d(13));
    let v = run(&w, d(20)).await;
    assert!(v.changes.iter().all(|c| c.week.is_none()));
    assert!(v.week_climb.is_none());
    assert_eq!(change(&v, "ann").unwrap().day, Some(2));
    assert!(v.day_climb.is_some());
}

#[tokio::test]
async fn first_day_has_no_changes() {
    let mut w = World::default();
    w.days
        .insert(d(18), vec![("ann", 1, at(18, 6)), ("bob", 2, at(18, 6))]);
    let v = run(&w, d(18)).await;
    assert!(v.changes.is_empty());
    assert!(v.day_climb.is_none() && v.week_climb.is_none());
}

#[tokio::test]
async fn nobody_climbing_gives_no_climb() {
    let mut w = World::default();
    w.days
        .insert(d(20), vec![("ann", 2, at(20, 6)), ("bob", 1, at(20, 6))]);
    w.days
        .insert(d(19), vec![("ann", 1, at(19, 6)), ("bob", 2, at(19, 6))]);
    let v = run(&w, d(20)).await;
    assert_eq!(change(&v, "ann").unwrap().day, Some(-1));
    assert_eq!(change(&v, "bob").unwrap().day, Some(1));
    assert_eq!(v.day_climb.unwrap().kol, "bob");
    assert!(v.week_climb.is_none());
}

#[tokio::test]
async fn a_climber_unknown_to_the_registry_keeps_no_name() {
    let mut w = World::default();
    w.days.insert(d(20), vec![("ghost", 1, at(20, 6))]);
    w.days.insert(d(19), vec![("ghost", 4, at(19, 6))]);
    let v = run(&w, d(20)).await;
    let c = v.day_climb.unwrap();
    assert_eq!((c.kol.as_str(), c.places, c.name), ("ghost", 3, None));
}

#[tokio::test]
async fn missing_date_is_no_data() {
    let err = moves(&world(), &world(), &Names, &Person::NOBODY, d(25))
        .await
        .unwrap_err();
    assert_eq!(err, plugin_moves::MovesError::NoData(d(25)));
}

#[test]
fn pure_changes_ignore_a_kol_stale_on_one_day() {
    let day = |ranks: &[(&str, u32)], fresh: &[&str]| Day {
        ranks: ranks.iter().map(|(k, r)| (k.to_string(), *r)).collect(),
        fresh: fresh.iter().map(|k| k.to_string()).collect(),
    };
    let now = day(&[("a", 1), ("b", 2)], &["a", "b"]);
    let then = day(&[("a", 3), ("b", 1)], &["a"]);
    let out = changes(&now, Some(&then), None);
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].kol.as_str(), out[0].day), ("a", Some(2)));
}

#[test]
fn fresh_means_fetched_on_the_day() {
    assert!(is_fresh(d(20), at(20, 0)));
    assert!(is_fresh(d(20), at(21, 3)));
    assert!(!is_fresh(d(20), at(19, 23)));
}

// ---- the public route ----

/// A context whose Score, Snapshots and Registry are the stand-ins.
fn router() -> Router {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://x@127.0.0.1:1/x")
        .unwrap();
    let ctx = Ctx::new(pool, Config::default(), "test").unwrap();
    let w = Arc::new(world());
    ctx.apis
        .provide::<dyn ScoreApi>("score", vec![("default", w.clone())], w.clone())
        .unwrap();
    ctx.apis
        .provide::<dyn SnapshotsApi>("snapshots", vec![("default", w.clone())], w.clone())
        .unwrap();
    ctx.apis
        .provide::<dyn RegistryApi>(
            "registry",
            vec![("default", Arc::new(Names))],
            Arc::new(Names),
        )
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
    let r = router();
    let (status, body) = get(&r, "/api/moves?date=2026-10-20").await;
    assert_eq!(status, 200);
    assert_eq!(keys(&body), ["changes", "date", "day_climb", "week_climb"]);
    assert_eq!(keys(&body["changes"][0]), ["day", "kol", "week"]);
    assert_eq!(keys(&body["week_climb"]), ["kol", "name", "places"]);
    assert_eq!(body["week_climb"]["kol"], "ann");
    assert_eq!(body["date"], "2026-10-20");
    // registry fields beyond the name never reach the response
    assert!(!body.to_string().contains("secret_handle"));
}

#[tokio::test]
async fn route_without_date_uses_the_latest_snapshot() {
    let (status, body) = get(&router(), "/api/moves").await;
    assert_eq!(status, 200);
    assert_eq!(body["date"], "2026-10-20");
}

#[tokio::test]
async fn route_errors_have_a_code() {
    let r = router();
    let (status, body) = get(&r, "/api/moves?date=yesterday").await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (400, Some("bad_date"))
    );
    let (status, body) = get(&r, "/api/moves?date=2026-10-30").await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (404, Some("no_snapshot"))
    );
}
