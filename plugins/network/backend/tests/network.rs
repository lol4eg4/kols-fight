//! Network section acceptance: members on the board, the waiting list, the leak allowlist,
//! an empty network. Stand-in trait impls only; invented KOLs and addresses.

use api_registry::{
    KolLinks, KolProfile, KolUpsert, PublishedWallet, Reach, ReachRecord, RegistryApi,
    RegistryError, WalletSource,
};
use api_roster::{NetworkMember, Rates, RosterApi, RosterError, XHandle};
use api_score::{Badge, RankedKol, Ranking, ScoreApi, ScoreCard, ScoreError, Tier};
use api_snapshots::{
    Member, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotStatus, SnapshotView,
    SnapshotsApi, SnapshotsError,
};
use api_wallets::{Candidate, KolWalletStatus, WalletStatus, WalletsApi, WalletsError};
use arena_platform::{Api, Apis, Person, Registry};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use plugin_network::{Network, routes};
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

fn at(d: u32, h: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, d, h, 0, 0).unwrap()
}

// Invented addresses: a confirmed main, a hypothesis, a sheet wallet, another wallet.
const W_MAIN: &str = "4vw54BmAogeRV3vPKWyFjzo2i2uLyqPEKvmbWX7DHZRe";
const W_HYPO: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
const W_SHEET: &str = "DfMxre4cKmvogbLrPigxmibVTTQDuzjdXojWzjCXXhzj";
const W_OTHER: &str = "GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ";
/// Team-only texts that must never reach a client.
const SECRETS: &[&str] = &[
    W_MAIN,
    W_HYPO,
    W_SHEET,
    W_OTHER,
    "fomo_alder",
    "pump_alder",
    "tg_alder",
    "ref=alder-77",
    "Group A",
    "1500",
];

// ----- stand-ins ------------------------------------------------------------------------

/// Every person an API was called with.
#[derive(Default)]
struct Calls(Mutex<Vec<(&'static str, Person)>>);

impl Calls {
    fn add(&self, api: &'static str, p: &Person) {
        self.0.lock().unwrap().push((api, *p));
    }
}

struct FakeRoster {
    members: Vec<NetworkMember>,
    calls: Arc<Calls>,
}

#[async_trait]
impl RosterApi for FakeRoster {
    async fn members(&self, p: &Person) -> Result<Vec<NetworkMember>, RosterError> {
        self.calls.add("roster.members", p);
        Ok(self.members.clone())
    }
    async fn is_member(&self, p: &Person, kol: &str) -> Result<bool, RosterError> {
        self.calls.add("roster.is_member", p);
        Ok(self.members.iter().any(|m| m.kol == kol))
    }
    async fn verified_handle(&self, p: &Person, _: &str) -> Result<Option<XHandle>, RosterError> {
        self.calls.add("roster.verified_handle", p);
        Ok(None)
    }
    async fn rates(&self, p: &Person, kols: &[String]) -> Result<Vec<Rates>, RosterError> {
        // A careless caller would get rates here; the section must never ask.
        self.calls.add("roster.rates", p);
        Ok(kols
            .iter()
            .map(|k| Rates {
                kol: k.clone(),
                low_usd: Some(1500.0),
                high_usd: Some(3000.0),
                formats: vec!["post".into()],
            })
            .collect())
    }
}

struct FakeWallets {
    status: HashMap<String, WalletStatus>,
    calls: Arc<Calls>,
}

#[async_trait]
impl WalletsApi for FakeWallets {
    async fn propose(
        &self,
        p: &Person,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<Candidate, WalletsError> {
        self.calls.add("wallets.propose", p);
        Err(WalletsError::NotBuilt)
    }
    async fn status(&self, p: &Person, kol: &str) -> Result<WalletStatus, WalletsError> {
        self.calls.add("wallets.status", p);
        self.status
            .get(kol)
            .cloned()
            .ok_or(WalletsError::NotMember { kol: kol.into() })
    }
}

struct FakeScore {
    ranking: Result<Ranking, ScoreError>,
}

#[async_trait]
impl ScoreApi for FakeScore {
    async fn score(&self, _: &Person, _: &Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
    async fn rank(&self, _: &Person, _: NaiveDate) -> Result<(), ScoreError> {
        Err(ScoreError::NotBuilt)
    }
    async fn ranking(&self, _: &Person, date: NaiveDate) -> Result<Ranking, ScoreError> {
        match &self.ranking {
            Ok(r) if r.date == date => Ok(r.clone()),
            Ok(_) => Err(ScoreError::NoSnapshot { date }),
            Err(e) => Err(e.clone()),
        }
    }
}

struct FakeSnapshots {
    view: SnapshotView,
}

fn unused() -> SnapshotsError {
    SnapshotsError::Internal {
        message: "not used".into(),
    }
}

#[async_trait]
impl SnapshotsApi for FakeSnapshots {
    async fn open(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(unused())
    }
    async fn close(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(unused())
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
        Err(unused())
    }
    async fn set_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
        _: &[String],
    ) -> Result<(), SnapshotsError> {
        Err(unused())
    }
    async fn copy_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
    ) -> Result<(), SnapshotsError> {
        Err(unused())
    }
    async fn snapshot(&self, _: &Person, date: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        if date == self.view.snapshot.date {
            Ok(self.view.clone())
        } else {
            Err(SnapshotsError::NoSnapshot { date })
        }
    }
    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Ok(
            (self.view.snapshot.status == SnapshotStatus::Closed)
                .then_some(self.view.snapshot.date),
        )
    }
    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Err(unused())
    }
    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        Err(unused())
    }
}

