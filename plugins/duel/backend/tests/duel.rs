//! Duel tests: the pair is deterministic per date, taken from the top 20, stored; public route.
//! Score and fight are stand-ins that serve fixed data. The pair store uses a throwaway `TestDb`.

use api_fight::{Decision, FightApi, FightError, FightResult, Fighter, HP_SEGMENTS, KolId, Round};
use api_score::{RankedKol, Ranking, ScoreApi, ScoreCard, ScoreError, Tier};
use arena_platform::{Config, Ctx, Person, testing::TestDb};
use async_trait::async_trait;
use axum::{Router, body::Body, http::Request};
use chrono::{NaiveDate, TimeZone, Utc};
use plugin_duel::{DuelError, ENTRY, MIGRATOR, NAME, Store, TOP, duel, pick_pair};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

fn date(m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, m, d).unwrap()
}

fn kols(n: usize) -> Vec<KolId> {
    (1..=n).map(|i| format!("kol{i:02}")).collect()
}

fn card(rank: u32) -> ScoreCard {
    ScoreCard {
        score: (100 - rank) as u8,
        tier: Tier::Goat,
        parts: vec![],
        badges: vec![],
    }
}

/// Ranking of `ids` (rank = place in the list) for any date but the missing ones.
struct FakeScore {
    ids: Mutex<Vec<KolId>>,
    missing: Vec<NaiveDate>,
}

impl FakeScore {
    fn new(n: usize) -> Arc<Self> {
        Arc::new(FakeScore {
            ids: Mutex::new(kols(n)),
            missing: vec![],
        })
    }
}

#[async_trait]
impl ScoreApi for FakeScore {
    async fn score(&self, _: &Person, _: &api_score::Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::Internal {
            message: "not used".into(),
        })
    }
    async fn rank(&self, _: &Person, _: NaiveDate) -> Result<(), ScoreError> {
        Err(ScoreError::Internal {
            message: "not used".into(),
        })
    }
    async fn ranking(&self, _: &Person, d: NaiveDate) -> Result<Ranking, ScoreError> {
        if self.missing.contains(&d) {
            return Err(ScoreError::NoSnapshot { date: d });
        }
        let rows = self
            .ids
            .lock()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, k)| RankedKol {
                kol: k.clone(),
                rank: i as u32 + 1,
                card: card(i as u32 + 1),
            })
            .collect();
        Ok(Ranking {
            date: d,
            ranked_at: Utc.with_ymd_and_hms(2026, 10, 18, 7, 0, 0).unwrap(),
            rows,
        })
    }
    async fn ranked_dates(&self, _: &Person) -> Result<Vec<NaiveDate>, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
}

/// Fights nobody; remembers who it was asked about. The first listed wins.
#[derive(Default)]
struct FakeFight {
    asked: Mutex<Vec<(Vec<KolId>, Option<NaiveDate>)>>,
}

#[async_trait]
impl FightApi for FakeFight {
    async fn fight(
        &self,
        _: &Person,
        ids: &[KolId],
        d: Option<NaiveDate>,
    ) -> Result<FightResult, FightError> {
        self.asked.lock().unwrap().push((ids.to_vec(), d));
        let rounds: Vec<Round> = vec![];
        Ok(FightResult {
            date: d.unwrap(),
            snapshot_closed_at: None,
            fighters: ids
                .iter()
                .enumerate()
                .map(|(i, k)| Fighter {
                    kol: k.clone(),
                    name: k.to_uppercase(),
                    rank: Some(i as u32 + 1),
                    card: card(i as u32 + 1),
                    points: if i == 0 { 5.0 } else { 3.0 },
                    hp: if i == 0 { HP_SEGMENTS } else { 3 },
                    rounds_won: vec![],
                })
                .collect(),
            rounds,
            winner: ids.first().cloned(),
            decision: Decision::Rounds,
            counted_rounds: 8,
            missing: vec![],
            counted_line: String::new(),
            summary: vec![],
        })
    }
}

#[test]
fn pair_is_deterministic_per_date() {
    let top = kols(30);
    let a = pick_pair(date(10, 18), &top).unwrap();
    for _ in 0..5 {
        assert_eq!(pick_pair(date(10, 18), &top).unwrap(), a);
    }
    // pinned: a change here changes every stored pair of the future
    assert_eq!(a, ("kol06".to_string(), "kol10".to_string()));
}

#[test]
fn pair_is_two_different_kols_from_the_top_20_better_rank_first() {
    let top = kols(30);
    let mut seen = BTreeSet::new();
    for day in 1..=28 {
        for month in [10, 11, 12] {
            let (a, b) = pick_pair(date(month, day), &top).unwrap();
            assert_ne!(a, b);
            assert!(a < b, "{a} before {b}");
            let place = |k: &str| top.iter().position(|t| t == k).unwrap();
            assert!(place(&a) < TOP && place(&b) < TOP);
            seen.insert((a, b));
        }
    }
    // not one pair forever: the date moves it
    assert!(seen.len() > 20, "{} different pairs in 84 days", seen.len());
}

