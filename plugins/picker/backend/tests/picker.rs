//! Picker acceptance: every filter, both exports, and no rate, group or note in the client
//! file. Stand-ins for roster, score and registry; invented KOLs.

use api_registry::{
    KolId, KolLinks, KolProfile, KolUpsert, Reach, ReachRecord, RegistryApi, RegistryError,
    WalletSource,
};
use api_roster::{NetworkMember, Rates, RosterApi, RosterError, XHandle};
use api_score::{Metrics, RankedKol, Ranking, ScoreApi, ScoreCard, ScoreError, Tier};
use arena_platform::{Config, Ctx, Person, Role, Tag, testing::TestDb};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::{Days, NaiveDate, Utc};
use plugin_picker::{Filters, PickList, build_plugin, service::Error};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;

const STAFF: Person = Person {
    id: Some(3),
    role: Role::Staff,
};

// ------------------------------------------------------------------ stand-ins

struct Member {
    kol: &'static str,
    name: &'static str,
    formats: &'static [&'static str],
    rates: (Option<f64>, Option<f64>),
}

const MEMBERS: [Member; 5] = [
    Member {
        kol: "ayla-moss",
        name: "Ayla Moss",
        formats: &["post", "thread"],
        rates: (Some(500.0), Some(1500.0)),
    },
    Member {
        kol: "brix-tane",
        name: "Brix Tane",
        formats: &["buy"],
        rates: (Some(2000.0), Some(4000.0)),
    },
    Member {
        kol: "cato-fell",
        name: "Cato, Fell",
        formats: &["call"],
        rates: (Some(300.0), None),
    },
    Member {
        kol: "dune-rook",
        name: "Dune Rook",
        formats: &["post"],
        rates: (None, None),
    },
    Member {
        kol: "eel-vance",
        name: "=Eel Vance",
        formats: &["space"],
        rates: (Some(100.0), Some(200.0)),
    },
];

#[derive(Default)]
struct FakeRoster {
    rate_calls: AtomicUsize,
}

#[async_trait]
impl RosterApi for FakeRoster {
    async fn members(&self, _person: &Person) -> Result<Vec<NetworkMember>, RosterError> {
        Ok(MEMBERS
            .iter()
            .map(|m| NetworkMember {
                kol: m.kol.into(),
                name: m.name.into(),
                x_handle: Some(format!("{}_x", m.kol.replace('-', "_"))),
                x_verified: false,
                fomo_handle: Some(m.kol.replace('-', "")),
                pumpfun: None,
                sheet_wallets: vec!["SheetWalletNeverShown1111111111111111111111".into()],
                formats: m.formats.iter().map(|f| f.to_string()).collect(),
                keep: false,
            })
            .collect())
    }

    async fn is_member(&self, _person: &Person, kol: &str) -> Result<bool, RosterError> {
        Ok(MEMBERS.iter().any(|m| m.kol == kol))
    }

    async fn verified_handle(
        &self,
        _person: &Person,
        _kol: &str,
    ) -> Result<Option<XHandle>, RosterError> {
        Ok(None)
    }

    async fn rates(&self, person: &Person, kols: &[String]) -> Result<Vec<Rates>, RosterError> {
        self.rate_calls.fetch_add(1, Ordering::SeqCst);
        if !person.is_staff() {
            return Err(RosterError::Forbidden);
        }
        Ok(MEMBERS
            .iter()
            .filter(|m| kols.iter().any(|k| k == m.kol))
            .map(|m| Rates {
                kol: m.kol.into(),
                low_usd: m.rates.0,
                high_usd: m.rates.1,
                formats: m.formats.iter().map(|f| f.to_string()).collect(),
            })
            .collect())
    }
}

/// Rankings by date; a date not listed is not ranked.
#[derive(Default)]
struct FakeScore {
    rankings: Mutex<BTreeMap<NaiveDate, Ranking>>,
}

fn card(score: u8, tier: Tier) -> ScoreCard {
    ScoreCard {
        score,
        tier,
        parts: Vec::new(),
        badges: Vec::new(),
    }
}