struct FakeRegistry {
    profiles: Vec<KolProfile>,
}

fn reg_unused() -> RegistryError {
    RegistryError::Internal {
        message: "not used".into(),
    }
}

#[async_trait]
impl RegistryApi for FakeRegistry {
    async fn upsert_kol(&self, _: &Person, _: &KolUpsert) -> Result<String, RegistryError> {
        Err(reg_unused())
    }
    async fn get_kol(&self, _: &Person, id: &str) -> Result<Option<KolProfile>, RegistryError> {
        Ok(self.profiles.iter().find(|p| p.id == id).cloned())
    }
    async fn list_kols(
        &self,
        _: &Person,
        ids: &[String],
    ) -> Result<Vec<KolProfile>, RegistryError> {
        Ok(ids
            .iter()
            .filter_map(|id| self.profiles.iter().find(|p| &p.id == id).cloned())
            .collect())
    }
    async fn find_by_handle(&self, _: &Person, _: &str) -> Result<Option<String>, RegistryError> {
        Err(reg_unused())
    }
    async fn find_by_wallet(&self, _: &Person, _: &str) -> Result<Option<String>, RegistryError> {
        Err(reg_unused())
    }
    async fn publish_wallets(
        &self,
        _: &Person,
        _: &str,
        _: Option<&str>,
        _: &[String],
        _: WalletSource,
    ) -> Result<(), RegistryError> {
        Err(reg_unused())
    }
    async fn set_reach(
        &self,
        _: &Person,
        _: &str,
        _: &Reach,
        _: &str,
        _: NaiveDate,
    ) -> Result<(), RegistryError> {
        Err(reg_unused())
    }
    async fn set_formats(&self, _: &Person, _: &str, _: &[String]) -> Result<(), RegistryError> {
        Err(reg_unused())
    }
}

// ----- world ----------------------------------------------------------------------------

fn member(kol: &str, name: &str) -> NetworkMember {
    NetworkMember {
        kol: kol.into(),
        name: name.into(),
        x_handle: Some(format!("x_{kol}")),
        x_verified: false,
        fomo_handle: Some(format!("fomo_{kol}")),
        pumpfun: Some(format!("pump_{kol}")),
        // A roster that wrongly hands sheet wallets to a visitor: the section drops them.
        sheet_wallets: vec![W_SHEET.into()],
        formats: vec!["post".into(), "call".into()],
        keep: false,
    }
}

fn profile(id: &str, name: &str) -> KolProfile {
    let wallet = |a: &str| PublishedWallet {
        address: a.into(),
        source: WalletSource::Network,
        published_at: at(17, 6),
    };
    KolProfile {
        id: id.into(),
        name: name.into(),
        x_handle: Some(format!("x_{id}")),
        links: KolLinks {
            fomo: Some(format!("fomo_{id}")),
            pumpfun: Some(format!("pump_{id}")),
            telegram: Some(format!("tg_{id}")),
            ..KolLinks::default()
        },
        main_wallet: Some(wallet(W_MAIN)),
        other_wallets: vec![wallet(W_OTHER)],
        reach: Some(ReachRecord {
            reach: Reach {
                x_followers: Some(12_000),
                telegram_subscribers: None,
                pumpfun_followers: None,
            },
            source: "ref=alder-77".into(),
            date: day(17),
        }),
        formats: vec!["post".into()],
    }
}

