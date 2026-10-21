//! FOMO match acceptance: a fake local Solana node, stand-in Wallets and Journal.
//! Invented mints, wallets and KOLs.

#[path = "../../tests/fake_site.rs"]
mod fake_site;

use api_journal::{Entry, EntryId, JournalApi, JournalError};
use api_wallets::{
    Candidate, CandidateSource, CandidateStatus, KolWalletStatus, WalletStatus, WalletsApi,
    WalletsError,
};
use arena_platform::{Config, Ctx, Person, Role, Tag, testing::TestDb};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use fake_site::{FakeNode, Mode, TOKEN, TOKEN_2022};
use plugin_fomo::{
    MatchRequest, MatchResult, Options, Outcome, base58, build_plugin, service::PositionInput,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tower::ServiceExt;

const STAFF: Person = Person {
    id: Some(4),
    role: Role::Staff,
};
const KOL: &str = "wren-tallis";

// ------------------------------------------------------------------ stand-ins

#[derive(Default)]
struct FakeWallets {
    members: Mutex<BTreeSet<String>>,
    candidates: Mutex<BTreeMap<(String, String), Candidate>>,
    proposals: Mutex<Vec<(String, String, String)>>,
    busy: AtomicBool,
}

fn candidate(
    id: i64,
    kol: &str,
    address: &str,
    status: CandidateStatus,
    source: CandidateSource,
) -> Candidate {
    let now = chrono::Utc::now();
    Candidate {
        id,
        kol: kol.into(),
        address: address.into(),
        status,
        source,
        evidence: None,
        sent_by: None,
        confirmation: None,
        reason: None,
        main: false,
        created_at: now,
        updated_at: now,
    }
}

#[async_trait]
impl WalletsApi for FakeWallets {
    async fn propose(
        &self,
        _person: &Person,
        kol: &str,
        address: &str,
        evidence: &str,
    ) -> Result<Candidate, WalletsError> {
        if self.busy.load(Ordering::SeqCst) {
            return Err(WalletsError::Busy);
        }
        if !self.members.lock().unwrap().contains(kol) {
            return Err(WalletsError::NotMember { kol: kol.into() });
        }
        self.proposals
            .lock()
            .unwrap()
            .push((kol.into(), address.into(), evidence.into()));
        let mut all = self.candidates.lock().unwrap();
        let next = all.len() as i64 + 1;
        Ok(all
            .entry((kol.into(), address.into()))
            .or_insert_with(|| {
                candidate(
                    next,
                    kol,
                    address,
                    CandidateStatus::Hypothesis,
                    CandidateSource::Fomo,
                )
            })
            .clone())
    }

    async fn status(&self, _person: &Person, kol: &str) -> Result<WalletStatus, WalletsError> {
        if !self.members.lock().unwrap().contains(kol) {
            return Err(WalletsError::NotMember { kol: kol.into() });
        }
        Ok(WalletStatus {
            kol: kol.into(),
            status: KolWalletStatus::NotFound,
            main_address: None,
            last_trade_at: None,
        })
    }
}

#[derive(Default)]
struct FakeJournal {
    entries: Mutex<Vec<Entry>>,
    fail: AtomicBool,
}

#[async_trait]
impl JournalApi for FakeJournal {
    async fn record(
        &self,
        person: &Person,
        block: &str,
        object: &str,
        action: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) -> Result<EntryId, JournalError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(JournalError::Internal {
                message: "journal down".into(),
            });
        }
        let mut entries = self.entries.lock().unwrap();
        let id = entries.len() as i64 + 1;
        entries.push(Entry {
            id,
            at: chrono::Utc::now(),
            person_id: person.id,
            role: person.role,
            block: block.into(),
            object: object.into(),
            action: action.into(),
            before,
            after,
        });
        Ok(id)
    }

    async fn history(
        &self,
        _person: &Person,
        block: &str,
        object: &str,
    ) -> Result<Vec<Entry>, JournalError> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.block == block && e.object == object)
            .cloned()
            .collect())
    }
}

// ------------------------------------------------------------------ harness

struct Env {
    _db: TestDb,
    node: FakeNode,
    wallets: Arc<FakeWallets>,
    journal: Arc<FakeJournal>,
    app: Router,
}