#[async_trait]
impl ScoreApi for FakeScore {
    async fn score(&self, _person: &Person, _metrics: &Metrics) -> Result<ScoreCard, ScoreError> {
        unimplemented!("the picker never scores")
    }

    async fn rank(&self, _person: &Person, _date: NaiveDate) -> Result<(), ScoreError> {
        unimplemented!("the picker never ranks")
    }

    async fn ranking(&self, _person: &Person, date: NaiveDate) -> Result<Ranking, ScoreError> {
        self.rankings
            .lock()
            .unwrap()
            .get(&date)
            .cloned()
            .ok_or(ScoreError::NotRanked { date })
    }
    async fn ranked_dates(&self, _: &Person) -> Result<Vec<NaiveDate>, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
}

#[derive(Default)]
struct FakeRegistry;

fn profile(m: &Member) -> KolProfile {
    KolProfile {
        id: m.kol.into(),
        name: m.name.into(),
        x_handle: Some(m.kol.replace('-', "")),
        links: KolLinks {
            kolexplorer: None,
            solanatracker: false,
            kolscan: false,
            fomo: Some(format!("{}_fomo", m.kol.split('-').next().unwrap())),
            pumpfun: None,
            telegram: None,
        },
        main_wallet: None,
        other_wallets: Vec::new(),
        reach: (m.kol == "ayla-moss").then(|| ReachRecord {
            reach: Reach {
                x_followers: Some(12_345),
                telegram_subscribers: None,
                pumpfun_followers: None,
            },
            source: "prototype".into(),
            date: NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
        }),
        formats: Vec::new(),
    }
}

#[async_trait]
impl RegistryApi for FakeRegistry {
    async fn upsert_kol(&self, _p: &Person, _k: &KolUpsert) -> Result<KolId, RegistryError> {
        unimplemented!()
    }
    async fn get_kol(&self, _p: &Person, _id: &str) -> Result<Option<KolProfile>, RegistryError> {
        unimplemented!()
    }
    async fn list_kols(
        &self,
        _p: &Person,
        ids: &[KolId],
    ) -> Result<Vec<KolProfile>, RegistryError> {
        // dune-rook has no registry profile: the roster's fields stand in
        Ok(ids
            .iter()
            .filter_map(|id| MEMBERS.iter().find(|m| m.kol == id && m.kol != "dune-rook"))
            .map(profile)
            .collect())
    }
    async fn find_by_handle(&self, _p: &Person, _x: &str) -> Result<Option<KolId>, RegistryError> {
        unimplemented!()
    }
    async fn find_by_wallet(&self, _p: &Person, _a: &str) -> Result<Option<KolId>, RegistryError> {
        unimplemented!()
    }
    async fn publish_wallets(
        &self,
        _p: &Person,
        _kol: &str,
        _main: Option<(&str, WalletSource)>,
        _others: &[(String, WalletSource)],
    ) -> Result<(), RegistryError> {
        unimplemented!()
    }
    async fn set_reach(
        &self,
        _p: &Person,
        _kol: &str,
        _reach: &Reach,
        _source: &str,
        _date: NaiveDate,
    ) -> Result<(), RegistryError> {
        unimplemented!()
    }
    async fn set_formats(
        &self,
        _p: &Person,
        _kol: &str,
        _formats: &[String],
    ) -> Result<(), RegistryError> {
        unimplemented!()
    }
}

// ------------------------------------------------------------------ harness

struct Env {
    _db: TestDb,
    ctx: Ctx,
    roster: Arc<FakeRoster>,
    score: Arc<FakeScore>,
    app: Router,
}

fn today() -> NaiveDate {
    Utc::now().date_naive()
}

/// Ranked two days ago: ayla 1, brix 2, cato 3, dune 4; eel is not ranked.
fn ranked_day() -> NaiveDate {
    today().checked_sub_days(Days::new(2)).unwrap()
}

