//! Fight rules (plan section 7), the routes, the image and page meta, over stand-in snapshots,
//! score and registry APIs fed from the day-0 fixture (a plugin backend never depends on
//! another plugin's backend).

use api_fight::{
    Decision, FightApi, FightError, FightResult, RoundKind, RoundOutcome, ScoreCard, Tier,
    VoidReason,
};
use api_registry::{
    KolId, KolLinks, KolProfile, KolUpsert, Reach, RegistryApi, RegistryError, WalletSource,
};
use api_score::{RankedKol, Ranking, ScoreApi, ScoreError};
use api_snapshots::{
    MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotView, SnapshotsApi,
    SnapshotsError,
};
use arena_platform::{Config, Ctx, MetaAttr, Person, Plugin};
use async_trait::async_trait;
use axum::{body::Body, http::Request};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use plugin_fight::{FighterInput, resolve};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

const P: Person = Person::NOBODY;
const DAY0_VIEW: &str =
    include_str!("../../../snapshots/backend/tests/fixtures/day0_2026-10-18.json");
const PROTOTYPE: &str = include_str!("../../../../data/prototype/arena_data_v2.json");
const EXPECTED: &str = include_str!("../../../../data/prototype/expected_ranking_2026-10-18.json");

fn day0() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()
}

fn day1() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
}

// ---------- stand-ins ----------

#[derive(Default)]
struct Store {
    views: HashMap<NaiveDate, SnapshotView>,
    rankings: HashMap<NaiveDate, Ranking>,
}

type Shared = Arc<Mutex<Store>>;

struct FakeSnapshots(Shared);

#[async_trait]
impl SnapshotsApi for FakeSnapshots {
    async fn open(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }
    async fn close(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
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
        Err(SnapshotsError::NotBuilt)
    }
    async fn set_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
        _: &[KolId],
    ) -> Result<(), SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }
    async fn copy_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
    ) -> Result<(), SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }
    async fn snapshot(&self, _: &Person, date: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        let store = self.0.lock().unwrap();
        store
            .views
            .get(&date)
            .cloned()
            .ok_or(SnapshotsError::NoSnapshot { date })
    }
    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Ok(self.0.lock().unwrap().views.keys().max().copied())
    }
    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        let mut d: Vec<NaiveDate> = self.0.lock().unwrap().views.keys().copied().collect();
        d.sort();
        Ok(d)
    }
    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }
}

struct FakeScore(Shared);

#[async_trait]
impl ScoreApi for FakeScore {
    async fn score(&self, _: &Person, _: &Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
    async fn rank(&self, _: &Person, _: NaiveDate) -> Result<(), ScoreError> {
        Err(ScoreError::NotBuilt)
    }
    async fn ranking(&self, _: &Person, date: NaiveDate) -> Result<Ranking, ScoreError> {
        let store = self.0.lock().unwrap();
        store
            .rankings
            .get(&date)
            .cloned()
            .ok_or(ScoreError::NotRanked { date })
    }
}

struct FakeRegistry(HashMap<String, KolProfile>);

#[async_trait]
impl RegistryApi for FakeRegistry {
    async fn upsert_kol(&self, _: &Person, _: &KolUpsert) -> Result<KolId, RegistryError> {
        Err(RegistryError::NotBuilt)
    }
    async fn get_kol(&self, _: &Person, id: &str) -> Result<Option<KolProfile>, RegistryError> {
        Ok(self.0.get(id).cloned())
    }
    async fn list_kols(&self, _: &Person, ids: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        Ok(ids
            .iter()
            .filter_map(|id| self.0.get(id).cloned())
            .collect())
    }
    async fn find_by_handle(&self, _: &Person, _: &str) -> Result<Option<KolId>, RegistryError> {
        Err(RegistryError::NotBuilt)
    }
    async fn find_by_wallet(&self, _: &Person, _: &str) -> Result<Option<KolId>, RegistryError> {
        Err(RegistryError::NotBuilt)
    }
    async fn publish_wallets(
        &self,
        _: &Person,
        _: &str,
        _: Option<&str>,
        _: &[String],
        _: WalletSource,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::NotBuilt)
    }
    async fn set_reach(
        &self,
        _: &Person,
        _: &str,
        _: &Reach,
        _: &str,
        _: NaiveDate,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::NotBuilt)
    }
    async fn set_formats(&self, _: &Person, _: &str, _: &[String]) -> Result<(), RegistryError> {
        Err(RegistryError::NotBuilt)
    }
}

