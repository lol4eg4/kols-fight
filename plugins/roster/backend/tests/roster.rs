//! Roster acceptance (spec «C Network»): sheet parse, membership, keep flag, leaks, export.
//! Invented KOLs, rates and addresses only; the real sheet is never read.

#[path = "../../tests/support/registry_journal.rs"]
mod registry_journal;
#[path = "../../tests/support/snapshots.rs"]
mod snapshots;

use api_board::InNetwork;
use api_journal::JournalApi;
use api_registry::RegistryApi;
use api_roster::{RosterApi, RosterError, TableRead, XHandleVerified};
use api_schedule::SnapshotOpened;
use api_snapshots::MemberReason;
use arena_platform::{
    BoxError, Config, Ctx, DataLock, LockSpec, Person, Role, Tag,
    scheduler::{DATA_LOCK, run_locked},
    testing::TestDb,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use chrono::NaiveDate;
use plugin_roster::{Roster, RosterPlugin, SheetError, SheetFetch, parse_sheet};
use registry_journal::{ADDR, FakeJournal, FakeRegistry};
use serde_json::{Value, json};
use snapshots::FakeSnapshots;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const SHEET: &str = include_str!("../../tests/fixtures/sheet.csv");
const SHEET_PUMPFUN: &str = include_str!("../../tests/fixtures/sheet_pumpfun.csv");
const SHEET_NO_FOMO: &str = include_str!("../../tests/fixtures/sheet_no_fomo.csv");
const SHEET_HEADER_CHANGED: &str = include_str!("../../tests/fixtures/sheet_header_changed.csv");
const URL: &str = "https://sheet.example/export?token=secret-token-1";
const EVM: &str = "0x5b9e3c1a7d2f4086be13a9c74d0e58f26a1b3c97";

// ----- parse ----------------------------------------------------------------------------

#[test]
fn parses_the_sheet_by_header_name() {
    let parsed = parse_sheet(SHEET).unwrap();
    assert!(parsed.skipped.is_empty());
    let names: Vec<_> = parsed.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Alder Vane",
            "Briar Quill",
            "Cobalt Finch",
            "Dune Harrow",
            "Ember Lysk",
            "Fable Moss"
        ]
    );
    let alder = &parsed.rows[0];
    assert_eq!(
        alder.fomo, "alder_vane",
        "a FOMO URL is reduced to the handle"
    );
    assert_eq!(alder.group.as_deref(), Some("Quartz"));
    assert_eq!(
        (alder.low_usd, alder.high_usd),
        (Some(8123.0), Some(9457.0))
    );
    assert_eq!(alder.wallets, [ADDR[0]]);
    assert_eq!(alder.formats, ["post", "thread"]);
    let briar = &parsed.rows[1];
    assert_eq!(briar.fomo, "briar_quill", "a leading @ is dropped");
    assert_eq!(
        briar.wallets,
        [ADDR[1], ADDR[2]],
        "the trailing unnamed column is a second wallet"
    );
    let cobalt = &parsed.rows[2];
    assert_eq!(
        (cobalt.low_usd, cobalt.high_usd),
        (Some(6500.0), Some(7200.0)),
        "6.5k and 7.2k"
    );
    assert!(cobalt.wallets.is_empty());
    assert_eq!(
        cobalt.wallet_note.as_deref(),
        Some("asks to be paid in advance")
    );
    assert_eq!(cobalt.formats, ["post", "buy", "space", "call"]);
    let dune = &parsed.rows[3];
    assert!(
        dune.wallets.is_empty(),
        "an EVM address is not a Solana wallet"
    );
    assert_eq!(dune.wallet_note.as_deref(), Some(EVM));
    assert_eq!((dune.low_usd, dune.high_usd), (Some(5719.0), None));
    assert_eq!(dune.formats, ["call"]);
    assert_eq!(parsed.rows[4].formats, ["space"]);
    assert_eq!(parsed.rows[5].wallets, [ADDR[3]]);
    assert!(parsed.rows[5].formats.is_empty() && parsed.rows[5].note.is_none());
}