async fn env() -> Env {
    let db = TestDb::with_plugins(&[("picker", &plugin_picker::MIGRATOR)]).await;
    let ctx = Ctx::new(
        db.pool().clone(),
        Config::from_lookup(|_| None, None).unwrap(),
        "test",
    )
    .unwrap();
    let roster = Arc::new(FakeRoster::default());
    let score = Arc::new(FakeScore::default());
    let rows = [
        ("ayla-moss", 1, 82, Tier::Goat),
        ("brix-tane", 2, 61, Tier::Diamond),
        ("cato-fell", 3, 40, Tier::Sendoor),
        ("dune-rook", 4, 20, Tier::Degen),
    ]
    .into_iter()
    .map(|(kol, rank, s, t)| RankedKol {
        kol: kol.into(),
        rank,
        card: card(s, t),
    })
    .collect();
    score.rankings.lock().unwrap().insert(
        ranked_day(),
        Ranking {
            date: ranked_day(),
            ranked_at: Utc::now(),
            rows,
        },
    );
    ctx.apis
        .provide::<dyn RosterApi>(
            "roster",
            vec![("default", roster.clone() as Arc<dyn RosterApi>)],
            Arc::new(api_roster::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn ScoreApi>(
            "score",
            vec![("default", score.clone() as Arc<dyn ScoreApi>)],
            Arc::new(api_score::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn RegistryApi>(
            "registry",
            vec![("default", Arc::new(FakeRegistry) as Arc<dyn RegistryApi>)],
            Arc::new(api_registry::stub::Stub),
        )
        .unwrap();
    let plugin = build_plugin(&ctx).unwrap();
    let mut app = Router::default();
    for route in arena_platform::Plugin::routes(plugin.as_ref(), &ctx) {
        assert_eq!(route.tag, Tag::Staff, "{} must be staff-only", route.path);
        app = app.route(&route.path, route.handler);
    }
    Env {
        _db: db,
        ctx,
        roster,
        score,
        app,
    }
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: String,
}

async fn get(app: &Router, who: Person, uri: &str) -> Reply {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .extension(who)
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    Reply {
        status,
        headers,
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

impl Env {
    async fn list(&self, query: &str) -> PickList {
        let r = get(&self.app, STAFF, &format!("/api/team/picker{query}")).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.body);
        serde_json::from_str(&r.body).unwrap()
    }

    async fn kols(&self, query: &str) -> Vec<String> {
        self.list(query)
            .await
            .rows
            .into_iter()
            .map(|r| r.public.kol)
            .collect()
    }

    async fn error(&self, uri: &str) -> (StatusCode, String) {
        let r = get(&self.app, STAFF, uri).await;
        let v: Value = serde_json::from_str(&r.body).unwrap();
        (r.status, v["error"].as_str().unwrap().to_string())
    }
}

// ------------------------------------------------------------------ list and filters

#[tokio::test]
async fn the_list_holds_every_member_ranked_first_with_rates() {
    let e = env().await;
    let l = e.list("").await;
    assert_eq!(l.date, Some(ranked_day()));
    assert_eq!(l.no_ranking, None);
    assert_eq!(l.total, 5);
    let got: Vec<_> = l
        .rows
        .iter()
        .map(|r| {
            (
                r.public.kol.as_str(),
                r.public.rank,
                r.public.score,
                r.low_usd,
                r.high_usd,
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("ayla-moss", Some(1), Some(82), Some(500.0), Some(1500.0)),
            ("brix-tane", Some(2), Some(61), Some(2000.0), Some(4000.0)),
            ("cato-fell", Some(3), Some(40), Some(300.0), None),
            ("dune-rook", Some(4), Some(20), None, None),
            ("eel-vance", None, None, Some(100.0), Some(200.0)),
        ]
    );
    // profile fields from the registry; the roster's stand in when there is no profile
    let ayla = &l.rows[0].public;
    assert_eq!(
        (
            ayla.x_handle.as_deref(),
            ayla.fomo.as_deref(),
            ayla.x_followers
        ),
        (Some("aylamoss"), Some("ayla_fomo"), Some(12_345))
    );
    let dune = &l.rows[3].public;
    assert_eq!(
        (dune.x_handle.as_deref(), dune.fomo.as_deref()),
        (Some("dune_rook_x"), Some("dunerook"))
    );
    assert_eq!(
        l.formats,
        ["post", "buy", "thread", "stream", "space", "call"]
    );
    // the sheet's wallet candidates never appear
    let raw = get(&e.app, STAFF, "/api/team/picker").await.body;
    assert!(!raw.contains("SheetWallet"), "{raw}");
}

#[tokio::test]
async fn the_score_filter_is_inclusive_and_leaves_out_kols_without_score() {
    let e = env().await;
    assert_eq!(
        e.kols("?score_min=40&score_max=61").await,
        ["brix-tane", "cato-fell"]
    );
    assert_eq!(e.kols("?score_min=70").await, ["ayla-moss"]);
    assert_eq!(e.kols("?score_max=40").await, ["cato-fell", "dune-rook"]);
    assert_eq!(
        e.error("/api/team/picker?score_min=lots").await,
        (StatusCode::BAD_REQUEST, "score from: not a number".into())
    );
}

#[tokio::test]
async fn the_tier_filter_keeps_one_tier() {
    let e = env().await;
    assert_eq!(e.kols("?tier=diamond").await, ["brix-tane"]);
    assert_eq!(e.kols("?tier=degen").await, ["dune-rook"]);
    assert_eq!(
        e.error("/api/team/picker?tier=platinum").await,
        (StatusCode::BAD_REQUEST, "unknown tier platinum".into())
    );
}

#[tokio::test]
async fn the_rate_filter_keeps_ranges_that_overlap_and_leaves_out_kols_without_rates() {
    let e = env().await;
    // ayla 500–1500 and brix 2000–4000 overlap 1000–2500
    assert_eq!(
        e.kols("?rate_min=1000&rate_max=2500").await,
        ["ayla-moss", "brix-tane"]
    );
    // cato has only a low rate (300); eel 100–200
    assert_eq!(e.kols("?rate_max=350").await, ["cato-fell", "eel-vance"]);
    assert_eq!(e.kols("?rate_min=3500").await, ["brix-tane"]);
    assert_eq!(
        e.kols("?rate_min=1,000&rate_max=$2,500").await,
        ["ayla-moss", "brix-tane"]
    );
    assert_eq!(
        e.error("/api/team/picker?rate_max=-5").await,
        (StatusCode::BAD_REQUEST, "rate to: not a number".into())
    );
}

#[tokio::test]
async fn the_format_filter_keeps_kols_that_do_it() {
    let e = env().await;
    assert_eq!(e.kols("?format=post").await, ["ayla-moss", "dune-rook"]);
    assert_eq!(e.kols("?format=space").await, ["eel-vance"]);
    assert_eq!(e.kols("?format=stream").await, Vec::<String>::new());
    assert_eq!(
        e.error("/api/team/picker?format=dance").await,
        (StatusCode::BAD_REQUEST, "unknown format dance".into())
    );
    // filters combine
    assert_eq!(
        e.kols("?format=post&score_min=50&rate_max=1000&tier=goat")
            .await,
        ["ayla-moss"]
    );
}

#[tokio::test]
async fn without_a_ranking_scores_are_missing_with_the_reason() {
    let e = env().await;
    e.score.rankings.lock().unwrap().clear();
    let l = e.list("").await;
    assert_eq!(l.date, None);
    assert_eq!(
        l.no_ranking.as_deref(),
        Some("no ranking in the last 14 days")
    );
    assert!(l.rows.iter().all(|r| r.public.score.is_none()));
    assert_eq!(e.kols("?score_min=0").await, Vec::<String>::new());

    // a given date is used as is
    let other = today().checked_sub_days(Days::new(30)).unwrap();
    let l = e.list(&format!("?date={other}")).await;
    assert_eq!(
        l.no_ranking,
        Some(format!("snapshot {other} is not ranked"))
    );
    assert_eq!(
        e.error("/api/team/picker?date=yesterday").await,
        (
            StatusCode::BAD_REQUEST,
            "date: yesterday is not YYYY-MM-DD".into()
        )
    );
}

// ------------------------------------------------------------------ exports

#[tokio::test]
async fn the_client_export_holds_public_fields_only() {
    let e = env().await;
    let r = get(
        &e.app,
        STAFF,
        "/api/team/picker/export?kols=eel-vance,ayla-moss,cato-fell,not-ours",
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.headers[header::CONTENT_TYPE], "text/csv; charset=utf-8");
    assert_eq!(
        r.headers[header::CONTENT_DISPOSITION],
        format!("attachment; filename=\"picker-{}.csv\"", ranked_day()).as_str()
    );
    let d = ranked_day();
    assert_eq!(
        r.body,
        format!(
            "Name,X,FOMO,X followers,Score,Tier,Rank,Formats,Snapshot\r\n\
             Ayla Moss,https://x.com/aylamoss,ayla_fomo,12345,82,GOAT,1,post thread,{d}\r\n\
             \"Cato, Fell\",https://x.com/catofell,cato_fomo,—,40,Sendoor,3,call,{d}\r\n\
             '=Eel Vance,https://x.com/eelvance,eel_fomo,—,—,—,—,space,{d}\r\n"
        )
    );
    // the rates were not even read, and nothing team-only is in the file
    assert_eq!(e.roster.rate_calls.load(Ordering::SeqCst), 0);
    let lower = r.body.to_lowercase();
    for word in [
        "low $",
        "high $",
        "rate",
        "group",
        "note",
        "sheetwallet",
        "1500",
        "200,",
    ] {
        assert!(!lower.contains(word), "{word} in {}", r.body);
    }
}

#[tokio::test]
async fn the_export_with_rates_adds_low_and_high() {
    let e = env().await;
    let r = get(
        &e.app,
        STAFF,
        "/api/team/picker/export?kols=brix-tane,dune-rook&rates=1",
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        r.headers[header::CONTENT_DISPOSITION],
        format!(
            "attachment; filename=\"picker-with-rates-{}.csv\"",
            ranked_day()
        )
        .as_str()
    );
    let d = ranked_day();
    assert_eq!(
        r.body,
        format!(
            "Name,X,FOMO,X followers,Score,Tier,Rank,Formats,Snapshot,Low $,High $\r\n\
             Brix Tane,https://x.com/brixtane,brix_fomo,—,61,Diamond,2,buy,{d},2000,4000\r\n\
             Dune Rook,https://x.com/dune_rook_x,dunerook,—,20,Degen,4,post,{d},—,—\r\n"
        )
    );
    assert_eq!(e.roster.rate_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn exports_need_checked_network_kols_and_staff() {
    let e = env().await;
    assert_eq!(
        e.error("/api/team/picker/export").await,
        (StatusCode::BAD_REQUEST, "check at least one KOL".into())
    );
    assert_eq!(
        e.error("/api/team/picker/export?kols=,%20").await,
        (StatusCode::BAD_REQUEST, "check at least one KOL".into())
    );
    assert_eq!(
        e.error("/api/team/picker/export?kols=not-ours").await,
        (
            StatusCode::BAD_REQUEST,
            "none of these KOLs is in the network".into()
        )
    );
    for uri in [
        "/api/team/picker",
        "/api/team/picker/export?kols=ayla-moss",
        "/api/team/picker/export?kols=ayla-moss&rates=1",
    ] {
        let r = get(&e.app, Person::NOBODY, uri).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{uri}");
        assert!(!r.body.contains("Ayla"), "{uri}");
    }
    assert_eq!(e.roster.rate_calls.load(Ordering::SeqCst), 0);

    // the service checks the person too
    let picker = build_plugin(&e.ctx).unwrap().picker();
    assert_eq!(
        picker
            .list(&Person::NOBODY, &Filters::default(), today())
            .await
            .unwrap_err(),
        Error::Forbidden
    );
    assert_eq!(
        picker
            .export(&Person::NOBODY, &["ayla-moss".into()], true, &None, today())
            .await
            .unwrap_err(),
        Error::Forbidden
    );
}