fn metrics(d30: f64, vol30: f64, days30: i32) -> Metrics {
    Metrics {
        d30: Some(d30),
        vol30: Some(vol30),
        days30: Some(days30),
        ..Metrics::default()
    }
}

fn snap_member(kol: &str, m: Option<Metrics>) -> Member {
    Member {
        kol: kol.into(),
        reasons: vec![MemberReason::Network],
        row: m.map(|metrics| MetricRow {
            date: day(20),
            kol: kol.into(),
            source: MetricSource::Solanatracker,
            fetched_at: at(20, 6),
            metrics,
        }),
    }
}

fn ranked(kol: &str, rank: u32, score: u8, tier: Tier) -> RankedKol {
    RankedKol {
        kol: kol.into(),
        rank,
        card: ScoreCard {
            score,
            tier,
            parts: Vec::new(),
            badges: vec![Badge::OnFire],
        },
    }
}

fn status(
    kol: &str,
    s: KolWalletStatus,
    main: Option<&str>,
    last: Option<DateTime<Utc>>,
) -> WalletStatus {
    WalletStatus {
        kol: kol.into(),
        status: s,
        main_address: main.map(String::from),
        last_trade_at: last,
    }
}

struct World {
    members: Vec<NetworkMember>,
    profiles: Vec<KolProfile>,
    wallets: Vec<WalletStatus>,
    snapshot: Vec<Member>,
    snapshot_status: SnapshotStatus,
    ranking: Result<Ranking, ScoreError>,
}

/// Five members: alder and briar ranked; cobalt hypothesis; dune nothing found;
/// ember confirmed but quiet. Plus a non-member on the board.
fn world() -> World {
    World {
        members: vec![
            member("alder", "Alder Vane"),
            member("briar", "Briar Quill"),
            member("cobalt", "Cobalt Finch"),
            member("dune", "Dune Harrow"),
            member("ember", "Ember Lysk"),
        ],
        profiles: vec![
            profile("alder", "Alder Vane"),
            profile("briar", "Briar Quill"),
            profile("cobalt", "Cobalt Finch"),
            profile("dune", "Dune Harrow"),
            profile("ember", "Ember Lysk"),
        ],
        wallets: vec![
            status(
                "alder",
                KolWalletStatus::Confirmed,
                Some(W_MAIN),
                Some(at(20, 3)),
            ),
            status(
                "briar",
                KolWalletStatus::Confirmed,
                Some(W_OTHER),
                Some(at(19, 3)),
            ),
            // the wallets API never gives a hypothesis address; a careless fake might
            status("cobalt", KolWalletStatus::Hypothesis, Some(W_HYPO), None),
            status("dune", KolWalletStatus::NotFound, None, None),
            status(
                "ember",
                KolWalletStatus::Confirmed,
                Some(W_MAIN),
                Some(at(8, 12)),
            ),
        ],
        snapshot: vec![
            snap_member("alder", Some(metrics(-1250.5, 88_000.0, 21))),
            snap_member("briar", Some(metrics(40_200.0, 510_000.0, 29))),
            snap_member("outsider", Some(metrics(9.0, 9.0, 1))),
            snap_member("ember", None),
        ],
        snapshot_status: SnapshotStatus::Closed,
        ranking: Ok(Ranking {
            date: day(20),
            ranked_at: at(20, 7),
            rows: vec![
                ranked("briar", 3, 74, Tier::Goat),
                ranked("outsider", 5, 60, Tier::Diamond),
                ranked("alder", 12, 41, Tier::Sendoor),
            ],
        }),
    }
}

struct Env {
    app: Router,
    calls: Arc<Calls>,
}

