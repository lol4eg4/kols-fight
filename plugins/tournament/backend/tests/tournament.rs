//! Tournament tests: the bracket of 8, a draw advances the higher rank, bad input is refused,
//! the public route returns only public keys. Fight, score and roster are stand-ins.

use api_fight::{Decision, FightApi, FightError, FightResult, Fighter, KolId};
use api_roster::{NetworkMember, Rates, RosterApi, RosterError, XHandle};
use api_score::{RankedKol, Ranking, ScoreApi, ScoreCard, ScoreError, Tier};
use arena_platform::{Config, Ctx, Person, testing::TestDb};
use async_trait::async_trait;
use axum::{Router, body::Body, http::Request};
use chrono::{NaiveDate, TimeZone, Utc};
use plugin_tournament::{
    ENTRY, MIGRATOR, MatchDecision, NAME, RoundName, TournamentError, lineups, parse_date,
    parse_ids, run_bracket,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()
}

fn names() -> Vec<KolId> {
    (1..=8).map(|i| format!("kol{i}")).collect()
}

fn csv(ids: &[KolId]) -> String {
    ids.join(",")
}

fn card(score: u8) -> ScoreCard {
    ScoreCard {
        score,
        tier: Tier::Goat,
        parts: vec![],
        badges: vec![],
    }
}

/// Fights by strength: the stronger wins, equal strength is a draw. `rank` is the place on the board.
struct FakeFight {
    /// kol -> (strength, rank)
    table: HashMap<KolId, (u32, u32)>,
    asked: Mutex<Vec<(Vec<KolId>, Option<NaiveDate>)>>,
    error: Option<FightError>,
}

impl FakeFight {
    fn new(table: &[(&str, u32, u32)]) -> Arc<Self> {
        Arc::new(FakeFight {
            table: table
                .iter()
                .map(|(k, s, r)| (k.to_string(), (*s, *r)))
                .collect(),
            asked: Mutex::default(),
            error: None,
        })
    }

    /// kol1..kol8 with rank = place, strength = 9 - place: kol1 beats everyone.
    fn by_rank() -> Arc<Self> {
        Self::new(
            &(1..=8)
                .map(|i| (format!("kol{i}"), 9 - i, i))
                .map(|(k, s, r)| (Box::leak(k.into_boxed_str()) as &str, s, r))
                .collect::<Vec<_>>(),
        )
    }
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
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        let missing: Vec<KolId> = ids
            .iter()
            .filter(|k| !self.table.contains_key(*k))
            .cloned()
            .collect();
        if ids.len() - missing.len() < 2 {
            return Err(FightError::TooFewFighters { missing });
        }
        let fighters: Vec<Fighter> = ids
            .iter()
            .map(|k| {
                let (s, r) = self.table[k];
                Fighter {
                    kol: k.clone(),
                    name: k.to_uppercase(),
                    rank: Some(r),
                    card: card(50),
                    points: s as f64,
                    hp: 8,
                    rounds_won: vec![],
                }
            })
            .collect();
        let (a, b) = (&fighters[0], &fighters[1]);
        let (winner, decision) = if a.points == b.points {
            (None, Decision::Draw)
        } else if a.points > b.points {
            (Some(a.kol.clone()), Decision::Rounds)
        } else {
            (Some(b.kol.clone()), Decision::Rounds)
        };
        Ok(FightResult {
            // a fixed date for `None`: the latest snapshot
            date: d.unwrap_or_else(date),
            snapshot_closed_at: None,
            fighters,
            rounds: vec![],
            winner,
            decision,
            counted_rounds: 8,
            missing: vec![],
            counted_line: String::new(),
            summary: vec![],
        })
    }
}

struct FakeScore {
    rows: Vec<(KolId, u32)>,
}