#[test]
fn format_keywords_are_whole_words_in_a_fixed_order() {
    let f = plugin_roster::formats_of("Spaces, a THREAD and 2 buys; livestream; postcard");
    assert_eq!(f, ["buy", "thread", "space"]);
    assert!(plugin_roster::formats_of("").is_empty());
}

#[test]
fn parses_a_sheet_with_pumpfun_and_no_wallet_column() {
    let parsed = parse_sheet(SHEET_PUMPFUN).unwrap();
    let pf: Vec<_> = parsed.rows.iter().map(|r| r.pumpfun.as_deref()).collect();
    assert_eq!(pf, [Some("gale_ostrin_pf"), Some("hollis_pf"), None]);
    assert!(
        parsed
            .rows
            .iter()
            .all(|r| r.wallets.is_empty() && r.wallet_note.is_none())
    );
    assert_eq!(parsed.rows[1].formats, ["thread", "stream"]);
    assert_eq!(parsed.rows[2].high_usd, None);
}

#[test]
fn a_sheet_without_the_fomo_column_fails() {
    assert_eq!(
        parse_sheet(SHEET_NO_FOMO).unwrap_err(),
        SheetError::MissingColumn("FOMO")
    );
}

#[test]
fn a_changed_header_fails() {
    assert_eq!(
        parse_sheet(SHEET_HEADER_CHANGED).unwrap_err(),
        SheetError::MissingColumn("Name")
    );
    assert_eq!(parse_sheet("").unwrap_err(), SheetError::Empty);
}

#[test]
fn unusable_rows_are_skipped_with_a_reason() {
    let text = "Name,Group,Low $,High $,Deliverable / note,FOMO\n\
                Rook One,A,1,2,,rook_one\n\
                ,A,1,2,,nameless_handle\n\
                No Handle,A,1,2,,\n\
                Rook Again,A,1,2,,ROOK_ONE\n\
                \n\
                \"Quoted \"\"Two\"\"\",A,1,2,\"line one\nline two\",quoted_two\n";
    let parsed = parse_sheet(text).unwrap();
    let names: Vec<_> = parsed.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Rook One", "Quoted \"Two\""]);
    assert_eq!(parsed.rows[1].note.as_deref(), Some("line one\nline two"));
    let lines: Vec<_> = parsed.skipped.iter().map(|s| s.line).collect();
    assert_eq!(lines, [3, 4, 5], "the blank row is skipped silently");
    assert!(parsed.skipped.iter().all(|s| !s.reason.is_empty()));
}

// ----- environment ----------------------------------------------------------------------

struct FakeFetcher {
    body: Mutex<Result<String, String>>,
    urls: Mutex<Vec<String>>,
}

impl FakeFetcher {
    fn set(&self, csv: &str) {
        *self.body.lock().unwrap() = Ok(csv.to_string());
    }

    fn fail(&self, error: &str) {
        *self.body.lock().unwrap() = Err(error.to_string());
    }
}

#[async_trait]
impl SheetFetch for FakeFetcher {
    async fn fetch(&self, url: &str) -> Result<String, String> {
        self.urls.lock().unwrap().push(url.to_string());
        self.body.lock().unwrap().clone()
    }
}

struct Env {
    db: TestDb,
    ctx: Ctx,
    plugin: Arc<RosterPlugin>,
    roster: Arc<Roster>,
    registry: Arc<FakeRegistry>,
    journal: Arc<FakeJournal>,
    snapshots: Arc<FakeSnapshots>,
    fetcher: Arc<FakeFetcher>,
    app: Router,
    table_reads: Arc<Mutex<Vec<NaiveDate>>>,
    verified: Arc<Mutex<Vec<String>>>,
}

