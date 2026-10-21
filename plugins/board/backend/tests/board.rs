//! Board routes over stand-in snapshots, score and registry APIs fed from the day-0 fixture
//! (a plugin backend never depends on another plugin's backend).

use api_board::InNetwork;
use api_registry::{
    KolId, KolLinks, KolProfile, KolUpsert, PublishedWallet, Reach, ReachRecord, RegistryApi,
    RegistryError, WalletSource,
};
use api_score::{RankedKol, Ranking, ScoreApi, ScoreCard, ScoreError, Tier};
use api_snapshots::{
    MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotView, SnapshotsApi,
    SnapshotsError,
};
use arena_platform::{Config, Ctx, Person, Plugin};
use async_trait::async_trait;
use axum::{body::Body, http::Request};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use plugin_board::{is_stale, parse_date, solscan_url};
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};
use tower::ServiceExt;

const DAY0_VIEW: &str =
    include_str!("../../../snapshots/backend/tests/fixtures/day0_2026-10-18.json");
const PROTOTYPE: &str = include_str!("../../../../data/prototype/arena_data_v2.json");
const EXPECTED: &str = include_str!("../../../../data/prototype/expected_ranking_2026-10-18.json");

const MAIN: &str = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU";
const OTHER: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

fn day0() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()
}

fn day1() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
}

// ---------- stand-ins ----------

struct FakeSnapshots(HashMap<NaiveDate, SnapshotView>);

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
        self.0
            .get(&date)
            .cloned()
            .ok_or(SnapshotsError::NoSnapshot { date })
    }
    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Ok(self.0.keys().max().copied())
    }
    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        let mut d: Vec<NaiveDate> = self.0.keys().copied().collect();
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

struct FakeScore(HashMap<NaiveDate, Ranking>);