// ---------- day-0 data ----------

fn tier_of(s: &str) -> Tier {
    match s {
        "GOAT" => Tier::Goat,
        "Diamond" => Tier::Diamond,
        "Sendoor" => Tier::Sendoor,
        _ => Tier::Degen,
    }
}

fn card(score: u8, tier: Tier) -> ScoreCard {
    ScoreCard {
        score,
        tier,
        parts: Vec::new(),
        badges: Vec::new(),
    }
}

/// Day-0 ranking from the pinned 122-row fixture.
fn day0_ranking() -> Ranking {
    let rows: Vec<serde_json::Value> = serde_json::from_str(EXPECTED).unwrap();
    Ranking {
        date: day0(),
        ranked_at: "2026-10-18T10:41:00Z".parse().unwrap(),
        rows: rows
            .iter()
            .map(|r| RankedKol {
                kol: r["slug"].as_str().unwrap().into(),
                rank: r["rank"].as_u64().unwrap() as u32,
                card: card(
                    r["score"].as_u64().unwrap() as u8,
                    tier_of(r["tier"].as_str().unwrap()),
                ),
            })
            .collect(),
    }
}

fn profiles() -> HashMap<String, KolProfile> {
    let v: serde_json::Value = serde_json::from_str(PROTOTYPE).unwrap();
    v["kols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            let id = k["slug"].as_str().unwrap().to_string();
            (
                id.clone(),
                KolProfile {
                    id,
                    name: k["name"].as_str().unwrap().into(),
                    x_handle: k["x"].as_str().map(String::from),
                    links: KolLinks::default(),
                    main_wallet: None,
                    other_wallets: Vec::new(),
                    reach: None,
                    formats: Vec::new(),
                },
            )
        })
        .collect()
}

/// Day 0 and day 1; on day 1 `tekkerrss` has left the board.
fn store() -> Shared {
    let view0: SnapshotView = serde_json::from_str(DAY0_VIEW).unwrap();
    let mut view1 = view0.clone();
    view1.snapshot.date = day1();
    view1.snapshot.closed_at = Some("2026-10-19T06:30:00Z".parse().unwrap());
    view1.members.retain(|m| m.kol != "tekkerrss");
    let r0 = day0_ranking();
    let mut r1 = r0.clone();
    r1.date = day1();
    r1.rows.retain(|r| r.kol != "tekkerrss");
    let mut s = Store::default();
    s.views.insert(day0(), view0);
    s.views.insert(day1(), view1);
    s.rankings.insert(day0(), r0);
    s.rankings.insert(day1(), r1);
    Arc::new(Mutex::new(s))
}

struct Setup {
    store: Shared,
    plugin: Arc<dyn Plugin>,
    ctx: Ctx,
    api: Arc<dyn FightApi>,
}