async fn env_with(rpc_url: Option<&str>, opts: Options) -> Env {
    let node = FakeNode::start().await;
    let url = rpc_url.map(|u| u.replace("{node}", &node.url));
    let db = TestDb::with_plugins(&[("fomo", &plugin_fomo::MIGRATOR)]).await;
    let config = Config::from_lookup(
        |name| (name == "PUBLIC_RPC_URL").then(|| url.clone()).flatten(),
        None,
    )
    .unwrap();
    let ctx = Ctx::new(db.pool().clone(), config, "test").unwrap();
    let wallets = Arc::new(FakeWallets::default());
    wallets.members.lock().unwrap().insert(KOL.into());
    let journal = Arc::new(FakeJournal::default());
    ctx.apis
        .provide::<dyn WalletsApi>(
            "wallets",
            vec![("default", wallets.clone() as Arc<dyn WalletsApi>)],
            Arc::new(api_wallets::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn JournalApi>(
            "journal",
            vec![("default", journal.clone() as Arc<dyn JournalApi>)],
            Arc::new(api_journal::stub::Stub),
        )
        .unwrap();
    let plugin = build_plugin(&ctx, opts).unwrap();
    let mut app = Router::default();
    for route in arena_platform::Plugin::routes(plugin.as_ref(), &ctx) {
        assert_eq!(route.tag, Tag::Staff, "{} must be staff-only", route.path);
        app = app.route(&route.path, route.handler);
    }
    Env {
        _db: db,
        node,
        wallets,
        journal,
        app,
    }
}

async fn env() -> Env {
    env_with(Some("{node}"), Options::default()).await
}

/// An invented 32-byte key and its address.
fn key(seed: u8) -> [u8; 32] {
    let mut k = [seed; 32];
    k[0] = 0x5a;
    k[31] = seed.wrapping_mul(7);
    k
}
fn addr(seed: u8) -> String {
    base58::encode(&key(seed))
}

const W_KOL: u8 = 10;
const W_OTHER: u8 = 11;
const W_THIRD: u8 = 12;

fn mint(n: u8) -> String {
    addr(100 + n)
}

async fn send(app: &Router, who: Person, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/api/team/fomo/match")
        .extension(who)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, v)
}

fn body(positions: &[(&str, &str)]) -> Value {
    json!({
        "kol": KOL,
        "positions": positions.iter().map(|(m, a)| json!({ "mint": m, "amount": a })).collect::<Vec<_>>(),
    })
}

impl Env {
    async fn run(&self, positions: &[(&str, &str)]) -> MatchResult {
        let (status, v) = send(&self.app, STAFF, body(positions)).await;
        assert_eq!(status, StatusCode::OK, "{v}");
        serde_json::from_value(v).unwrap()
    }

    fn journaled(&self) -> Vec<Value> {
        self.journal
            .entries
            .lock()
            .unwrap()
            .iter()
            .map(|e| {
                assert_eq!(
                    (e.block.as_str(), e.object.as_str(), e.action.as_str()),
                    ("fomo", KOL, "match")
                );
                assert_eq!(e.person_id, STAFF.id);
                e.after.clone().unwrap()
            })
            .collect()
    }

    fn proposals(&self) -> Vec<(String, String, String)> {
        self.wallets.proposals.lock().unwrap().clone()
    }

    /// Nothing proposed, the run journaled with `outcome`, and the node URL nowhere.
    fn failed_with(&self, r: &MatchResult, outcome: Outcome) {
        assert_eq!(r.outcome, outcome, "{r:?}");
        assert!(r.candidate.is_none());
        assert!(self.proposals().is_empty());
        let journaled = self.journaled();
        assert_eq!(journaled.len(), 1);
        assert_eq!(journaled[0]["outcome"], json!(outcome));
        let all = format!("{r:?} {journaled:?}");
        assert!(!all.contains("node-secret"), "{all}");
        assert!(!all.contains("127.0.0.1"), "{all}");
    }
}

/// 6 decimals: `units` whole tokens as a raw amount.
fn raw6(units: f64) -> u64 {
    (units * 1e6).round() as u64
}

// ------------------------------------------------------------------ tests

#[tokio::test]
async fn one_wallet_holding_every_position_becomes_a_hypothesis() {
    let e = env().await;
    let (m1, m2, m3) = (mint(1), mint(2), mint(3));
    e.node.token(
        &m1,
        TOKEN,
        6,
        &[
            (key(W_KOL), raw6(12_345.5)),
            (key(W_OTHER), raw6(12_345.5)),
            (key(W_THIRD), raw6(3.0)),
        ],
    );
    e.node.token(
        &m2,
        TOKEN,
        9,
        &[(key(W_KOL), 1_500_000_000_000), (key(W_THIRD), 1)],
    );
    e.node
        .token(&m3, TOKEN, 0, &[(key(W_KOL), 2_000_000), (key(W_OTHER), 7)]);

    let r = e
        .run(&[(&m1, "12,345.5"), (&m2, "1.5K"), (&m3, "2M")])
        .await;
    assert_eq!(r.outcome, Outcome::Hypothesis, "{r:?}");
    let c = r.candidate.clone().unwrap();
    assert_eq!(
        (c.address.as_str(), c.status, c.source),
        (
            addr(W_KOL).as_str(),
            CandidateStatus::Hypothesis,
            CandidateSource::Fomo
        )
    );
    assert_eq!(r.wallet_count, 1);
    assert_eq!(r.wallets[0].positions, vec![0, 1, 2]);
    // per position: decimals, holders, matches
    let p: Vec<_> = r
        .positions
        .iter()
        .map(|p| (p.decimals, p.holders, p.match_count))
        .collect();
    assert_eq!(
        p,
        vec![
            (Some(6), Some(3), 2),
            (Some(9), Some(2), 1),
            (Some(0), Some(2), 1)
        ]
    );
    assert_eq!(r.positions[0].matches, {
        let mut v = vec![addr(W_KOL), addr(W_OTHER)];
        v.sort();
        v
    });

    // proposed once, with the positions as evidence
    let props = e.proposals();
    assert_eq!(props.len(), 1);
    assert_eq!(
        (props[0].0.as_str(), props[0].1.as_str()),
        (KOL, addr(W_KOL).as_str())
    );
    assert!(
        props[0].2.starts_with("FOMO match, 3 of 3 positions"),
        "{}",
        props[0].2
    );
    assert!(props[0].2.contains(&m2));

    // journaled once
    let j = e.journaled();
    assert_eq!(j.len(), 1);
    assert_eq!(j[0]["outcome"], "hypothesis");
    assert_eq!(j[0]["candidate"]["address"], json!(addr(W_KOL)));

    // the calls: per position the supply, then the token program with a size filter,
    // then Token-2022 without one; owner and amount through a 40-byte slice
    let calls = e.node.calls();
    let methods: Vec<_> = calls
        .iter()
        .map(|c| c["method"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        methods,
        ["getTokenSupply", "getProgramAccounts", "getProgramAccounts"].repeat(3)
    );
    assert_eq!(calls[0]["params"], json!([m1]));
    assert_eq!(
        calls[1]["params"],
        json!([TOKEN, {
            "encoding": "base64",
            "dataSlice": { "offset": 32, "length": 40 },
            "filters": [{ "dataSize": 165 }, { "memcmp": { "offset": 0, "bytes": m1 } }]
        }])
    );
    assert_eq!(
        calls[2]["params"],
        json!([TOKEN_2022, {
            "encoding": "base64",
            "dataSlice": { "offset": 32, "length": 40 },
            "filters": [{ "memcmp": { "offset": 0, "bytes": m1 } }]
        }])
    );
}

#[tokio::test]
async fn owners_are_intersected_across_positions() {
    let e = env().await;
    let (m1, m2, m3) = (mint(1), mint(2), mint(3));
    // W_KOL holds 1 and 3; W_OTHER holds only 2; W_THIRD only 1
    e.node.token(
        &m1,
        TOKEN,
        6,
        &[(key(W_KOL), raw6(50.0)), (key(W_THIRD), raw6(50.0))],
    );
    e.node.token(
        &m2,
        TOKEN,
        6,
        &[(key(W_OTHER), raw6(70.0)), (key(W_KOL), raw6(1.0))],
    );
    e.node.token(&m3, TOKEN, 6, &[(key(W_KOL), raw6(90.0))]);
    let r = e.run(&[(&m1, "50"), (&m2, "70"), (&m3, "90")]).await;
    assert_eq!(r.outcome, Outcome::Hypothesis);
    assert_eq!(r.candidate.unwrap().address, addr(W_KOL));
    assert_eq!(r.wallets[0].positions, vec![0, 2]);
    assert!(
        e.proposals()[0]
            .2
            .starts_with("FOMO match, 2 of 3 positions")
    );
}

#[tokio::test]
async fn no_wallet_in_two_positions_is_no_match() {
    let e = env().await;
    let (m1, m2) = (mint(1), mint(2));
    e.node.token(&m1, TOKEN, 6, &[(key(W_KOL), raw6(50.0))]);
    e.node.token(&m2, TOKEN, 6, &[(key(W_OTHER), raw6(70.0))]);
    let r = e.run(&[(&m1, "50"), (&m2, "70")]).await;
    assert_eq!(
        (r.positions[0].match_count, r.positions[1].match_count),
        (1, 1)
    );
    assert_eq!(r.wallet_count, 0);
    e.failed_with(&r, Outcome::NoMatch);
}

#[tokio::test]
async fn two_wallets_holding_the_positions_are_several_and_nothing_is_proposed() {
    let e = env().await;
    let (m1, m2) = (mint(1), mint(2));
    e.node.token(
        &m1,
        TOKEN,
        6,
        &[(key(W_KOL), raw6(50.0)), (key(W_OTHER), raw6(50.1))],
    );
    e.node.token(
        &m2,
        TOKEN,
        6,
        &[(key(W_KOL), raw6(70.0)), (key(W_OTHER), raw6(69.9))],
    );
    let r = e.run(&[(&m1, "50"), (&m2, "70")]).await;
    assert_eq!(r.wallet_count, 2);
    assert_eq!(r.wallets.len(), 2);
    e.failed_with(&r, Outcome::Several);
}

#[tokio::test]
async fn a_holding_matches_within_half_a_percent() {
    let e = env().await;
    let (m1, m2) = (mint(1), mint(2));
    let (a, b, c, d) = (key(1), key(2), key(3), key(4));
    e.node.token(
        &m1,
        TOKEN,
        6,
        &[
            (a, raw6(1004.9)),
            (b, raw6(995.1)),
            (c, raw6(1005.1)),
            (d, raw6(994.9)),
        ],
    );
    e.node.token(&m2, TOKEN, 6, &[(key(W_KOL), raw6(1.0))]);
    let r = e.run(&[(&m1, "1,000"), (&m2, "1")]).await;
    let mut inside = vec![base58::encode(&a), base58::encode(&b)];
    inside.sort();
    assert_eq!(r.positions[0].matches, inside);
    assert_eq!(r.positions[0].holders, Some(4));
    assert_eq!(r.outcome, Outcome::NoMatch);
}

#[tokio::test]
async fn token_2022_mints_are_read_without_a_size_filter() {
    let e = env().await;
    let (m1, m2) = (mint(1), mint(2));
    e.node
        .token(&m1, TOKEN_2022, 6, &[(key(W_KOL), raw6(321.0))]);
    e.node.token(&m2, TOKEN, 6, &[(key(W_KOL), raw6(77.0))]);
    let r = e.run(&[(&m1, "321"), (&m2, "77")]).await;
    assert_eq!(r.outcome, Outcome::Hypothesis, "{r:?}");
    assert_eq!(r.positions[0].holders, Some(1));
    assert_eq!(r.candidate.unwrap().address, addr(W_KOL));
}

#[tokio::test]
async fn an_address_the_kol_already_has_is_known_not_a_new_hypothesis() {
    let e = env().await;
    e.wallets.candidates.lock().unwrap().insert(
        (KOL.into(), addr(W_KOL)),
        candidate(
            9,
            KOL,
            &addr(W_KOL),
            CandidateStatus::Confirmed,
            CandidateSource::Sheet,
        ),
    );
    let (m1, m2) = (mint(1), mint(2));
    e.node.token(&m1, TOKEN, 6, &[(key(W_KOL), raw6(5.0))]);
    e.node.token(&m2, TOKEN, 6, &[(key(W_KOL), raw6(6.0))]);
    let r = e.run(&[(&m1, "5"), (&m2, "6")]).await;
    assert_eq!(r.outcome, Outcome::Known);
    assert_eq!(r.candidate.unwrap().status, CandidateStatus::Confirmed);
}

#[tokio::test]
async fn a_busy_wallets_saves_nothing_and_says_so() {
    let e = env().await;
    e.wallets.busy.store(true, Ordering::SeqCst);
    let (m1, m2) = (mint(1), mint(2));
    e.node.token(&m1, TOKEN, 6, &[(key(W_KOL), raw6(5.0))]);
    e.node.token(&m2, TOKEN, 6, &[(key(W_KOL), raw6(6.0))]);
    let r = e.run(&[(&m1, "5"), (&m2, "6")]).await;
    e.failed_with(&r, Outcome::Busy);
}

// ------------------------------------------------------------------ error lines

/// Positions 1 and 2 are fine; the failure is on position 2, so position 3 is never read.
async fn failing(mode: Mode, method: &str, opts: Options) -> (Env, MatchResult) {
    let e = env_with(Some("{node}"), opts).await;
    let (m1, m2, m3) = (mint(1), mint(2), mint(3));
    for m in [&m1, &m2, &m3] {
        e.node.token(m, TOKEN, 6, &[(key(W_KOL), raw6(5.0))]);
    }
    e.node.mode(&m2, method, mode);
    let r = e.run(&[(&m1, "5"), (&m2, "5"), (&m3, "5")]).await;
    assert_eq!(r.positions[0].error, None);
    assert!(r.positions[1].error.is_some());
    assert_eq!(r.positions[2].holders, None, "stops at the first failure");
    assert!(
        e.node.calls().iter().all(|c| !c.to_string().contains(&m3)),
        "position 3 was read"
    );
    (e, r)
}

#[tokio::test]
async fn an_rpc_error_is_node_down() {
    let (e, r) = failing(
        Mode::RpcError(-32603, "Internal error".into()),
        "getProgramAccounts",
        Options::default(),
    )
    .await;
    e.failed_with(&r, Outcome::NodeDown);
    assert!(r.detail.unwrap().contains("Internal error"));
}

#[tokio::test]
async fn an_http_error_is_node_down() {
    let (e, r) = failing(Mode::Http(502), "getTokenSupply", Options::default()).await;
    e.failed_with(&r, Outcome::NodeDown);
    assert!(r.detail.unwrap().contains("HTTP 502"));
}

#[tokio::test]
async fn a_timeout_is_node_down() {
    let opts = Options {
        timeout: Duration::from_millis(300),
        ..Options::default()
    };
    let (e, r) = failing(
        Mode::Slow(Duration::from_secs(3)),
        "getProgramAccounts",
        opts,
    )
    .await;
    e.failed_with(&r, Outcome::NodeDown);
    assert!(r.detail.unwrap().contains("timed out"));
}

#[tokio::test]
async fn a_node_that_does_not_listen_is_node_down_and_its_url_is_not_shown() {
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap()
    };
    let e = env_with(
        Some(&format!("http://{closed}/?api-key=node-secret-7f3a")),
        Options::default(),
    )
    .await;
    let r = e.run(&[(&mint(1), "5"), (&mint(2), "5")]).await;
    e.failed_with(&r, Outcome::NodeDown);
    assert_eq!(r.positions[0].error, Some(Outcome::NodeDown));
}

#[tokio::test]
async fn a_scan_limit_error_is_too_many_holders() {
    let (e, r) = failing(
        Mode::RpcError(
            -32010,
            "scan aborted: The accumulated scan results exceeded the limit".into(),
        ),
        "getProgramAccounts",
        Options::default(),
    )
    .await;
    e.failed_with(&r, Outcome::TooManyHolders);
}

#[tokio::test]
async fn an_answer_over_64_mb_is_too_many_holders() {
    let (e, r) = failing(
        Mode::Oversize(65 << 20),
        "getProgramAccounts",
        Options::default(),
    )
    .await;
    e.failed_with(&r, Outcome::TooManyHolders);
    assert!(r.detail.unwrap().contains("over 64 MB"));
}

#[tokio::test]
async fn an_answer_just_under_the_cap_is_read() {
    let opts = Options {
        max_body: 1 << 20,
        ..Options::default()
    };
    let e = env_with(Some("{node}"), opts).await;
    let (m1, m2) = (mint(1), mint(2));
    e.node.token(&m1, TOKEN, 6, &[(key(W_KOL), raw6(5.0))]);
    e.node.token(&m2, TOKEN, 6, &[(key(W_KOL), raw6(5.0))]);
    e.node
        .mode(&m1, "getProgramAccounts", Mode::Oversize((1 << 20) - 200));
    e.node
        .mode(&m2, "getProgramAccounts", Mode::Oversize(1 << 20));
    let r = e.run(&[(&m1, "5"), (&m2, "5")]).await;
    assert_eq!(r.positions[0].error, None);
    assert_eq!(r.positions[1].error, Some(Outcome::TooManyHolders));
    e.failed_with(&r, Outcome::TooManyHolders);
}

#[tokio::test]
async fn without_a_node_url_nothing_is_called() {
    let e = env_with(None, Options::default()).await;
    let r = e.run(&[(&mint(1), "5"), (&mint(2), "5")]).await;
    e.failed_with(&r, Outcome::NotConfigured);
    assert_eq!(r.detail.as_deref(), Some("PUBLIC_RPC_URL not configured"));
    assert!(e.node.calls().is_empty());
}

// ------------------------------------------------------------------ route

#[tokio::test]
async fn the_route_is_staff_only_and_checks_the_request_first() {
    let e = env().await;
    let ok = body(&[(&mint(1), "5"), (&mint(2), "5")]);

    let (status, v) = send(&e.app, Person::NOBODY, ok.clone()).await;
    assert_eq!(
        (status, v["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("forbidden"))
    );

    let (status, v) = send(&e.app, STAFF, json!({ "kol": KOL })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    let (status, v) = send(&e.app, STAFF, body(&[(&mint(1), "5")])).await;
    assert_eq!(
        (status, v["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("enter 2 or 3 positions"))
    );
    let (status, v) = send(
        &e.app,
        STAFF,
        json!({ "kol": "nobody-known", "positions": ok["positions"] }),
    )
    .await;
    assert_eq!(
        (status, v["error"].as_str()),
        (
            StatusCode::NOT_FOUND,
            Some("nobody-known is not in the network")
        )
    );
    // none of these reached the node or the journal
    assert!(e.node.calls().is_empty());
    assert!(e.journal.entries.lock().unwrap().is_empty());

    // a run whose journal fails answers an error
    e.journal.fail.store(true, Ordering::SeqCst);
    let (status, v) = send(&e.app, STAFF, ok).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{v}");

    // the service rejects a non-staff person even when called directly
    let f = build_plugin(
        &{
            let db = &e._db;
            let ctx = Ctx::new(db.pool().clone(), Config::default(), "test").unwrap();
            ctx.apis
                .provide::<dyn WalletsApi>(
                    "wallets",
                    vec![("default", e.wallets.clone() as Arc<dyn WalletsApi>)],
                    Arc::new(api_wallets::stub::Stub),
                )
                .unwrap();
            ctx.apis
                .provide::<dyn JournalApi>(
                    "journal",
                    vec![("default", e.journal.clone() as Arc<dyn JournalApi>)],
                    Arc::new(api_journal::stub::Stub),
                )
                .unwrap();
            ctx
        },
        Options::default(),
    )
    .unwrap()
    .fomo();
    let req = MatchRequest {
        kol: KOL.into(),
        positions: vec![
            PositionInput {
                mint: mint(1),
                amount: "5".into(),
            },
            PositionInput {
                mint: mint(2),
                amount: "5".into(),
            },
        ],
    };
    assert_eq!(
        f.run(&Person::NOBODY, &req).await.unwrap_err(),
        plugin_fomo::service::Error::Forbidden
    );
}