#[async_trait]
impl ScoreApi for FakeScore {
    async fn score(&self, _: &Person, _: &Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
    async fn rank(&self, _: &Person, _: NaiveDate) -> Result<(), ScoreError> {
        Err(ScoreError::NotBuilt)
    }
    async fn ranking(&self, _: &Person, date: NaiveDate) -> Result<Ranking, ScoreError> {
        self.0
            .get(&date)
            .cloned()
            .ok_or(ScoreError::NotRanked { date })
    }
    async fn ranked_dates(&self, _: &Person) -> Result<Vec<NaiveDate>, ScoreError> {
        let mut dates: Vec<NaiveDate> = self.0.keys().copied().collect();
        dates.sort();
        Ok(dates)
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
        _: Option<(&str, WalletSource)>,
        _: &[(String, WalletSource)],
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

/// Roster's filler of the `in_network` slot: only Cooker is in the network.
struct Members;

#[async_trait]
impl InNetwork for Members {
    async fn is_member(&self, _: &Person, kol: &str) -> bool {
        kol == "cooker"
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

fn day0_ranking() -> Ranking {
    let rows: Vec<Value> = serde_json::from_str(EXPECTED).unwrap();
    Ranking {
        date: day0(),
        ranked_at: "2026-10-18T10:41:00Z".parse().unwrap(),
        rows: rows
            .iter()
            .map(|r| RankedKol {
                kol: r["slug"].as_str().unwrap().into(),
                rank: r["rank"].as_u64().unwrap() as u32,
                card: ScoreCard {
                    score: r["score"].as_u64().unwrap() as u8,
                    tier: tier_of(r["tier"].as_str().unwrap()),
                    parts: Vec::new(),
                    badges: Vec::new(),
                },
            })
            .collect(),
    }
}

fn wallet(address: &str) -> PublishedWallet {
    PublishedWallet {
        address: address.into(),
        source: WalletSource::Network,
        published_at: "2026-10-18T09:00:00Z".parse().unwrap(),
    }
}

fn profiles() -> HashMap<String, KolProfile> {
    let v: Value = serde_json::from_str(PROTOTYPE).unwrap();
    let mut out: HashMap<String, KolProfile> = v["kols"]
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
        .collect();
    let cooker = out.get_mut("cooker").unwrap();
    cooker.main_wallet = Some(wallet(MAIN));
    // An address that is not base58 is never published as a link.
    cooker.other_wallets = vec![wallet(OTHER), wallet("not-a-wallet<script>")];
    cooker.links = KolLinks {
        kolexplorer: Some("cooker".into()),
        solanatracker: true,
        kolscan: false,
        fomo: Some("@cookerfomo".into()),
        pumpfun: Some("bad handle\"".into()),
        telegram: None,
    };
    cooker.formats = vec!["post".into(), "stream".into()];
    cooker.reach = Some(ReachRecord {
        reach: Reach {
            x_followers: Some(12_000),
            telegram_subscribers: None,
            pumpfun_followers: None,
        },
        source: "x".into(),
        date: day0(),
    });
    // A registry entry that is not on the board.
    out.insert(
        "offboard".into(),
        KolProfile {
            id: "offboard".into(),
            name: "Off Board".into(),
            x_handle: None,
            links: KolLinks::default(),
            main_wallet: None,
            other_wallets: Vec::new(),
            reach: None,
            formats: Vec::new(),
        },
    );
    out
}

struct Setup {
    plugin: Arc<dyn Plugin>,
    ctx: Ctx,
}

/// Day 0 and day 1. Fetch times are relative to now: Decu 49h ago (stale), Cooker 47h ago,
/// everyone else an hour ago. On day 1 Tekkerrss has left the board.
fn setup(fill_slot: bool) -> Setup {
    build(fill_slot, true)
}

/// `setup`, with day 1 closed but not ranked yet when `rank_day1` is false.
fn build(fill_slot: bool, rank_day1: bool) -> Setup {
    let now = Utc::now();
    let mut view0: SnapshotView = serde_json::from_str(DAY0_VIEW).unwrap();
    for m in &mut view0.members {
        let ago = match m.kol.as_str() {
            "decu" => 49,
            "cooker" => 47,
            _ => 1,
        };
        m.row.as_mut().unwrap().fetched_at = now - Duration::hours(ago);
    }
    let mut view1 = view0.clone();
    view1.snapshot.date = day1();
    view1.snapshot.closed_at = Some("2026-10-19T06:30:00Z".parse().unwrap());
    view1.members.retain(|m| m.kol != "tekkerrss");
    let r0 = day0_ranking();
    let mut r1 = r0.clone();
    r1.date = day1();
    r1.rows.retain(|r| r.kol != "tekkerrss");

    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://x@127.0.0.1:1/x")
        .unwrap();
    let ctx = Ctx::new(pool, Config::default(), "test").unwrap();
    ctx.apis
        .provide::<dyn SnapshotsApi>(
            "snapshots",
            vec![(
                "default",
                Arc::new(FakeSnapshots(HashMap::from([
                    (day0(), view0),
                    (day1(), view1),
                ]))),
            )],
            Arc::new(api_snapshots::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn ScoreApi>(
            "score",
            vec![(
                "default",
                Arc::new(FakeScore(HashMap::from_iter(
                    [(day0(), r0), (day1(), r1)]
                        .into_iter()
                        .filter(|(d, _)| rank_day1 || *d != day1()),
                ))),
            )],
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
    let plugin = (plugin_board::ENTRY.build)(&ctx).unwrap();
    if fill_slot {
        ctx.slots
            .fill::<dyn InNetwork>("roster", Arc::new(Members))
            .unwrap();
    }
    Setup { plugin, ctx }
}

async fn get(s: &Setup, uri: &str) -> (u16, Value) {
    let path = uri.split('?').next().unwrap();
    let pattern = if path.starts_with("/api/kol/") {
        "/api/kol/{id}"
    } else {
        path
    };
    let route = s
        .plugin
        .routes(&s.ctx)
        .into_iter()
        .find(|r| r.path == pattern)
        .unwrap();
    let res = route
        .handler
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status().as_u16();
    let body = axum::body::to_bytes(res.into_body(), 1 << 24)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

fn row<'a>(board: &'a Value, kol: &str) -> &'a Value {
    board["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kol"] == kol)
        .unwrap_or_else(|| panic!("{kol}"))
}

// ---------- helpers ----------

#[test]
fn stale_after_48_hours() {
    let now: DateTime<Utc> = "2026-10-20T12:00:00Z".parse().unwrap();
    assert!(!is_stale(now - Duration::hours(48), now));
    assert!(is_stale(
        now - Duration::hours(48) - Duration::seconds(1),
        now
    ));
    assert!(!is_stale(now, now));
}

#[test]
fn dates_are_strict() {
    assert_eq!(parse_date("2026-10-18"), Some(day0()));
    for bad in [
        "2026-10-8",
        "2026/10/18",
        "18-10-2026",
        "2026-13-01",
        "",
        "2026-10-18x",
    ] {
        assert_eq!(parse_date(bad), None, "{bad}");
    }
    assert_eq!(
        solscan_url(MAIN),
        format!("https://solscan.io/account/{MAIN}")
    );
}

// ---------- /api/board ----------

#[tokio::test]
async fn board_of_a_date() {
    let s = setup(false);
    let (status, b) = get(&s, "/api/board?date=2026-10-18").await;
    assert_eq!(status, 200);
    assert_eq!(b["date"], "2026-10-18");
    assert_eq!(b["closed_at"], "2026-10-18T10:40:00Z");
    assert_eq!(b["dates"], serde_json::json!(["2026-10-18", "2026-10-19"]));
    let rows = b["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 122);
    assert_eq!(
        (rows[0]["kol"].as_str(), rows[0]["rank"].as_u64()),
        (Some("decu"), Some(1))
    );
    assert_eq!(rows[0]["name"], "Decu");
    assert_eq!(rows[0]["x_handle"], "notdecu");
    assert_eq!(rows[0]["score"], 95);
    assert_eq!(rows[0]["tier"], serde_json::to_value(Tier::Goat).unwrap());
    // Stale only past 48 hours.
    assert_eq!(row(&b, "decu")["stale"], true);
    assert_eq!(row(&b, "cooker")["stale"], false);
    assert_eq!(row(&b, "trunoest")["stale"], false);
    // The slot has no filler: nobody is in the network.
    assert!(rows.iter().all(|r| r["in_network"] == false));
    assert!(rows.iter().any(|r| r["top100"] == true));
    assert!(row(&b, "tekkerrss")["metrics"]["d30"].is_number());

    let stats = &b["stats"];
    assert_eq!(stats["tracked"], 122);
    let up = rows
        .iter()
        .filter(|r| r["metrics"]["d30"].as_f64().is_some_and(|v| v > 0.0))
        .count();
    assert_eq!(stats["up_30d"], up);
    assert!(up > 0 && up < 122);

    let kinds: Vec<&str> = b["ticker"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["top_score", "best_week", "traded_last_hour", "up_30d"]
    );
    assert_eq!(b["ticker"][0]["kol"], "decu");
    assert_eq!(b["ticker"][0]["value"], 95.0);
    assert_eq!(b["ticker"][3]["value"], up as f64);
    let best = rows
        .iter()
        .filter_map(|r| r["metrics"]["d7"].as_f64())
        .fold(f64::MIN, f64::max);
    assert_eq!(b["ticker"][1]["value"], best);

    let sources = b["sources"].as_array().unwrap();
    assert_eq!(
        sources[0]["source"],
        serde_json::to_value(MetricSource::Kolexplorer).unwrap()
    );
    let total: u64 = sources.iter().map(|x| x["kols"].as_u64().unwrap()).sum();
    assert_eq!(total, 122);
}

#[tokio::test]
async fn board_defaults_to_latest_and_checks_dates() {
    let s = setup(false);
    let (status, b) = get(&s, "/api/board").await;
    assert_eq!(status, 200);
    assert_eq!(b["date"], "2026-10-19");
    assert_eq!(b["rows"].as_array().unwrap().len(), 121);
    for (uri, want) in [
        ("/api/board?date=2026-10-18", 200),
        ("/api/board?date=", 200),
        ("/api/board?date=2026-10-17", 404),
        ("/api/board?date=2026-10-20", 404),
        ("/api/board?date=2026-10-1", 400),
        ("/api/board?date=yesterday", 400),
        ("/api/board?date=2026-10-18%27--", 400),
    ] {
        let (status, body) = get(&s, uri).await;
        assert_eq!(status, want, "{uri}: {body}");
        if want != 200 {
            assert!(body["error"].is_string());
        }
    }
}

#[tokio::test]
async fn closed_but_unranked_day_is_not_shown() {
    let s = build(false, false);
    let (status, b) = get(&s, "/api/board").await;
    assert_eq!(status, 200, "{b}");
    assert_eq!(b["date"], "2026-10-18");
    let (status, b) = get(&s, "/api/board?date=2026-10-19").await;
    assert_eq!(status, 404, "{b}");
}

#[tokio::test]
async fn in_network_comes_from_the_slot() {
    let s = setup(true);
    let (_, b) = get(&s, "/api/board?date=2026-10-18").await;
    let members: Vec<&str> = b["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["in_network"] == true)
        .map(|r| r["kol"].as_str().unwrap())
        .collect();
    assert_eq!(members, ["cooker"]);
    let (_, c) = get(&s, "/api/kol/cooker").await;
    assert_eq!(c["in_network"], true);
}

// ---------- /api/kol/:id ----------

#[tokio::test]
async fn kol_card_with_wallet_links() {
    let s = setup(false);
    let (status, c) = get(&s, "/api/kol/cooker?date=2026-10-18").await;
    assert_eq!(status, 200, "{c}");
    assert_eq!(c["kol"], "cooker");
    assert_eq!(c["name"], "Cooker");
    assert_eq!(c["date"], "2026-10-18");
    assert_eq!(
        (c["rank"].as_u64(), c["ranked"].as_u64()),
        (Some(2), Some(122))
    );
    assert_eq!(c["score"], 88);
    assert_eq!(c["in_network"], false);
    assert_eq!(c["stale"], false);
    assert!(c["metrics"]["d30"].is_number());
    assert_eq!(c["formats"], serde_json::json!(["post", "stream"]));
    assert_eq!(
        c["links"],
        serde_json::json!({
            "kolexplorer": "cooker", "solanatracker": true, "kolscan": false,
            "fomo": "cookerfomo", "pumpfun": null, "telegram": null
        })
    );
    assert_eq!(
        c["wallets"],
        serde_json::json!([
            {"address": MAIN, "main": true, "solscan_url": format!("https://solscan.io/account/{MAIN}")},
            {"address": OTHER, "main": false, "solscan_url": format!("https://solscan.io/account/{OTHER}")},
        ])
    );

    let (_, decu) = get(&s, "/api/kol/decu?date=2026-10-18").await;
    assert_eq!(decu["stale"], true);
    assert_eq!(decu["wallets"], serde_json::json!([]));
}

#[tokio::test]
async fn kol_card_checks_ids_and_dates() {
    let s = setup(false);
    let long = format!("/api/kol/{}", "a".repeat(65));
    for (uri, want) in [
        ("/api/kol/nobody-here", 404),
        (long.as_str(), 404),
        ("/api/kol/offboard", 404),
        // Off the latest board, still on the day-0 one.
        ("/api/kol/tekkerrss", 404),
        ("/api/kol/tekkerrss?date=2026-10-18", 200),
        ("/api/kol/decu?date=2026-09-01", 404),
        ("/api/kol/decu?date=20261018", 400),
    ] {
        let (status, body) = get(&s, uri).await;
        assert_eq!(status, want, "{uri}: {body}");
    }
}

// ---------- leak ----------

fn keys(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                out.insert(k.clone());
                keys(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| keys(x, out)),
        _ => {}
    }
}

fn assert_public(v: &Value, allow: &[&str]) {
    let mut found = BTreeSet::new();
    keys(v, &mut found);
    let extra: Vec<&String> = found
        .iter()
        .filter(|k| !allow.contains(&k.as_str()))
        .collect();
    assert!(extra.is_empty(), "keys outside the allowlist: {extra:?}");
    for k in &found {
        for bad in [
            "rate", "group", "note", "hypothes", "contact", "referral", "reach",
        ] {
            assert!(!k.contains(bad), "private key {k}");
        }
    }
}

const METRIC_KEYS: &[&str] = &[
    "d1",
    "d7",
    "d30",
    "wr7",
    "wr30",
    "wr_all",
    "avg_pos",
    "hold_s",
    "last_min_at_fetch",
    "unreal",
    "total",
    "vol30",
    "days30",
];

const BOARD_KEYS: &[&str] = &[
    "date",
    "closed_at",
    "dates",
    "stats",
    "sources",
    "ticker",
    "rows",
    "tracked",
    "up_30d",
    "traded_last_hour",
    "total_d30",
    "source",
    "fetched_at",
    "kols",
    "kind",
    "kol",
    "name",
    "value",
    "x_handle",
    "rank",
    "score",
    "tier",
    "badges",
    "top100",
    "in_network",
    "stale",
    "metrics",
];

const CARD_KEYS: &[&str] = &[
    "kol",
    "name",
    "x_handle",
    "date",
    "rank",
    "ranked",
    "top100",
    "in_network",
    "score",
    "tier",
    "parts",
    "badges",
    "source",
    "fetched_at",
    "stale",
    "metrics",
    "formats",
    "links",
    "wallets",
    "kind",
    "raw",
    "all_time",
    "share",
    "points",
    "status",
    "kolexplorer",
    "solanatracker",
    "kolscan",
    "fomo",
    "pumpfun",
    "telegram",
    "address",
    "main",
    "solscan_url",
];

fn with_metrics(keys: &[&'static str]) -> Vec<&'static str> {
    keys.iter().chain(METRIC_KEYS).copied().collect()
}

#[tokio::test]
async fn board_and_card_have_only_public_keys() {
    let s = setup(true);
    let (_, b) = get(&s, "/api/board?date=2026-10-18").await;
    assert_public(&b, &with_metrics(BOARD_KEYS));
    let (_, c) = get(&s, "/api/kol/cooker?date=2026-10-18").await;
    assert_public(&c, &with_metrics(CARD_KEYS));
}