async fn env_with(configured: bool) -> Env {
    let db = TestDb::with_plugins(&[("roster", &plugin_roster::MIGRATOR)]).await;
    let config = Config::from_lookup(
        |name| (configured && name == "NETWORK_SHEET_CSV_URL").then(|| URL.to_string()),
        None,
    )
    .unwrap();
    let ctx = Ctx::new(db.pool().clone(), config, "test").unwrap();
    let registry = Arc::new(FakeRegistry::default());
    let journal = Arc::new(FakeJournal::default());
    let snapshots = Arc::new(FakeSnapshots::default());
    ctx.apis
        .provide::<dyn RegistryApi>(
            "registry",
            vec![("default", registry.clone() as Arc<dyn RegistryApi>)],
            Arc::new(api_registry::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn JournalApi>(
            "journal",
            vec![("default", journal.clone() as Arc<dyn JournalApi>)],
            Arc::new(api_journal::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn api_snapshots::SnapshotsApi>(
            "snapshots",
            vec![(
                "default",
                snapshots.clone() as Arc<dyn api_snapshots::SnapshotsApi>,
            )],
            Arc::new(api_snapshots::stub::Stub),
        )
        .unwrap();
    ctx.slots
        .declare::<dyn InNetwork>("board", "in_network", Arc::new(api_board::NotInNetwork))
        .unwrap();
    let fetcher = Arc::new(FakeFetcher {
        body: Mutex::new(Err("no sheet set".into())),
        urls: Mutex::default(),
    });
    let plugin = plugin_roster::build_with(&ctx, fetcher.clone()).unwrap();

    let table_reads = Arc::new(Mutex::new(Vec::new()));
    let seen = table_reads.clone();
    ctx.events.on::<TableRead, _, _>("probe", move |e| {
        let seen = seen.clone();
        async move {
            seen.lock().unwrap().push(e.date);
            Ok::<(), BoxError>(())
        }
    });
    let verified = Arc::new(Mutex::new(Vec::new()));
    let seen = verified.clone();
    ctx.events.on::<XHandleVerified, _, _>("probe", move |e| {
        let seen = seen.clone();
        async move {
            seen.lock().unwrap().push(e.kol);
            Ok::<(), BoxError>(())
        }
    });

    let mut app = Router::default();
    for route in arena_platform::Plugin::routes(plugin.as_ref(), &ctx) {
        app = app.route(&route.path, route.handler);
    }
    Env {
        roster: plugin.roster(),
        db,
        ctx,
        plugin,
        registry,
        journal,
        snapshots,
        fetcher,
        app,
        table_reads,
        verified,
    }
}

async fn env() -> Env {
    env_with(true).await
}

const STAFF: Person = Person {
    id: Some(1),
    role: Role::Staff,
};
const ADMIN: Person = Person {
    id: Some(2),
    role: Role::Admin,
};

/// One request against the mounted routes as `who`.
async fn send(
    app: Router,
    who: Person,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, String) {
    let mut req = Request::builder().method(method).uri(uri).extension(who);
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let res = app.oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

impl Env {
    async fn call(
        &self,
        who: Person,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, String) {
        send(self.app.clone(), who, method, uri, body).await
    }

    async fn sync(&self, csv: &str) -> plugin_roster::SyncReport {
        self.fetcher.set(csv);
        self.roster.sync_unlocked(&Person::SYSTEM).await
    }

    async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(self.db.pool())
            .await
            .unwrap()
    }

    async fn active_kols(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT kol FROM roster.member WHERE active ORDER BY kol")
            .fetch_all(self.db.pool())
            .await
            .unwrap()
    }
}

/// A sheet with the given (name, fomo, low, high, note) rows.
fn sheet(rows: &[(&str, &str, &str, &str, &str)]) -> String {
    let mut out = String::from("Name,Group,Low $,High $,Deliverable / note,FOMO,Wallet\n");
    for (name, fomo, low, high, note) in rows {
        out.push_str(&format!("{name},Quartz,{low},{high},{note},{fomo},\n"));
    }
    out
}

/// The 13 prototype network KOLs that the owner's sheet does list, plus the registry entries
/// the day-0 seed would have created for those without a FOMO handle.
fn prototype_rows() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)> {
    vec![
        ("Iced", "IcedKnife", "1101", "1202", ""),
        ("J777Crypto", "J777Crypto", "1103", "1204", ""),
        ("Levis", "LevisNFT", "1105", "1206", ""),
        ("Maurits", "mauritsneo", "1107", "1208", ""),
        ("juicy", "Juicycooks", "1109", "1210", ""),
        ("TheSOLstice", "The__Solstice", "1111", "1212", ""),
        ("Smokey", "smokey0x", "1113", "1214", ""),
        ("BrazilZZ", "xbrazilzz", "1115", "1216", ""),
        ("Zagrebian", "Zagy998", "1117", "1218", ""),
        ("BTCFORLIFE", "BTCFORLIFE", "1119", "1220", ""),
        ("NDF_Sonar_trade", "NDF_Sonar_trade", "1121", "1222", ""),
        ("trancey", "trancey", "1123", "1224", ""),
        ("pow", "traderpow", "1125", "1226", ""),
    ]
}

fn seed_registry(env: &Env) {
    env.registry.put("iced", "Iced", Some("IcedKnife"), None);
    env.registry
        .put("j7", "J777Crypto", Some("J777Crypto"), None);
    env.registry.put("pow", "pow", Some("traderpow"), None);
}

// ----- sync -----------------------------------------------------------------------------

#[tokio::test]
async fn sync_adds_sheet_rows_as_members_and_links_them_in_the_registry() {
    let env = env().await;
    let before = env.count("SELECT count(*) FROM roster.member").await;
    assert_eq!(before, 22, "day-0 seed: the 22 prototype network KOLs");
    let report = env.sync(SHEET).await;
    assert_eq!(report.status, "ok", "{:?}", report.error);
    assert_eq!((report.rows, report.added), (6, 6));
    assert_eq!(env.fetcher.urls.lock().unwrap().as_slice(), [URL]);

    let alder = env.registry.get("alder-vane");
    assert_eq!(alder.links.fomo.as_deref(), Some("alder_vane"));
    assert_eq!(
        env.registry.get("briar-quill").formats,
        Vec::<String>::new()
    );
    assert_eq!(
        env.registry.get("cobalt-finch").formats,
        ["post", "buy", "space", "call"]
    );
    assert!(env.roster.is_member("alder-vane").await.unwrap());
    assert_eq!(
        env.table_reads.lock().unwrap().len(),
        1,
        "table_read after a good read"
    );

    // a second read of the same sheet changes nothing and makes no second registry KOL
    let again = env.sync(SHEET).await;
    assert_eq!(
        (again.status, again.added, again.updated, again.removed),
        ("ok", 0, 0, 0)
    );
    assert_eq!(env.registry.kols.lock().unwrap().len(), 6);
    assert_eq!(env.table_reads.lock().unwrap().len(), 2);
    assert_eq!(
        env.count("SELECT count(*) FROM roster.sync_run WHERE status = 'ok'")
            .await,
        2
    );
}

#[tokio::test]
async fn nine_prototype_kols_stay_when_the_sheet_lacks_them_and_removed_rows_end_membership() {
    let env = env().await;
    seed_registry(&env);
    let keep: Vec<String> =
        sqlx::query_scalar("SELECT kol FROM roster.member WHERE keep ORDER BY kol")
            .fetch_all(env.db.pool())
            .await
            .unwrap();
    assert_eq!(
        keep,
        [
            "babatooonde",
            "kook",
            "laseasy",
            "mrpunk",
            "naka",
            "ocr",
            "peter",
            "shah",
            "tekkerrss"
        ]
    );

    let mut rows = prototype_rows();
    rows.push(("Alder Vane", "alder_vane", "8123", "9457", "Post"));
    let first = env.sync(&sheet(&rows)).await;
    assert_eq!(first.status, "ok", "{:?}", first.error);
    assert_eq!(
        (first.added, first.removed),
        (1, 0),
        "the 13 prototype rows match existing members"
    );
    assert_eq!(
        env.count("SELECT count(*) FROM roster.member WHERE active")
            .await,
        23
    );

    // two rows leave the sheet: one ordinary, one new
    rows.retain(|r| r.1 != "Zagy998" && r.1 != "alder_vane");
    let second = env.sync(&sheet(&rows)).await;
    assert_eq!(
        (second.status, second.removed),
        ("ok", 2),
        "{:?}",
        second.error
    );
    let active = env.active_kols().await;
    assert!(!active.contains(&"zagy".to_string()) && !active.contains(&"alder-vane".to_string()));
    for k in &keep {
        assert!(active.contains(k), "{k} is kept without a sheet row");
    }
    assert!(!env.roster.is_member("zagy").await.unwrap());

    // history stays
    let events: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM roster.membership_event WHERE kol = 'zagy' ORDER BY id",
    )
    .fetch_all(env.db.pool())
    .await
    .unwrap();
    assert_eq!(events, ["joined", "left"]);
    assert_eq!(
        env.count("SELECT count(*) FROM roster.member WHERE kol = 'zagy'")
            .await,
        1
    );
    assert_eq!(
        env.count("SELECT count(*) FROM roster.terms WHERE kol = 'zagy'")
            .await,
        1
    );
    let removed = env.journal.of("roster", "zagy");
    assert!(removed.iter().any(|e| e.action == "removed"));

    // the row comes back
    let third = env.sync(&sheet(&prototype_rows())).await;
    assert_eq!(third.added, 1, "{:?}", third.error);
    assert!(env.roster.is_member("zagy").await.unwrap());
    let events: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM roster.membership_event WHERE kol = 'zagy' ORDER BY id",
    )
    .fetch_all(env.db.pool())
    .await
    .unwrap();
    assert_eq!(events, ["joined", "left", "rejoined"]);
}

#[tokio::test]
async fn a_kol_without_a_fomo_handle_in_the_seed_is_found_by_its_x_handle() {
    let env = env().await;
    seed_registry(&env);
    let report = env.sync(&sheet(&prototype_rows())).await;
    assert_eq!(
        report.added, 0,
        "iced, j7 and pow are matched, not duplicated"
    );
    let fomo: Option<String> =
        sqlx::query_scalar("SELECT fomo_handle FROM roster.member WHERE kol = 'iced'")
            .fetch_one(env.db.pool())
            .await
            .unwrap();
    assert_eq!(fomo.as_deref(), Some("IcedKnife"));
    assert_eq!(
        env.registry.get("iced").links.fomo.as_deref(),
        Some("IcedKnife")
    );
}

#[tokio::test]
async fn a_failed_read_changes_nothing() {
    let env = env().await;
    assert_eq!(env.sync(SHEET).await.status, "ok");
    let before = env.active_kols().await;
    let reads = env.table_reads.lock().unwrap().len();

    env.fetcher.fail(&format!("timeout calling {URL}"));
    let report = env.roster.sync_unlocked(&Person::SYSTEM).await;
    assert_eq!(report.status, "failed");
    let error = report.error.unwrap();
    assert!(
        !error.contains("secret-token-1"),
        "the sheet URL stays out of messages: {error}"
    );

    for (label, text) in [
        ("header changed", SHEET_HEADER_CHANGED),
        ("no FOMO column", SHEET_NO_FOMO),
        ("empty", ""),
        ("no usable rows", "Name,FOMO\n,\n"),
    ] {
        let report = env.sync(text).await;
        assert_eq!(report.status, "failed", "{label}");
        assert!(report.error.is_some(), "{label}");
    }
    assert_eq!(
        env.active_kols().await,
        before,
        "the roster stays as it was"
    );
    assert_eq!(
        env.table_reads.lock().unwrap().len(),
        reads,
        "no table_read after a failed read"
    );
    assert_eq!(
        env.count("SELECT count(*) FROM roster.sync_run WHERE status = 'failed'")
            .await,
        5
    );
    let (status, body) = env
        .call(ADMIN, "GET", "/api/admin/network/sync", None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("failed"), "{body}");
}

#[tokio::test]
async fn without_a_configured_url_the_sheet_is_not_read() {
    let env = env_with(false).await;
    env.fetcher.set(SHEET);
    let report = env.roster.sync_unlocked(&Person::SYSTEM).await;
    assert_eq!(report.status, "failed");
    assert_eq!(
        report.error.as_deref(),
        Some("NETWORK_SHEET_CSV_URL not configured")
    );
    assert!(env.fetcher.urls.lock().unwrap().is_empty());
    assert_eq!(env.count("SELECT count(*) FROM roster.member").await, 22);
}

#[tokio::test]
async fn skipped_rows_are_reported() {
    let env = env().await;
    let csv = format!("{SHEET}No Handle,Quartz,1,2,,\n");
    let report = env.sync(&csv).await;
    assert_eq!((report.status, report.rows), ("ok", 6));
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].line, 8);
}