#[test]
fn pair_with_two_or_fewer_kols() {
    assert_eq!(pick_pair(date(10, 18), &kols(1)), None);
    assert_eq!(pick_pair(date(10, 18), &[]), None);
    assert_eq!(
        pick_pair(date(10, 18), &kols(2)),
        Some(("kol01".into(), "kol02".into()))
    );
}

#[test]
fn only_the_top_20_count() {
    // the same first 20 places give the same pair however long the list is
    assert_eq!(
        pick_pair(date(10, 18), &kols(20)),
        pick_pair(date(10, 18), &kols(120))
    );
}

async fn store() -> (TestDb, Store) {
    let db = TestDb::with_plugins(&[(NAME, &MIGRATOR)]).await;
    let s = Store::new(db.pool().clone());
    (db, s)
}

#[tokio::test]
async fn the_pair_is_stored_and_survives_a_new_ranking() {
    let (db, store) = store().await;
    let score = FakeScore::new(30);
    let fight = FakeFight::default();
    let d = date(10, 18);

    let first = duel(&store, &*score, &fight, &Person::NOBODY, d)
        .await
        .unwrap();
    let ids: Vec<_> = first.fighters.iter().map(|f| f.kol.clone()).collect();
    assert_eq!(ids.len(), 2);
    let expected = pick_pair(d, &kols(30)).unwrap();
    assert_eq!(ids, [expected.0.clone(), expected.1.clone()]);
    assert_eq!(first.winner.as_deref(), Some(ids[0].as_str()));
    assert_eq!(first.counted_rounds, 8);
    assert_eq!(first.fighters[0].name, ids[0].to_uppercase());
    assert_eq!(
        fight.asked.lock().unwrap().last().unwrap(),
        &(ids.clone(), Some(d))
    );

    // the day is ranked again and the order is reversed: the stored pair stays
    score.ids.lock().unwrap().reverse();
    let second = duel(&store, &*score, &fight, &Person::NOBODY, d)
        .await
        .unwrap();
    assert_eq!(second, first);

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM duel.pick")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(rows, 1);

    // another date gets its own row
    duel(&store, &*score, &fight, &Person::NOBODY, date(10, 19))
        .await
        .unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM duel.pick")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(rows, 2);
    db.close().await;
}

#[tokio::test]
async fn a_new_store_gives_the_same_pair_for_a_date() {
    // two servers (or a restart with an empty table) agree because the pair is a function of the date
    let (db1, s1) = store().await;
    let (db2, s2) = store().await;
    let fight = FakeFight::default();
    let a = duel(
        &s1,
        &*FakeScore::new(25),
        &fight,
        &Person::NOBODY,
        date(11, 2),
    )
    .await
    .unwrap();
    let b = duel(
        &s2,
        &*FakeScore::new(25),
        &fight,
        &Person::NOBODY,
        date(11, 2),
    )
    .await
    .unwrap();
    assert_eq!(a, b);
    db1.close().await;
    db2.close().await;
}

#[tokio::test]
async fn no_ranking_or_one_kol_is_no_duel() {
    let (db, store) = store().await;
    let fight = FakeFight::default();
    let d = date(10, 18);

    let score = Arc::new(FakeScore {
        ids: Mutex::new(kols(30)),
        missing: vec![d],
    });
    let err = duel(&store, &*score, &fight, &Person::NOBODY, d)
        .await
        .unwrap_err();
    assert_eq!(err, DuelError::NoDuel(d));

    let err = duel(&store, &*FakeScore::new(1), &fight, &Person::NOBODY, d)
        .await
        .unwrap_err();
    assert_eq!(err, DuelError::NoDuel(d));
    // nothing was stored for either
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM duel.pick")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0);
    db.close().await;
}

// ---- the public route ----

async fn router(db: &TestDb) -> Router {
    let ctx = Ctx::new(db.pool().clone(), Config::default(), "test").unwrap();
    let score = FakeScore::new(30);
    let fight = Arc::new(FakeFight::default());
    ctx.apis
        .provide::<dyn ScoreApi>("score", vec![("default", score.clone())], score)
        .unwrap();
    ctx.apis
        .provide::<dyn FightApi>("fight", vec![("default", fight.clone())], fight)
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
    let db = TestDb::with_plugins(&[(NAME, &MIGRATOR)]).await;
    let r = router(&db).await;
    let (status, body) = get(&r, "/api/duel?date=2026-10-18").await;
    assert_eq!(status, 200);
    assert_eq!(
        keys(&body),
        ["counted_rounds", "date", "fighters", "winner"]
    );
    assert_eq!(
        keys(&body["fighters"][0]),
        ["hp", "kol", "name", "points", "rank", "score", "tier"]
    );
    assert_eq!(body["fighters"].as_array().unwrap().len(), 2);
    assert_eq!(body["date"], "2026-10-18");
    let (again_status, again) = get(&r, "/api/duel?date=2026-10-18").await;
    assert_eq!((again_status, again), (200, body));

    let (status, body) = get(&r, "/api/duel").await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (400, Some("bad_date"))
    );
    let (status, body) = get(&r, "/api/duel?date=18.10").await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (400, Some("bad_date"))
    );
    db.close().await;
}