fn setup() -> Setup {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://x@127.0.0.1:1/x")
        .unwrap();
    let ctx = Ctx::new(pool, Config::default(), "test").unwrap();
    let store = store();
    ctx.apis
        .provide::<dyn SnapshotsApi>(
            "snapshots",
            vec![("default", Arc::new(FakeSnapshots(store.clone())))],
            Arc::new(api_snapshots::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn ScoreApi>(
            "score",
            vec![("default", Arc::new(FakeScore(store.clone())))],
            Arc::new(api_score::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn RegistryApi>(
            "registry",
            vec![("default", Arc::new(FakeRegistry(profiles())))],
            Arc::new(api_registry::stub::Stub),
        )
        .unwrap();
    let plugin = (plugin_fight::ENTRY.build)(&ctx).unwrap();
    let api = ctx.apis.require::<dyn FightApi>("test").unwrap().pick(&P);
    Setup {
        store,
        plugin,
        ctx,
        api,
    }
}

async fn get(s: &Setup, uri: &str) -> (u16, Vec<u8>, String) {
    let path = uri.split('?').next().unwrap();
    let route = s
        .plugin
        .routes(&s.ctx)
        .into_iter()
        .find(|r| r.path == path)
        .unwrap();
    let res = route
        .handler
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status().as_u16();
    let kind = res
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = axum::body::to_bytes(res.into_body(), 1 << 24)
        .await
        .unwrap();
    (status, body.to_vec(), kind)
}

fn ids(list: &[&str]) -> Vec<KolId> {
    list.iter().map(|s| s.to_string()).collect()
}

// ---------- pure rules ----------

/// A fighter with every metric set; `f` edits the metrics.
fn fighter(kol: &str, score: u8, f: impl FnOnce(&mut Metrics)) -> FighterInput {
    let mut m = Metrics {
        d1: Some(100.0),
        d7: Some(1_000.0),
        d30: Some(10_000.0),
        wr7: Some(0.5),
        wr30: Some(0.5),
        wr_all: None,
        avg_pos: Some(300.0),
        last_min_at_fetch: Some(60),
        ..Metrics::default()
    };
    f(&mut m);
    FighterInput {
        kol: kol.into(),
        name: kol.to_uppercase(),
        rank: None,
        card: card(score, Tier::Sendoor),
        row: Some(MetricRow {
            date: day0(),
            kol: kol.into(),
            source: MetricSource::Kolexplorer,
            fetched_at: "2026-10-17T12:00:00Z".parse().unwrap(),
            metrics: m,
        }),
    }
}

fn outcome(r: &FightResult, kind: RoundKind) -> &RoundOutcome {
    &r.rounds.iter().find(|x| x.kind == kind).unwrap().outcome
}

#[test]
fn eight_rounds_in_order_with_hp_and_points() {
    let a = fighter("a", 60, |m| {
        m.d30 = Some(20_000.0);
        m.d7 = Some(2_000.0);
        m.d1 = Some(200.0);
        m.last_min_at_fetch = Some(5);
    });
    let b = fighter("b", 50, |m| {
        m.wr30 = Some(0.7);
        m.wr7 = Some(0.7);
        m.avg_pos = Some(900.0);
    });
    let r = resolve(&[a, b], day0());
    let kinds: Vec<RoundKind> = r.rounds.iter().map(|x| x.kind).collect();
    assert_eq!(kinds, RoundKind::ALL);
    assert_eq!(
        r.rounds.iter().map(|x| x.number).collect::<Vec<_>>(),
        (1..=8).collect::<Vec<u8>>()
    );
    // a: Score, PnL 30d, 7d, 24h, Last trade (lower wins). b: Win rate, Win rate 7d, Position.
    assert_eq!(
        outcome(&r, RoundKind::LastTrade),
        &RoundOutcome::Win { kol: "a".into() }
    );
    assert_eq!(
        outcome(&r, RoundKind::WinRate),
        &RoundOutcome::Win { kol: "b".into() }
    );
    assert_eq!((r.fighters[0].points, r.fighters[1].points), (5.0, 3.0));
    assert_eq!((r.fighters[0].hp, r.fighters[1].hp), (5, 3));
    assert_eq!(r.winner.as_deref(), Some("a"));
    assert_eq!(r.decision, Decision::Rounds);
    assert_eq!(r.counted_rounds, 8);
    // Values carry the row's source and fetch time; the Score round has none.
    let pnl = &r.rounds[1].values[0];
    assert_eq!(
        (pnl.value, pnl.source),
        (Some(20_000.0), Some(MetricSource::Kolexplorer))
    );
    assert!(pnl.fetched_at.is_some());
    assert_eq!(r.rounds[0].values[0].source, None);
    assert_eq!(
        r.summary,
        [
            "A wins 5–3 on rounds.",
            "A took Arena Score, PnL 30d, PnL 7d, PnL 24h, Last trade.",
            "B took Win rate, Win rate 7d, Position size.",
        ]
    );
}

#[test]
fn three_and_four_fighters() {
    let r = resolve(
        &[
            fighter("a", 70, |m| m.d30 = Some(1.0)),
            fighter("b", 40, |m| m.d30 = Some(3.0)),
            fighter("c", 20, |m| m.d30 = Some(2.0)),
        ],
        day0(),
    );
    assert_eq!(r.fighters.len(), 3);
    assert_eq!(
        outcome(&r, RoundKind::Score),
        &RoundOutcome::Win { kol: "a".into() }
    );
    assert_eq!(
        outcome(&r, RoundKind::Pnl30d),
        &RoundOutcome::Win { kol: "b".into() }
    );
    let round = &r.rounds[1];
    assert_eq!(round.damaged, ids(&["a", "c"]));

    let four = resolve(
        &[
            fighter("a", 70, |_| {}),
            fighter("b", 60, |_| {}),
            fighter("c", 50, |_| {}),
            fighter("d", 40, |_| {}),
        ],
        day0(),
    );
    assert_eq!(four.fighters.len(), 4);
    // Only the Score round differs; the rest are all tied, so void.
    assert_eq!(four.counted_rounds, 1);
    assert_eq!(four.winner.as_deref(), Some("a"));
    assert_eq!(
        four.fighters.iter().map(|f| f.hp).collect::<Vec<_>>(),
        [8, 7, 7, 7]
    );
    assert_eq!(four.summary.len(), 3);
    assert_eq!(
        four.summary[2],
        "B took no rounds; C took no rounds; D took no rounds."
    );
}

#[test]
fn missing_value_voids_the_round() {
    let r = resolve(
        &[
            fighter("a", 60, |m| m.d1 = None),
            fighter("b", 50, |m| m.d1 = Some(5.0)),
        ],
        day0(),
    );
    let round = &r.rounds[3];
    assert_eq!(round.kind, RoundKind::Pnl24h);
    assert_eq!(
        round.outcome,
        RoundOutcome::Void {
            reason: VoidReason::MissingData
        }
    );
    assert!(round.awards.is_empty() && round.damaged.is_empty());

    // No metric row at all: every metric round is void, the Score round still counts.
    let mut b = fighter("b", 50, |_| {});
    b.row = None;
    let r = resolve(&[fighter("a", 60, |_| {}), b], day0());
    assert_eq!(r.counted_rounds, 1);
    assert_eq!(r.counted_line, "1 of 8 rounds counted");
}

#[test]
fn win_rate_falls_back_to_all_time() {
    let r = resolve(
        &[
            fighter("a", 60, |m| {
                m.wr30 = None;
                m.wr_all = Some(0.9);
            }),
            fighter("b", 50, |_| {}),
        ],
        day0(),
    );
    let round = &r.rounds[4];
    assert!(round.values[0].all_time && !round.values[1].all_time);
    assert_eq!(round.outcome, RoundOutcome::Win { kol: "a".into() });
}

#[test]
fn tie_on_best_value_splits_the_point() {
    let r = resolve(
        &[
            fighter("a", 60, |m| m.d30 = Some(500.0)),
            fighter("b", 50, |m| m.d30 = Some(500.0)),
            fighter("c", 40, |m| m.d30 = Some(100.0)),
        ],
        day0(),
    );
    let round = &r.rounds[1];
    assert_eq!(
        round.outcome,
        RoundOutcome::Tie {
            kols: ids(&["a", "b"])
        }
    );
    assert_eq!(
        round.awards.iter().map(|a| a.points).collect::<Vec<_>>(),
        [0.5, 0.5]
    );
    assert_eq!(round.damaged, ids(&["c"]));
    assert!(r.fighters[1].rounds_won.contains(&RoundKind::Pnl30d));

    // Thirds add up exactly: three of four tie, three rounds in a row.
    let r = resolve(
        &[
            fighter("a", 60, |_| {}),
            fighter("b", 60, |_| {}),
            fighter("c", 60, |_| {}),
            fighter("d", 60, |m| {
                m.d30 = Some(1.0);
                m.d7 = Some(1.0);
                m.d1 = Some(1.0);
            }),
        ],
        day0(),
    );
    assert_eq!(r.counted_rounds, 3);
    assert_eq!(r.fighters[0].points, 1.0);
    assert_eq!(r.decision, Decision::Draw);
}

#[test]
fn all_tied_round_is_void() {
    let r = resolve(
        &[fighter("a", 50, |_| {}), fighter("b", 50, |_| {})],
        day0(),
    );
    for round in &r.rounds {
        assert_eq!(
            round.outcome,
            RoundOutcome::Void {
                reason: VoidReason::AllTied
            }
        );
    }
    assert_eq!(r.counted_rounds, 0);
    assert_eq!((r.fighters[0].hp, r.fighters[1].hp), (8, 8));
}

#[test]
fn rounds_tie_goes_to_higher_score() {
    // Score round to a; PnL 30d to b; everything else tied.
    let r = resolve(
        &[
            fighter("a", 60, |m| m.d30 = Some(1.0)),
            fighter("b", 50, |m| m.d30 = Some(2.0)),
        ],
        day0(),
    );
    assert_eq!((r.fighters[0].points, r.fighters[1].points), (1.0, 1.0));
    assert_eq!(r.decision, Decision::ScoreTiebreak);
    assert_eq!(r.winner.as_deref(), Some("a"));
    assert_eq!(
        r.summary[0],
        "Tie on rounds — A wins on higher Arena Score."
    );
}

#[test]
fn equal_rounds_and_score_is_a_draw() {
    let r = resolve(
        &[
            fighter("a", 50, |m| m.d30 = Some(99_999.0)),
            fighter("b", 50, |m| m.d7 = Some(9_999.0)),
        ],
        day0(),
    );
    assert_eq!(r.decision, Decision::Draw);
    assert_eq!(r.winner, None);
    assert_eq!(r.summary[0], "Draw: 1–1 on rounds and equal Arena Score.");
}

#[test]
fn counted_rounds_line() {
    let r = resolve(
        &[
            fighter("a", 60, |m| {
                m.d1 = None;
                m.wr7 = None;
            }),
            fighter("b", 50, |m| m.d30 = Some(1.0)),
        ],
        day0(),
    );
    // Void: 24h (missing), 7d win rate (missing), and three all-tied rounds.
    assert_eq!(r.counted_rounds, 2);
    assert_eq!(r.counted_line, "2 of 8 rounds counted");
}

// ---------- API and routes on day 0 ----------

#[tokio::test]
async fn day0_fight_of_three_on_latest_and_pinned_date() {
    let s = setup();
    let r = s
        .api
        .fight(&P, &ids(&["decu", "cooker", "trunoest"]), Some(day0()))
        .await
        .unwrap();
    assert_eq!(r.date, day0());
    assert_eq!(
        r.snapshot_closed_at,
        Some("2026-10-18T10:40:00Z".parse().unwrap())
    );
    let names: Vec<&str> = r.fighters.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Decu", "Cooker", "Trunoest"]);
    assert_eq!(
        r.fighters
            .iter()
            .map(|f| (f.rank, f.card.score))
            .collect::<Vec<_>>(),
        [(Some(1), 95), (Some(2), 88), (Some(3), 85)]
    );
    assert_eq!(
        r.rounds[0].outcome,
        RoundOutcome::Win { kol: "decu".into() }
    );
    assert!(r.missing.is_empty());
    // The same (ids, date) always gives the same result.
    let again = s
        .api
        .fight(&P, &ids(&["decu", "cooker", "trunoest"]), Some(day0()))
        .await
        .unwrap();
    assert_eq!(r, again);

    let latest = s
        .api
        .fight(&P, &ids(&["decu", "cooker"]), None)
        .await
        .unwrap();
    assert_eq!(latest.date, day1());
}

#[tokio::test]
async fn unknown_id_is_listed_and_too_few_fails() {
    let s = setup();
    let r = s
        .api
        .fight(&P, &ids(&["decu", "nobody-here", "cooker"]), Some(day0()))
        .await
        .unwrap();
    assert_eq!(r.missing, ids(&["nobody-here"]));
    assert_eq!(r.fighters.len(), 2);

    let err = s
        .api
        .fight(&P, &ids(&["decu", "nobody-here"]), Some(day0()))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        FightError::TooFewFighters {
            missing: ids(&["nobody-here"])
        }
    );
    let (status, body, _) = get(&s, "/api/fight?ids=decu,nobody-here&date=2026-10-18").await;
    assert_eq!(status, 400);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["missing"], serde_json::json!(["nobody-here"]));

    let five = ids(&["decu", "cooker", "trunoest", "tekkerrss", "peter"]);
    assert_eq!(
        s.api.fight(&P, &five, None).await.unwrap_err(),
        FightError::TooManyFighters { count: 5 }
    );
    assert_eq!(
        s.api
            .fight(&P, &ids(&["decu", "decu"]), None)
            .await
            .unwrap_err(),
        FightError::DuplicateFighter { kol: "decu".into() }
    );
}

#[tokio::test]
async fn stale_snapshot_link_keeps_its_fight() {
    let s = setup();
    // Tekkerrss left the board on day 1; the day-0 link still shows the day-0 fight.
    let old = s
        .api
        .fight(&P, &ids(&["tekkerrss", "decu"]), Some(day0()))
        .await
        .unwrap();
    assert_eq!(old.date, day0());
    assert_eq!(old.fighters.len(), 2);
    assert_eq!(old.fighters[0].card.score, 55);
    // On fresh data Tekkerrss is not on the board.
    let fresh = s
        .api
        .fight(&P, &ids(&["tekkerrss", "decu", "cooker"]), None)
        .await
        .unwrap();
    assert_eq!(fresh.missing, ids(&["tekkerrss"]));
    // A date outside the stored range: no snapshot, 404.
    let gone = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    assert_eq!(
        s.api
            .fight(&P, &ids(&["decu", "cooker"]), Some(gone))
            .await
            .unwrap_err(),
        FightError::NoSnapshot { date: Some(gone) }
    );
    let (status, _, _) = get(&s, "/api/fight?ids=decu,cooker&date=2026-09-01").await;
    assert_eq!(status, 404);
    let (status, _, _) = get(&s, "/api/fight?ids=decu,cooker&date=2026-9-1").await;
    assert_eq!(status, 400);
}

fn png_size(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[12..16], b"IHDR");
    let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
    (w, h)
}

#[tokio::test]
async fn image_is_a_1200x630_png_cached_by_snapshot_close() {
    let s = setup();
    let uri = "/api/fight/image?ids=decu,tekkerrss,cooker&date=2026-10-18";
    let (status, png, kind) = get(&s, uri).await;
    assert_eq!((status, kind.as_str()), (200, "image/png"));
    assert_eq!(png_size(&png), (1200, 630));
    assert!(png.len() > 10_000, "image has content: {} bytes", png.len());
    let (_, same, _) = get(&s, uri).await;
    assert_eq!(png, same);

    // Re-collect day 0: Tekkerrss now tops PnL 30d, and the snapshot closes again.
    {
        let mut store = s.store.lock().unwrap();
        let view = store.views.get_mut(&day0()).unwrap();
        view.snapshot.closed_at = view.snapshot.closed_at.map(|t| t + Duration::hours(1));
        let m = view
            .members
            .iter_mut()
            .find(|m| m.kol == "tekkerrss")
            .unwrap();
        m.row.as_mut().unwrap().metrics.d30 = Some(9_000_000.0);
    }
    let (_, fresh, _) = get(&s, uri).await;
    assert_eq!(png_size(&fresh), (1200, 630));
    assert_ne!(
        png, fresh,
        "a re-collected date must not serve the old image"
    );

    let (status, _, _) = get(&s, "/api/fight/image?ids=decu&date=2026-10-18").await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn fight_page_meta() {
    let s = setup();
    let meta = s
        .plugin
        .page_meta("/fight", Some("ids=decu%2Ccooker&date=2026-10-18"))
        .await
        .unwrap();
    assert_eq!(meta.title.as_deref(), Some("Decu vs Cooker — KOL Fight"));
    let tag = |key: &str| {
        meta.tags
            .iter()
            .find(|t| t.key == key)
            .unwrap_or_else(|| panic!("{key}"))
    };
    assert_eq!(tag("og:image").attr, MetaAttr::Property);
    assert_eq!(
        tag("og:image").content,
        "/api/fight/image?ids=decu,cooker&date=2026-10-18"
    );
    assert_eq!(tag("twitter:card").content, "summary_large_image");
    assert_eq!(tag("twitter:image").attr, MetaAttr::Name);
    assert!(tag("og:description").content.contains("rounds counted"));
    for key in [
        "og:title",
        "og:image:width",
        "og:image:height",
        "twitter:title",
    ] {
        tag(key);
    }
    // A link without a date pins the latest snapshot in the image link.
    let latest = s
        .plugin
        .page_meta("/fight", Some("ids=decu,cooker"))
        .await
        .unwrap();
    assert!(
        latest
            .tags
            .iter()
            .any(|t| t.content.ends_with("date=2026-10-19"))
    );
    // A fight that cannot run gets the generic meta; other paths get none.
    let generic = s.plugin.page_meta("/fight", None).await.unwrap();
    assert_eq!(generic.title.as_deref(), Some("KOL Fight — KOL Arena"));
    assert!(s.plugin.page_meta("/", None).await.is_none());
}

// ---------- leak ----------

fn keys(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                out.insert(k.clone());
                keys(x, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| keys(x, out)),
        _ => {}
    }
}

/// Every key `/api/fight` may send.
const FIGHT_KEYS: &[&str] = &[
    "date",
    "snapshot_closed_at",
    "fighters",
    "rounds",
    "winner",
    "decision",
    "counted_rounds",
    "counted_line",
    "summary",
    "missing",
    // fighter
    "kol",
    "name",
    "rank",
    "card",
    "points",
    "hp",
    "rounds_won",
    // score card
    "score",
    "tier",
    "parts",
    "badges",
    "kind",
    "raw",
    "all_time",
    "share",
    "status",
    // round
    "number",
    "values",
    "outcome",
    "awards",
    "damaged",
    "value",
    "source",
    "fetched_at",
    "type",
    "kols",
    "reason",
];

pub fn assert_public(v: &serde_json::Value, allow: &[&str]) {
    let mut found = BTreeSet::new();
    keys(v, &mut found);
    let extra: Vec<&String> = found
        .iter()
        .filter(|k| !allow.contains(&k.as_str()))
        .collect();
    assert!(extra.is_empty(), "keys outside the allowlist: {extra:?}");
    for k in &found {
        for bad in ["rate", "group", "note", "hypothes", "contact", "referral"] {
            assert!(!k.contains(bad), "private key {k}");
        }
    }
}

#[tokio::test]
async fn fight_json_has_only_public_keys() {
    let s = setup();
    let (status, body, _) = get(
        &s,
        "/api/fight?ids=decu,tekkerrss,nobody,cooker&date=2026-10-18",
    )
    .await;
    assert_eq!(status, 200);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["fighters"].as_array().unwrap().len(), 3);
    assert_eq!(v["missing"], serde_json::json!(["nobody"]));
    assert_public(&v, FIGHT_KEYS);
}