#[tokio::test]
async fn the_daily_job_runs_at_0545_with_the_data_lock() {
    let env = env().await;
    let jobs = arena_platform::Plugin::jobs(env.plugin.as_ref(), &env.ctx);
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].name, "roster_sync");
    assert_eq!(
        jobs[0].at,
        chrono::NaiveTime::from_hms_opt(5, 45, 0).unwrap()
    );
    assert!(matches!(jobs[0].data_lock, DataLock::Try));
    env.fetcher.set(SHEET);
    arena_platform::scheduler::run_job(env.db.pool(), &jobs[0])
        .await
        .unwrap();
    assert!(env.roster.is_member("ember-lysk").await.unwrap());
}

#[tokio::test]
async fn a_held_data_lock_answers_busy() {
    let env = env().await;
    env.fetcher.set(SHEET);
    let app = env.app.clone();
    let held = run_locked(
        env.db.pool(),
        vec![LockSpec::try_once(DATA_LOCK)],
        async move { send(app, ADMIN, "POST", "/api/admin/network/sync", None).await },
    )
    .await
    .unwrap();
    assert_eq!(held.0, StatusCode::CONFLICT, "{}", held.1);
    assert_eq!(env.count("SELECT count(*) FROM roster.sync_run").await, 0);
    // The server frees an advisory lock when it notices the closed connection, a moment
    // after the run returned: retry while busy.
    for round in 0..2 {
        let mut last = (StatusCode::CONFLICT, String::new());
        for _ in 0..40 {
            last = env
                .call(ADMIN, "POST", "/api/admin/network/sync", None)
                .await;
            if last.0 != StatusCode::CONFLICT {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert_eq!(last.0, StatusCode::OK, "run {round}: {}", last.1);
    }
}

// ----- events, snapshot, slot -----------------------------------------------------------

#[tokio::test]
async fn snapshot_opened_adds_network_members_with_a_wallet() {
    let env = env().await;
    env.sync(SHEET).await;
    env.registry
        .set_main("alder-vane", ADDR[4], api_registry::WalletSource::Network);
    env.registry.set_main(
        "fable-moss",
        ADDR[5],
        api_registry::WalletSource::Kolexplorer,
    );
    env.registry
        .set_main("briar-quill", ADDR[6], api_registry::WalletSource::Network);
    // briar leaves the sheet: not a member any more, wallet or not
    let without_briar = SHEET
        .lines()
        .filter(|l| !l.contains("briar_quill"))
        .collect::<Vec<_>>()
        .join("\n");
    env.sync(&without_briar).await;

    let date = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
    env.ctx.events.emit(&SnapshotOpened { date }).await.unwrap();
    let sets = env.snapshots.members.lock().unwrap().clone();
    assert_eq!(sets.len(), 1);
    let (d, reason, mut kols) = sets[0].clone();
    kols.sort();
    assert_eq!((d, reason), (date, MemberReason::Network));
    assert_eq!(
        kols,
        ["alder-vane", "fable-moss"],
        "no wallet or no longer a member: not added"
    );
}

#[tokio::test]
async fn fills_the_in_network_slot() {
    let env = env().await;
    let slot = env.ctx.slots.get::<dyn InNetwork>().unwrap();
    assert_eq!(slot.filler(), Some("roster"));
    assert!(!slot.get().is_member(&Person::NOBODY, "alder-vane").await);
    env.sync(SHEET).await;
    assert!(slot.get().is_member(&Person::NOBODY, "alder-vane").await);
    assert!(
        slot.get().is_member(&Person::NOBODY, "kook").await,
        "kept prototype KOL"
    );
    assert!(!slot.get().is_member(&Person::NOBODY, "someone-else").await);
}

#[tokio::test]
async fn staff_verifies_an_x_handle_and_the_event_goes_out() {
    let env = env().await;
    env.sync(SHEET).await;
    let uri = "/api/team/network/alder-vane";
    let body = json!({ "x_handle": "@alder_v", "formats": ["Thread", "post", "thread"] });
    let (status, text) = env.call(STAFF, "PATCH", uri, Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(env.verified.lock().unwrap().as_slice(), ["alder-vane"]);
    let profile = env.registry.get("alder-vane");
    assert_eq!(profile.x_handle.as_deref(), Some("alder_v"));
    assert_eq!(profile.formats, ["thread", "post"]);
    let api = plugin_roster::RosterImpl(env.roster.clone());
    let handle = api
        .verified_handle(&STAFF, "alder-vane")
        .await
        .unwrap()
        .unwrap();
    assert_eq!((handle.handle.as_str(), handle.verified), ("alder_v", true));
    assert!(
        env.journal
            .actions("roster", "alder-vane")
            .contains(&"patch".to_string())
    );

    // unchanged handle: no second event
    env.call(STAFF, "PATCH", uri, Some(json!({ "x_handle": "alder_v" })))
        .await;
    assert_eq!(env.verified.lock().unwrap().len(), 1);

    for (who, target, body, want) in [
        (
            STAFF,
            uri,
            json!({ "x_handle": "not a handle!" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            STAFF,
            uri,
            json!({ "formats": ["podcast"] }),
            StatusCode::BAD_REQUEST,
        ),
        (
            STAFF,
            "/api/team/network/zagy-not-here",
            json!({ "x_handle": "abc" }),
            StatusCode::NOT_FOUND,
        ),
        (
            Person::NOBODY,
            uri,
            json!({ "x_handle": "abc" }),
            StatusCode::FORBIDDEN,
        ),
    ] {
        let (status, text) = env.call(who, "PATCH", target, Some(body)).await;
        assert_eq!(status, want, "{text}");
    }
}

// ----- nothing public carries rates -----------------------------------------------------

const SECRETS: [&str; 11] = [
    "8123", "9457", "7311", "8802", "6500", "7200", "5719", "4283", "5114", "3377", "3996",
];
const PRIVATE_TEXT: [&str; 7] = [
    "Quartz",
    "Basalt",
    "Jasper",
    "asks to be paid in advance",
    "quoted retweet",
    "Livestream on request",
    EVM,
];

fn assert_clean(label: &str, text: &str) {
    for s in SECRETS.iter().chain(PRIVATE_TEXT.iter()).chain(&ADDR[..4]) {
        assert!(!text.contains(s), "{label} leaks {s}: {text}");
    }
}

#[tokio::test]
async fn no_public_surface_carries_rates_groups_notes_or_sheet_wallets() {
    let env = env().await;
    env.sync(SHEET).await;

    // routes: none is public, and a nobody gets 403 on all of them
    for route in arena_platform::Plugin::routes(env.plugin.as_ref(), &env.ctx) {
        assert_ne!(route.tag, Tag::Public, "{}", route.path);
    }
    for (method, uri) in [
        ("GET", "/api/team/network"),
        ("PATCH", "/api/team/network/alder-vane"),
        ("GET", "/api/admin/network/sync"),
        ("POST", "/api/admin/network/sync"),
        ("GET", "/api/admin/network/export"),
    ] {
        let (status, text) = env.call(Person::NOBODY, method, uri, Some(json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        assert_clean(uri, &text);
    }
    for (method, uri) in [
        ("GET", "/api/admin/network/sync"),
        ("POST", "/api/admin/network/sync"),
        ("GET", "/api/admin/network/export"),
    ] {
        let (status, _) = env.call(STAFF, method, uri, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "staff is not admin: {uri}");
    }

    // the API: rates are for staff only; the member list has no private field for a nobody
    let api = plugin_roster::RosterImpl(env.roster.clone());
    let members = api.members(&Person::NOBODY).await.unwrap();
    assert_eq!(
        members.len(),
        15,
        "9 kept prototype KOLs and the 6 sheet rows"
    );
    assert_clean("members(nobody)", &serde_json::to_string(&members).unwrap());
    assert!(members.iter().all(|m| m.sheet_wallets.is_empty()));
    let all: Vec<String> = members.iter().map(|m| m.kol.clone()).collect();
    assert_eq!(
        api.rates(&Person::NOBODY, &all).await,
        Err(RosterError::Forbidden)
    );
    let staff_members = api.members(&STAFF).await.unwrap();
    let alder = staff_members
        .iter()
        .find(|m| m.kol == "alder-vane")
        .unwrap();
    assert_eq!(
        alder.sheet_wallets,
        [ADDR[0]],
        "staff gets the sheet wallets"
    );

    // what the roster pushed into other plugins: no rates, groups, notes, sheet wallets
    let registry_state = serde_json::to_string(&*env.registry.kols.lock().unwrap()).unwrap();
    assert_clean("registry", &registry_state);
    let journal_payload: Vec<_> = env
        .journal
        .all()
        .into_iter()
        .map(|e| json!([e.object, e.action, e.before, e.after]))
        .collect();
    assert_clean("journal", &serde_json::to_string(&journal_payload).unwrap());
    assert!(
        env.registry.publishes.lock().unwrap().is_empty(),
        "the roster publishes no wallet"
    );

    // staff do get them from the team route
    let (status, text) = env.call(STAFF, "GET", "/api/team/network", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("Quartz") && text.contains("8123"), "{text}");
    let staff_rates = api
        .rates(&STAFF, &["alder-vane".to_string()])
        .await
        .unwrap();
    assert_eq!(
        (
            staff_rates[0].low_usd,
            staff_rates[0].high_usd,
            staff_rates[0].formats.clone()
        ),
        (
            Some(8123.0),
            Some(9457.0),
            vec!["post".to_string(), "thread".to_string()]
        )
    );
}

#[tokio::test]
async fn a_changed_rate_is_journaled_by_field_name_only() {
    let env = env().await;
    env.sync(SHEET).await;
    let changed = SHEET.replace("\"$8,123\",9457", "\"$8,555\",9999");
    let report = env.sync(&changed).await;
    assert_eq!((report.updated, report.added, report.removed), (1, 0, 0));
    let entries = env.journal.of("roster", "alder-vane");
    let update = entries
        .iter()
        .find(|e| e.action == "updated")
        .expect("an update entry");
    let text = serde_json::to_string(&json!([update.before, update.after])).unwrap();
    assert!(text.contains("rates"), "{text}");
    for v in ["8123", "8555", "9457", "9999"] {
        assert!(!text.contains(v), "journal leaks a rate: {text}");
    }
    let (_, team) = env.call(STAFF, "GET", "/api/team/network", None).await;
    assert!(team.contains("8555") && !team.contains("8123"));
}

#[tokio::test]
async fn the_admin_export_is_a_csv_with_the_sheet_columns() {
    let env = env().await;
    let evil = format!("{SHEET}=SUM(1+1),Quartz,1,2,\"a,b\",evil_row,\n");
    env.sync(&evil).await;
    let (status, text) = env
        .call(ADMIN, "GET", "/api/admin/network/export", None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("Name,Group,Low $,High $,Deliverable / note,FOMO,Pump.fun,Formats")
    );
    assert!(text.contains(
        "Alder Vane,Quartz,8123,9457,\"Post + Thread, quoted retweet\",alder_vane,,post thread"
    ));
    assert!(text.contains("Cobalt Finch,Basalt,6500,7200"));
    assert!(
        text.contains("'=SUM(1+1)"),
        "a formula cell is defused: {text}"
    );
    assert!(
        !text.contains(EVM) && !text.contains(ADDR[0]),
        "wallets are not exported"
    );
}