#[async_trait]
impl ScoreApi for FakeScore {
    async fn score(&self, _: &Person, _: &api_score::Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
    async fn rank(&self, _: &Person, _: NaiveDate) -> Result<(), ScoreError> {
        Err(ScoreError::NotBuilt)
    }
    async fn ranking(&self, _: &Person, d: NaiveDate) -> Result<Ranking, ScoreError> {
        Ok(Ranking {
            date: d,
            ranked_at: Utc.with_ymd_and_hms(2026, 10, 18, 7, 0, 0).unwrap(),
            rows: self
                .rows
                .iter()
                .map(|(k, r)| RankedKol {
                    kol: k.clone(),
                    rank: *r,
                    card: card(50),
                })
                .collect(),
        })
    }
    async fn ranked_dates(&self, _: &Person) -> Result<Vec<NaiveDate>, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
}

struct FakeRoster {
    members: Vec<KolId>,
}

#[async_trait]
impl RosterApi for FakeRoster {
    async fn members(&self, _: &Person) -> Result<Vec<NetworkMember>, RosterError> {
        Ok(self
            .members
            .iter()
            .map(|k| NetworkMember {
                kol: k.clone(),
                name: k.clone(),
                x_handle: None,
                x_verified: false,
                fomo_handle: None,
                pumpfun: None,
                sheet_wallets: vec![],
                formats: vec![],
                keep: false,
            })
            .collect())
    }
    async fn is_member(&self, _: &Person, k: &str) -> Result<bool, RosterError> {
        Ok(self.members.iter().any(|m| m == k))
    }
    async fn verified_handle(&self, _: &Person, _: &str) -> Result<Option<XHandle>, RosterError> {
        Ok(None)
    }
    async fn rates(&self, _: &Person, _: &[KolId]) -> Result<Vec<Rates>, RosterError> {
        Err(RosterError::Forbidden)
    }
}

fn all_kols(n: u32) -> Vec<(KolId, u32)> {
    (1..=n).map(|i| (format!("k{i:02}"), i)).collect()
}

// ---- the bracket ----

#[tokio::test]
async fn bracket_of_eight_has_quarters_semis_and_final() {
    let fight = FakeFight::by_rank();
    // shuffled order: pairs are (kol3,kol1) (kol5,kol2) (kol8,kol4) (kol7,kol6)
    let ids: Vec<KolId> = [
        "kol3", "kol1", "kol5", "kol2", "kol8", "kol4", "kol7", "kol6",
    ]
    .map(String::from)
    .to_vec();
    let v = run_bracket(&*fight, &Person::NOBODY, &ids, Some(date()))
        .await
        .unwrap();
    let shape: Vec<_> = v.rounds.iter().map(|r| (r.name, r.matches.len())).collect();
    assert_eq!(
        shape,
        [
            (RoundName::Quarterfinals, 4),
            (RoundName::Semifinals, 2),
            (RoundName::Final, 1)
        ]
    );
    let q = &v.rounds[0].matches;
    let winners: Vec<_> = q.iter().map(|m| m.winner.as_str()).collect();
    assert_eq!(winners, ["kol1", "kol2", "kol4", "kol6"]);
    assert_eq!((q[0].a.kol.as_str(), q[0].b.kol.as_str()), ("kol3", "kol1"));
    assert_eq!(q[0].decision, MatchDecision::Rounds);
    assert_eq!((q[0].points_a, q[0].points_b), (6.0, 8.0));
    let semis: Vec<_> = v.rounds[1]
        .matches
        .iter()
        .map(|m| m.winner.as_str())
        .collect();
    assert_eq!(semis, ["kol1", "kol4"]);
    assert_eq!(v.rounds[2].matches[0].winner, "kol1");
    assert_eq!(v.champion.kol, "kol1");
    assert_eq!(v.champion.name, "KOL1");
    assert_eq!(v.date, date());
    // seven fights, all on the requested date
    let asked = fight.asked.lock().unwrap();
    assert_eq!(asked.len(), 7);
    assert!(
        asked
            .iter()
            .all(|(ids, d)| ids.len() == 2 && *d == Some(date()))
    );
}

#[tokio::test]
async fn without_a_date_every_fight_uses_the_first_fights_date() {
    let fight = FakeFight::by_rank();
    let v = run_bracket(&*fight, &Person::NOBODY, &names(), None)
        .await
        .unwrap();
    assert_eq!(v.date, date());
    let asked = fight.asked.lock().unwrap();
    assert_eq!(asked[0].1, None);
    assert!(asked[1..].iter().all(|(_, d)| *d == Some(date())));
}

#[tokio::test]
async fn a_draw_advances_the_higher_rank() {
    // kol1..kol8 all equally strong: every fight is a draw; ranks: kol2 is best, kol1 worst
    let table: Vec<(&str, u32, u32)> = vec![
        ("kol1", 5, 8),
        ("kol2", 5, 1),
        ("kol3", 5, 3),
        ("kol4", 5, 4),
        ("kol5", 5, 5),
        ("kol6", 5, 2),
        ("kol7", 5, 7),
        ("kol8", 5, 6),
    ];
    let fight = FakeFight::new(&table);
    let v = run_bracket(&*fight, &Person::NOBODY, &names(), Some(date()))
        .await
        .unwrap();
    let w = |r: usize| -> Vec<&str> {
        v.rounds[r]
            .matches
            .iter()
            .map(|m| m.winner.as_str())
            .collect()
    };
    assert_eq!(w(0), ["kol2", "kol3", "kol6", "kol8"]);
    assert_eq!(w(1), ["kol2", "kol6"]);
    assert_eq!(w(2), ["kol2"]);
    assert!(
        v.rounds
            .iter()
            .flat_map(|r| &r.matches)
            .all(|m| m.decision == MatchDecision::RankTiebreak)
    );
    assert_eq!(v.champion.kol, "kol2");
    assert_eq!(v.champion.rank, Some(1));
}

#[tokio::test]
async fn a_draw_between_equal_ranks_advances_the_first_listed() {
    let table: Vec<(&str, u32, u32)> = names().iter().map(|k| (leak(k), 5, 1)).collect();
    let fight = FakeFight::new(&table);
    let v = run_bracket(&*fight, &Person::NOBODY, &names(), Some(date()))
        .await
        .unwrap();
    assert_eq!(v.champion.kol, "kol1");
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

// ---- bad input ----

#[test]
fn bad_ids_are_refused() {
    let ok = csv(&names());
    assert_eq!(parse_ids(&ok).unwrap(), names());
    let bad = |s: &str| matches!(parse_ids(s), Err(TournamentError::BadIds(_)));
    assert!(bad(""));
    assert!(bad("kol1,kol2,kol3,kol4,kol5,kol6,kol7"), "seven");
    assert!(bad("kol1,kol2,kol3,kol4,kol5,kol6,kol7,kol8,kol9"), "nine");
    assert!(bad("kol1,kol1,kol3,kol4,kol5,kol6,kol7,kol8"), "twice");
    assert!(bad("kol1,,kol3,kol4,kol5,kol6,kol7,kol8"), "empty id");
    assert!(bad("kol1,kol2,kol3,kol4,kol5,kol6,kol7,ko l8"), "space");
    assert!(bad("kol1,kol2,kol3,kol4,kol5,kol6,kol7,a/b"), "slash");
    assert!(bad("kol1,kol2,kol3,kol4,kol5,kol6,kol7,'; drop"), "quote");
    let long = "x".repeat(65);
    assert!(bad(&format!("kol1,kol2,kol3,kol4,kol5,kol6,kol7,{long}")));
}

#[test]
fn bad_dates_are_refused() {
    assert_eq!(parse_date("2026-10-18").unwrap(), date());
    for d in [
        "",
        "18-10-2026",
        "2026-13-01",
        "2026-02-30",
        "2026-10-18x",
        "today",
    ] {
        assert_eq!(parse_date(d), Err(TournamentError::BadDate), "{d}");
    }
}

#[tokio::test]
async fn unknown_kol_no_snapshot_and_off_are_typed_errors() {
    let fight = FakeFight::by_rank();
    let mut ids = names();
    ids[3] = "nobody".into();
    let err = run_bracket(&*fight, &Person::NOBODY, &ids, Some(date()))
        .await
        .unwrap_err();
    assert_eq!(err, TournamentError::UnknownKols(vec!["nobody".into()]));

    for (e, want) in [
        (
            FightError::NoSnapshot { date: Some(date()) },
            TournamentError::NoSnapshot,
        ),
        (FightError::Off, TournamentError::Unavailable),
        (FightError::NotBuilt, TournamentError::Unavailable),
    ] {
        let f = Arc::new(FakeFight {
            table: HashMap::new(),
            asked: Mutex::default(),
            error: Some(e),
        });
        let err = run_bracket(&*f, &Person::NOBODY, &names(), Some(date()))
            .await
            .unwrap_err();
        assert_eq!(err, want);
    }
}

// ---- lineups ----

#[tokio::test]
async fn lineups_are_seeded_and_the_network_needs_eight() {
    let score = FakeScore {
        rows: all_kols(20).into_iter().rev().collect(), // not sorted on purpose
    };
    let net: Vec<KolId> = [
        "k02", "k04", "k05", "k09", "k10", "k11", "k15", "k19", "k20",
    ]
    .map(String::from)
    .to_vec();
    let l = lineups(
        &score,
        &FakeRoster { members: net },
        &Person::NOBODY,
        date(),
    )
    .await
    .unwrap();
    // seeds 1 v 8, 4 v 5, 2 v 7, 3 v 6
    assert_eq!(
        l.top,
        ["k01", "k08", "k04", "k05", "k02", "k07", "k03", "k06"]
    );
    // network by rank: k02 k04 k05 k09 k10 k11 k15 k19 (k20 is ninth)
    assert_eq!(
        l.network,
        ["k02", "k19", "k09", "k10", "k04", "k15", "k05", "k11"]
    );

    let few = FakeRoster {
        members: vec!["k01".into(), "k02".into()],
    };
    let l = lineups(&score, &few, &Person::NOBODY, date())
        .await
        .unwrap();
    assert!(l.network.is_empty());
    assert_eq!(l.top.len(), 8);

    let small = FakeScore { rows: all_kols(5) };
    let l = lineups(&small, &few, &Person::NOBODY, date())
        .await
        .unwrap();
    assert!(l.top.is_empty());
}

// ---- the public routes ----

async fn router(db: &TestDb, fight: Arc<FakeFight>) -> Router {
    let ctx = Ctx::new(db.pool().clone(), Config::default(), "test").unwrap();
    let score = Arc::new(FakeScore { rows: all_kols(12) });
    let roster = Arc::new(FakeRoster {
        members: all_kols(12).into_iter().map(|(k, _)| k).collect(),
    });
    ctx.apis
        .provide::<dyn FightApi>("fight", vec![("default", fight.clone())], fight)
        .unwrap();
    ctx.apis
        .provide::<dyn ScoreApi>("score", vec![("default", score.clone())], score)
        .unwrap();
    ctx.apis
        .provide::<dyn RosterApi>("roster", vec![("default", roster.clone())], roster)
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
async fn route_returns_only_public_keys_and_refuses_bad_input() {
    let db = TestDb::with_plugins(&[(NAME, &MIGRATOR)]).await;
    let r = router(&db, FakeFight::by_rank()).await;

    let (status, body) = get(
        &r,
        &format!("/api/tournament?ids={}&date=2026-10-18", csv(&names())),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(keys(&body), ["champion", "date", "rounds"]);
    assert_eq!(
        keys(&body["champion"]),
        ["kol", "name", "rank", "score", "tier"]
    );
    let m = &body["rounds"][0]["matches"][0];
    assert_eq!(
        keys(m),
        [
            "a",
            "b",
            "counted_rounds",
            "decision",
            "points_a",
            "points_b",
            "winner"
        ]
    );
    assert_eq!(keys(&body["rounds"][0]), ["matches", "name"]);
    assert_eq!(body["rounds"][2]["name"], "final");
    assert_eq!(body["champion"]["kol"], "kol1");

    for uri in [
        "/api/tournament",
        "/api/tournament?ids=kol1,kol2",
        "/api/tournament?ids=kol1,kol1,kol3,kol4,kol5,kol6,kol7,kol8&date=2026-10-18",
    ] {
        let (status, body) = get(&r, uri).await;
        assert_eq!(status, 400, "{uri}");
        assert_eq!(body["error"]["code"], "bad_ids");
    }
    let (status, body) = get(
        &r,
        &format!("/api/tournament?ids={}&date=tomorrow", csv(&names())),
    )
    .await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (400, Some("bad_date"))
    );

    let (status, body) = get(
        &r,
        "/api/tournament?ids=kol1,kol2,kol3,kol4,kol5,kol6,kol7,ghost&date=2026-10-18",
    )
    .await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (404, Some("unknown_kol"))
    );

    let (status, body) = get(&r, "/api/tournament/lineups?date=2026-10-18").await;
    assert_eq!(status, 200);
    assert_eq!(keys(&body), ["date", "network", "top"]);
    assert_eq!(body["top"].as_array().unwrap().len(), 8);
    let (status, _) = get(&r, "/api/tournament/lineups").await;
    assert_eq!(status, 400);
    db.close().await;
}