fn env(w: World) -> Env {
    let calls = Arc::new(Calls::default());
    let apis = Apis::new(Registry::new("test", &[]));
    apis.provide::<dyn RosterApi>(
        "roster",
        vec![(
            "default",
            Arc::new(FakeRoster {
                members: w.members,
                calls: calls.clone(),
            }) as Arc<dyn RosterApi>,
        )],
        Arc::new(api_roster::stub::Stub),
    )
    .unwrap();
    apis.provide::<dyn WalletsApi>(
        "wallets",
        vec![(
            "default",
            Arc::new(FakeWallets {
                status: w.wallets.into_iter().map(|s| (s.kol.clone(), s)).collect(),
                calls: calls.clone(),
            }) as Arc<dyn WalletsApi>,
        )],
        Arc::new(api_wallets::stub::Stub),
    )
    .unwrap();
    apis.provide::<dyn ScoreApi>(
        "score",
        vec![(
            "default",
            Arc::new(FakeScore { ranking: w.ranking }) as Arc<dyn ScoreApi>,
        )],
        Arc::new(api_score::stub::Stub),
    )
    .unwrap();
    apis.provide::<dyn SnapshotsApi>(
        "snapshots",
        vec![(
            "default",
            Arc::new(FakeSnapshots {
                view: SnapshotView {
                    snapshot: Snapshot {
                        date: day(20),
                        status: w.snapshot_status,
                        opened_at: at(20, 6),
                        closed_at: Some(at(20, 7)),
                    },
                    members: w.snapshot,
                },
            }) as Arc<dyn SnapshotsApi>,
        )],
        Arc::new(api_snapshots::stub::Stub),
    )
    .unwrap();
    apis.provide::<dyn RegistryApi>(
        "registry",
        vec![(
            "default",
            Arc::new(FakeRegistry {
                profiles: w.profiles,
            }) as Arc<dyn RegistryApi>,
        )],
        Arc::new(api_registry::stub::Stub),
    )
    .unwrap();
    fn get<T: ?Sized + Send + Sync + 'static>(a: &Apis) -> Api<T> {
        a.get().unwrap()
    }
    let network = Network::new(get(&apis), get(&apis), get(&apis), get(&apis), get(&apis));
    let mut app = Router::default();
    for route in routes(&network) {
        assert_eq!(route.tag, arena_platform::Tag::Public);
        app = app.route(&route.path, route.handler);
    }
    Env { app, calls }
}

async fn get_json(app: &Router, uri: &str) -> (StatusCode, Value) {
    let mut req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    req.extensions_mut().insert(Person::NOBODY);
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn kols(v: &Value, list: &str) -> Vec<String> {
    v[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["kol"].as_str().unwrap().to_string())
        .collect()
}

// ----- tests ----------------------------------------------------------------------------

#[tokio::test]
async fn members_on_the_board_with_score_tier_and_30_day_numbers() {
    let e = env(world());
    let (code, v) = get_json(&e.app, "/api/network?date=2026-10-20").await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(v["date"], "2026-10-20");
    // by rank; the non-member on the board is not in the section
    assert_eq!(kols(&v, "members"), ["briar", "alder"]);
    let briar = &v["members"][0];
    assert_eq!(briar["name"], "Briar Quill");
    assert_eq!(briar["rank"], 3);
    assert_eq!(briar["score"], 74);
    assert_eq!(briar["tier"], "goat");
    assert_eq!(briar["pnl_30d"], 40_200.0);
    assert_eq!(briar["volume_30d"], 510_000.0);
    assert_eq!(briar["days_in_market"], 29);
    let alder = &v["members"][1];
    assert_eq!(alder["tier"], "sendoor");
    assert_eq!(alder["pnl_30d"], -1250.5);
}

#[tokio::test]
async fn waiting_list_gives_only_the_two_safe_reasons() {
    let e = env(world());
    let (code, v) = get_json(&e.app, "/api/network?date=2026-10-20").await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(kols(&v, "waiting"), ["cobalt", "dune", "ember"]);
    let w = v["waiting"].as_array().unwrap();
    // hypothesis and nothing found read the same: the client cannot tell them apart
    assert_eq!(w[0]["reason"], "wallet_not_confirmed");
    assert_eq!(w[0]["quiet_days"], Value::Null);
    assert_eq!(w[1]["reason"], "wallet_not_confirmed");
    assert_eq!(w[1]["quiet_days"], Value::Null);
    let without_ids = |x: &Value| {
        let mut x = x.clone();
        x.as_object_mut().unwrap().remove("kol");
        x.as_object_mut().unwrap().remove("name");
        x
    };
    assert_eq!(without_ids(&w[0]), without_ids(&w[1]));
    // confirmed wallet, last trade Oct 8 12:00, board of Oct 20: 12 days
    assert_eq!(w[2]["reason"], "quiet");
    assert_eq!(w[2]["quiet_days"], 12);
    // a network KOL on the snapshot without a row is not on the board
    assert!(!kols(&v, "members").contains(&"ember".to_string()));
}

#[tokio::test]
async fn before_ranking_the_board_is_the_snapshot_rows_without_scores() {
    let mut w = world();
    w.ranking = Err(ScoreError::NotRanked { date: day(20) });
    let e = env(w);
    let (code, v) = get_json(&e.app, "/api/network").await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(kols(&v, "members"), ["alder", "briar"]);
    assert_eq!(v["members"][0]["score"], Value::Null);
    assert_eq!(v["members"][0]["tier"], Value::Null);
    assert_eq!(v["members"][0]["days_in_market"], 21);
}

/// Every key anywhere in the response must be on this list.
const ALLOWED_KEYS: &[&str] = &[
    "date",
    "members",
    "waiting",
    "kol",
    "name",
    "rank",
    "score",
    "tier",
    "pnl_30d",
    "volume_30d",
    "days_in_market",
    "reason",
    "quiet_days",
];

fn walk(v: &Value, keys: &mut BTreeSet<String>, strings: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                keys.insert(k.clone());
                walk(x, keys, strings);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk(x, keys, strings)),
        Value::String(s) => strings.push(s.clone()),
        Value::Number(n) => strings.push(n.to_string()),
        _ => {}
    }
}

#[tokio::test]
async fn the_response_holds_only_allowed_keys_and_no_team_data() {
    for ranking in [true, false] {
        let mut w = world();
        if !ranking {
            w.ranking = Err(ScoreError::NotRanked { date: day(20) });
        }
        let e = env(w);
        let (code, v) = get_json(&e.app, "/api/network?date=2026-10-20").await;
        assert_eq!(code, StatusCode::OK);
        let raw = v.to_string();
        let (mut keys, mut values) = (BTreeSet::new(), Vec::new());
        walk(&v, &mut keys, &mut values);
        let extra: Vec<_> = keys
            .iter()
            .filter(|k| !ALLOWED_KEYS.contains(&k.as_str()))
            .collect();
        assert!(extra.is_empty(), "keys outside the allowlist: {extra:?}");
        for s in SECRETS {
            assert!(!raw.contains(s), "team data in the response: {s}");
        }
        assert!(values.iter().all(|s| !api_roster::is_solana_address(s)));
        // No staff API was called, and every call ran as a visitor.
        let calls = e.calls.0.lock().unwrap();
        assert!(calls.iter().all(|(api, _)| *api != "roster.rates"));
        assert!(calls.iter().all(|(_, p)| *p == Person::NOBODY));
    }
}

#[tokio::test]
async fn an_empty_network_is_two_empty_lists() {
    let mut w = world();
    w.members.clear();
    let e = env(w);
    let (code, v) = get_json(&e.app, "/api/network?date=2026-10-20").await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(v["members"], Value::Array(vec![]));
    assert_eq!(v["waiting"], Value::Array(vec![]));
}

#[tokio::test]
async fn bad_or_unknown_dates_and_open_snapshots() {
    let e = env(world());
    let (code, v) = get_json(&e.app, "/api/network?date=20-10-2026").await;
    assert_eq!(code, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"]["code"], "bad_date");
    let (code, v) = get_json(&e.app, "/api/network?date=2026-10-19").await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    assert_eq!(v["error"]["code"], "no_snapshot");

    // an open snapshot is not published yet
    let mut w = world();
    w.snapshot_status = SnapshotStatus::Open;
    let e = env(w);
    let (code, v) = get_json(&e.app, "/api/network?date=2026-10-20").await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    assert_eq!(v["error"]["code"], "no_snapshot");
    let (code, _) = get_json(&e.app, "/api/network").await;
    assert_eq!(code, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_upstream_failure_is_a_plain_502() {
    let mut w = world();
    w.ranking = Err(ScoreError::Internal {
        message: format!("db down near {W_HYPO}"),
    });
    let e = env(w);
    let (code, v) = get_json(&e.app, "/api/network?date=2026-10-20").await;
    assert_eq!(code, StatusCode::BAD_GATEWAY);
    assert_eq!(v["error"]["code"], "unavailable");
    assert!(!v.to_string().contains(W_HYPO));
}
