//! Wallets acceptance (spec «C Network»): the state machine, sheet addresses, conflicts,
//! manual confirmation, the journal. Fakes for every API; invented KOLs and addresses.

#[path = "../../../roster/tests/support/wallets.rs"]
mod fakes;
#[path = "../../../roster/tests/support/registry_journal.rs"]
mod registry_journal;

use api_journal::JournalApi;
use api_kolexplorer::KolexplorerApi;
use api_registry::{RegistryApi, WalletSource};
use api_roster::{RosterApi, TableRead, XHandleVerified, is_solana_address};
use api_solanatracker::SolanatrackerApi;
use api_wallets::{
    Candidate, CandidateSource, CandidateStatus, KolWalletStatus, WalletsApi, WalletsError,
};
use arena_platform::{
    Config, Ctx, LockSpec, Person, Role, Tag,
    scheduler::{DATA_LOCK, run_locked},
    testing::TestDb,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use chrono::NaiveDate;
use fakes::{FakeKolexplorer, FakeRoster, FakeSolanaTracker};
use plugin_wallets::{Error, Wallets, WalletsPlugin, allowed, kol_status};
use registry_journal::{ADDR, FakeJournal, FakeRegistry};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};
use tower::ServiceExt;

use CandidateStatus::*;

const STAFF: Person = Person {
    id: Some(1),
    role: Role::Staff,
};
const SYSTEM: Person = Person::SYSTEM;

struct Env {
    db: TestDb,
    ctx: Ctx,
    plugin: Arc<WalletsPlugin>,
    w: Arc<Wallets>,
    roster: Arc<FakeRoster>,
    st: Arc<FakeSolanaTracker>,
    ke: Arc<FakeKolexplorer>,
    registry: Arc<FakeRegistry>,
    journal: Arc<FakeJournal>,
    app: Router,
}

async fn env() -> Env {
    let db = TestDb::with_plugins(&[("wallets", &plugin_wallets::MIGRATOR)]).await;
    let config = Config::from_lookup(|_| None, None).unwrap();
    let ctx = Ctx::new(db.pool().clone(), config, "test").unwrap();
    let roster = Arc::new(FakeRoster::default());
    let st = Arc::new(FakeSolanaTracker::default());
    let ke = Arc::new(FakeKolexplorer::default());
    let registry = Arc::new(FakeRegistry::default());
    let journal = Arc::new(FakeJournal::default());
    ctx.apis
        .provide::<dyn RosterApi>(
            "roster",
            vec![("default", roster.clone() as Arc<dyn RosterApi>)],
            Arc::new(api_roster::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn SolanatrackerApi>(
            "solanatracker",
            vec![("default", st.clone() as Arc<dyn SolanatrackerApi>)],
            Arc::new(api_solanatracker::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn KolexplorerApi>(
            "kolexplorer",
            vec![("default", ke.clone() as Arc<dyn KolexplorerApi>)],
            Arc::new(api_kolexplorer::stub::Stub),
        )
        .unwrap();
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
    let plugin = plugin_wallets::build_plugin(&ctx).unwrap();
    let mut app = Router::default();
    for route in arena_platform::Plugin::routes(plugin.as_ref(), &ctx) {
        assert_eq!(route.tag, Tag::Staff, "{} must be staff-only", route.path);
        app = app.route(&route.path, route.handler);
    }
    Env {
        w: plugin.wallets(),
        db,
        ctx,
        plugin,
        roster,
        st,
        ke,
        registry,
        journal,
        app,
    }
}

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

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()
}

impl Env {
    /// A network KOL: in the roster (X handle = FOMO handle, not verified) and the registry.
    fn add(&self, kol: &str, handle: &str, sheet: &[&str]) {
        self.roster
            .add(kol, &format!("Name of {kol}"), handle, sheet);
        self.registry
            .put(kol, &format!("Name of {kol}"), Some(handle), Some(handle));
    }

    /// The pass the sheet read triggers.
    async fn pass(&self) {
        self.ctx
            .events
            .emit(&TableRead { date: date() })
            .await
            .unwrap();
    }

    async fn call(
        &self,
        who: Person,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, String) {
        send(self.app.clone(), who, method, uri, body).await
    }

    async fn all(&self, kol: &str) -> Vec<Candidate> {
        let o = self.w.overview(&STAFF).await.unwrap();
        o.kols
            .into_iter()
            .find(|k| k.kol == kol)
            .map(|k| k.candidates.into_iter().map(|c| c.candidate).collect())
            .unwrap_or_default()
    }

    async fn cand(&self, kol: &str, address: &str) -> Candidate {
        self.all(kol)
            .await
            .into_iter()
            .find(|c| c.address == address)
            .unwrap_or_else(|| panic!("{kol} has no candidate {address}"))
    }

    async fn status_of(&self, kol: &str, address: &str) -> CandidateStatus {
        self.cand(kol, address).await.status
    }

    async fn kol_status(&self, kol: &str) -> KolWalletStatus {
        self.w.status(&STAFF, kol).await.unwrap().status
    }

    fn actions(&self, kol: &str) -> Vec<String> {
        self.journal.actions("wallets", kol)
    }
}

#[test]
fn test_addresses_are_valid_and_distinct() {
    assert!(ADDR.iter().all(|a| is_solana_address(a)));
    assert_eq!(ADDR.iter().collect::<BTreeSet<_>>().len(), ADDR.len());
}

// ----- sheet addresses ------------------------------------------------------------------

#[tokio::test]
async fn a_sheet_address_with_trades_is_confirmed_with_source_sheet() {
    let e = env().await;
    e.add("alder-vane", "alder_vane", &[ADDR[0]]);
    e.st.trades(ADDR[0]);
    e.pass().await;

    let c = e.cand("alder-vane", ADDR[0]).await;
    assert_eq!(
        (c.status, c.source, c.main),
        (Confirmed, CandidateSource::Sheet, true)
    );
    assert_eq!(c.reason.as_deref(), Some("trades found"));
    assert_eq!(e.kol_status("alder-vane").await, KolWalletStatus::Confirmed);
    assert_eq!(e.registry.main_of("alder-vane").as_deref(), Some(ADDR[0]));
    assert_eq!(
        e.registry.get("alder-vane").main_wallet.unwrap().source,
        WalletSource::Network
    );
    let status = e.w.status(&Person::NOBODY, "alder-vane").await.unwrap();
    assert_eq!(status.main_address.as_deref(), Some(ADDR[0]));
    assert!(status.last_trade_at.is_some());
    assert_eq!(e.actions("alder-vane"), ["found", "publish"]);

    // a second pass with nothing new writes nothing
    e.pass().await;
    assert_eq!(e.actions("alder-vane"), ["found", "publish"]);
}

#[tokio::test]
async fn a_sheet_address_without_trades_stays_not_found_with_the_reason() {
    let e = env().await;
    e.add("briar-quill", "briar_quill", &[ADDR[1]]);
    e.st.no_trades(ADDR[1]);
    e.pass().await;

    let c = e.cand("briar-quill", ADDR[1]).await;
    assert_eq!(
        (c.status, c.source, c.main),
        (NotFound, CandidateSource::Sheet, false)
    );
    assert_eq!(
        c.reason.as_deref(),
        Some("no trades found for this address")
    );
    assert_eq!(e.kol_status("briar-quill").await, KolWalletStatus::NotFound);
    assert_eq!(
        e.registry.main_of("briar-quill"),
        None,
        "nothing is published"
    );
    let status = e.w.status(&Person::NOBODY, "briar-quill").await.unwrap();
    assert_eq!(
        (status.status, status.main_address),
        (KolWalletStatus::NotFound, None)
    );
    e.pass().await;
    assert_eq!(e.status_of("briar-quill", ADDR[1]).await, NotFound);
    assert_eq!(e.actions("briar-quill"), ["found"]);

    // trades show up later: the next pass confirms it
    e.st.trades(ADDR[1]);
    e.pass().await;
    let c = e.cand("briar-quill", ADDR[1]).await;
    assert_eq!((c.status, c.main), (Confirmed, true));
    assert_eq!(e.actions("briar-quill"), ["found", "confirm", "publish"]);
    assert_eq!(e.registry.main_of("briar-quill").as_deref(), Some(ADDR[1]));
}

#[tokio::test]
async fn a_failed_trades_check_is_not_a_confirmation() {
    let e = env().await;
    e.add("cobalt-finch", "cobalt_finch", &[ADDR[2]]);
    e.st.trades(ADDR[2]);
    e.st.failing
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(
        e.ctx
            .events
            .emit(&TableRead { date: date() })
            .await
            .is_err(),
        "the problem is reported"
    );
    let c = e.cand("cobalt-finch", ADDR[2]).await;
    assert_eq!(c.status, NotFound);
    assert!(c.reason.unwrap().starts_with("trades check failed"));
    assert_eq!(e.registry.main_of("cobalt-finch"), None);

    e.st.failing
        .store(false, std::sync::atomic::Ordering::SeqCst);
    e.pass().await;
    assert_eq!(e.status_of("cobalt-finch", ADDR[2]).await, Confirmed);
}

#[tokio::test]
async fn a_changed_sheet_address_retires_the_old_candidate() {
    let e = env().await;
    e.add("dune-harrow", "dune_harrow", &[ADDR[0]]);
    e.st.trades(ADDR[0]);
    e.st.trades(ADDR[1]);
    e.pass().await;
    assert_eq!(e.registry.main_of("dune-harrow").as_deref(), Some(ADDR[0]));

    e.roster.set_sheet("dune-harrow", &[ADDR[1]]);
    e.pass().await;
    let old = e.cand("dune-harrow", ADDR[0]).await;
    assert_eq!((old.status, old.main), (Retired, false));
    assert_eq!(old.reason.as_deref(), Some("sheet shows another address"));
    let new = e.cand("dune-harrow", ADDR[1]).await;
    assert_eq!((new.status, new.main), (Confirmed, true));
    assert_eq!(e.registry.main_of("dune-harrow").as_deref(), Some(ADDR[1]));
    assert!(
        e.registry.others_of("dune-harrow").is_empty(),
        "the old address is unpublished"
    );

    // an old candidate that was only «not found» retires too
    e.roster.set_sheet("dune-harrow", &[ADDR[2]]);
    e.st.no_trades(ADDR[2]);
    e.pass().await;
    assert_eq!(e.status_of("dune-harrow", ADDR[1]).await, Retired);
    assert_eq!(e.status_of("dune-harrow", ADDR[2]).await, NotFound);
    assert_eq!(e.registry.main_of("dune-harrow"), None);
    assert_eq!(e.kol_status("dune-harrow").await, KolWalletStatus::NotFound);
}

// ----- the state machine ----------------------------------------------------------------

#[test]
fn the_state_machine_allows_exactly_the_listed_moves() {
    let all = [
        Hypothesis, Confirmed, NotFound, Rejected, Revoked, Retired, Conflict,
    ];
    let want: BTreeSet<(String, String)> = [
        (Hypothesis, Confirmed),
        (Hypothesis, Rejected),
        (Hypothesis, Conflict),
        (NotFound, Confirmed),
        (NotFound, Conflict),
        (NotFound, Retired),
        (Confirmed, Revoked),
        (Confirmed, Conflict),
        (Confirmed, Retired),
        (Conflict, Confirmed),
        (Conflict, Rejected),
        (Conflict, Revoked),
        (Conflict, Retired),
        (Retired, Confirmed),
        (Retired, NotFound),
        (Retired, Conflict),
    ]
    .into_iter()
    .map(|(a, b)| (format!("{a:?}"), format!("{b:?}")))
    .collect();
    let got: BTreeSet<_> = all
        .iter()
        .flat_map(|a| all.iter().map(move |b| (*a, *b)))
        .filter(|(a, b)| allowed(*a, *b))
        .map(|(a, b)| (format!("{a:?}"), format!("{b:?}")))
        .collect();
    assert_eq!(got, want);
    for from in [Rejected, Revoked] {
        assert!(
            all.iter().all(|to| !allowed(from, *to)),
            "{from:?} is final"
        );
    }
}

#[test]
fn a_kol_status_is_conflict_over_confirmed_over_hypothesis_over_not_found() {
    assert_eq!(kol_status([]), KolWalletStatus::NotFound);
    assert_eq!(
        kol_status([Rejected, Revoked, Retired, NotFound]),
        KolWalletStatus::NotFound
    );
    assert_eq!(
        kol_status([NotFound, Hypothesis]),
        KolWalletStatus::Hypothesis
    );
    assert_eq!(
        kol_status([Hypothesis, Confirmed]),
        KolWalletStatus::Confirmed
    );
    assert_eq!(
        kol_status([Confirmed, Conflict, Hypothesis]),
        KolWalletStatus::Conflict
    );
}

/// Drives all 16 moves through the service, then reads them back from the journal.
#[tokio::test]
async fn every_transition_is_reached_and_journaled() {
    let e = env().await;
    let a = &ADDR;

    // confirmed -> revoked; a revoked address is not brought back by the sheet
    e.add("k1", "h1", &[a[0]]);
    e.st.trades(a[0]);
    e.pass().await;
    let id = e.cand("k1", a[0]).await.id;
    assert!(
        matches!(e.w.revoke(&STAFF, id, "  ").await, Err(Error::Refused(_))),
        "a reason is needed"
    );
    e.w.revoke(&STAFF, id, "not his wallet").await.unwrap();
    e.pass().await;
    assert_eq!(e.status_of("k1", a[0]).await, Revoked);
    assert_eq!(e.registry.main_of("k1"), None);

    // confirmed -> retired, not_found -> retired, retired -> confirmed,
    // retired -> not_found, not_found -> confirmed
    e.add("k2", "h2", &[a[1]]);
    e.st.trades(a[1]);
    e.pass().await;
    e.st.no_trades(a[2]);
    e.roster.set_sheet("k2", &[a[2]]);
    e.pass().await;
    assert_eq!(e.status_of("k2", a[1]).await, Retired);
    e.roster.set_sheet("k2", &[a[1]]);
    e.pass().await;
    assert_eq!(e.status_of("k2", a[2]).await, Retired);
    assert_eq!(e.status_of("k2", a[1]).await, Confirmed);
    e.roster.set_sheet("k2", &[a[1], a[2]]);
    e.pass().await;
    assert_eq!(e.status_of("k2", a[2]).await, NotFound);
    e.st.trades(a[2]);
    e.pass().await;
    assert_eq!(e.status_of("k2", a[2]).await, Confirmed);

    // hypothesis -> rejected (and a rejected address stays rejected)
    e.add("k3", "h3", &[]);
    e.st.identity("h3", a[3], None);
    e.pass().await;
    let c = e.cand("k3", a[3]).await;
    assert_eq!(
        (c.status, c.source),
        (Hypothesis, CandidateSource::Solanatracker)
    );
    e.w.reject(&STAFF, c.id, Some("name only")).await.unwrap();
    e.pass().await;
    assert_eq!(e.status_of("k3", a[3]).await, Rejected);
    assert!(matches!(
        e.w.confirm(&STAFF, c.id, "somewhere").await,
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        e.w.revoke(&STAFF, c.id, "why").await,
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        e.w.reject(&STAFF, c.id, None).await,
        Err(Error::Refused(_))
    ));

    // hypothesis -> confirmed
    e.add("k4", "h4", &[]);
    e.st.identity("h4", a[4], None);
    e.pass().await;
    let id = e.cand("k4", a[4]).await.id;
    e.w.confirm(&STAFF, id, "Space on X, 5 March")
        .await
        .unwrap();
    assert_eq!(e.status_of("k4", a[4]).await, Confirmed);

    // hypothesis -> conflict, confirmed -> conflict, then conflict -> retired and
    // conflict -> confirmed when the sheet of the other KOL moves on
    e.add("k5", "h5", &[]);
    e.st.identity("h5", a[5], None);
    e.add("k6", "h6", &[a[5]]);
    e.st.trades(a[5]);
    e.pass().await;
    assert_eq!(e.status_of("k5", a[5]).await, Hypothesis);
    assert_eq!(e.status_of("k6", a[5]).await, Confirmed);
    let id = e.cand("k5", a[5]).await.id;
    e.w.confirm(&STAFF, id, "DM, 6 March").await.unwrap();
    assert_eq!(e.status_of("k5", a[5]).await, Conflict);
    assert_eq!(e.status_of("k6", a[5]).await, Conflict);
    e.st.trades(a[6]);
    e.roster.set_sheet("k6", &[a[6]]);
    e.pass().await;
    assert_eq!(e.status_of("k6", a[5]).await, Retired);
    assert_eq!(e.status_of("k5", a[5]).await, Confirmed);
    assert_eq!(e.registry.main_of("k5").as_deref(), Some(a[5]));

    // conflict -> rejected: the other side is confirmed again
    e.add("k7", "h7", &[a[7]]);
    e.add("k8", "h8", &[a[7]]);
    e.st.trades(a[7]);
    e.pass().await;
    assert_eq!(e.status_of("k7", a[7]).await, Conflict);
    assert_eq!(e.status_of("k8", a[7]).await, Conflict);
    let id = e.cand("k8", a[7]).await.id;
    e.w.reject(&STAFF, id, Some("k7 owns it")).await.unwrap();
    assert_eq!(e.status_of("k8", a[7]).await, Rejected);
    assert_eq!(e.status_of("k7", a[7]).await, Confirmed);
    e.pass().await;
    assert_eq!(
        e.status_of("k8", a[7]).await,
        Rejected,
        "the sheet does not bring it back"
    );

    // conflict -> revoked
    e.add("k9", "h9", &[a[8]]);
    e.add("k10", "h10", &[a[8]]);
    e.st.trades(a[8]);
    e.pass().await;
    let id = e.cand("k9", a[8]).await.id;
    e.w.revoke(&STAFF, id, "k10 owns it").await.unwrap();
    assert_eq!(e.status_of("k9", a[8]).await, Revoked);
    assert_eq!(e.status_of("k10", a[8]).await, Confirmed);

    // retired -> conflict
    e.add("k11", "h11", &[a[9]]);
    e.st.no_trades(a[9]);
    e.pass().await;
    e.st.no_trades(a[10]);
    e.roster.set_sheet("k11", &[a[10]]);
    e.pass().await;
    assert_eq!(e.status_of("k11", a[9]).await, Retired);
    e.add("k12", "h12", &[a[9]]);
    e.st.trades(a[9]);
    e.pass().await;
    assert_eq!(e.status_of("k12", a[9]).await, Confirmed);
    e.roster.set_sheet("k11", &[a[9], a[10]]);
    e.pass().await;
    assert_eq!(e.status_of("k11", a[9]).await, Conflict);
    assert_eq!(e.status_of("k12", a[9]).await, Conflict);

    // not_found -> conflict: the other KOL comes first in the pass
    e.add("k15", "h15", &[a[11]]);
    e.st.no_trades(a[11]);
    e.pass().await;
    assert_eq!(e.status_of("k15", a[11]).await, NotFound);
    e.roster.remove("k15");
    e.add("k16", "h16", &[a[11]]);
    e.add("k15", "h15", &[a[11]]);
    e.st.trades(a[11]);
    e.pass().await;
    assert_eq!(e.status_of("k15", a[11]).await, Conflict);
    assert_eq!(e.status_of("k16", a[11]).await, Conflict);

    // the journal holds every move, and only the allowed ones
    let mut moves = BTreeSet::new();
    for entry in e.journal.all() {
        let from = entry
            .before
            .as_ref()
            .and_then(|b| b["status"].as_str().map(str::to_string));
        let to = entry
            .after
            .as_ref()
            .and_then(|b| b["status"].as_str().map(str::to_string));
        if let (Some(from), Some(to)) = (from, to) {
            assert_ne!(from, to);
            moves.insert((from, to));
        }
    }
    let all = [
        Hypothesis, Confirmed, NotFound, Rejected, Revoked, Retired, Conflict,
    ];
    let name = |s: CandidateStatus| {
        serde_json::to_value(s)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    };
    let want: BTreeSet<(String, String)> = all
        .iter()
        .flat_map(|a| all.iter().map(move |b| (*a, *b)))
        .filter(|(a, b)| allowed(*a, *b))
        .map(|(a, b)| (name(a), name(b)))
        .collect();
    assert_eq!(want.len(), 16);
    assert_eq!(moves, want, "every allowed move was made, and nothing else");

    // the last journal entry of every candidate says what it is now
    for k in e.w.overview(&STAFF).await.unwrap().kols {
        for c in k.candidates {
            let last = e
                .journal
                .of("wallets", &k.kol)
                .into_iter()
                .rev()
                .find(|j| j.after.as_ref().is_some_and(|a| a["id"] == c.candidate.id))
                .unwrap_or_else(|| {
                    panic!("no journal entry for {} {}", k.kol, c.candidate.address)
                });
            assert_eq!(
                last.after.unwrap()["status"],
                json!(name(c.candidate.status)),
                "{}",
                k.kol
            );
        }
    }
}

// ----- manual confirmation --------------------------------------------------------------

async fn hypothesis(e: &Env, kol: &str, handle: &str, address: &str) -> Candidate {
    e.add(kol, handle, &[]);
    e.st.identity(handle, address, None);
    e.pass().await;
    let c = e.cand(kol, address).await;
    assert_eq!(c.status, Hypothesis);
    c
}

#[tokio::test]
async fn confirming_a_hypothesis_needs_where_and_when_the_kol_confirmed() {
    let e = env().await;
    let c = hypothesis(&e, "ember-lysk", "ember_lysk", ADDR[3]).await;
    assert_eq!(
        c.reason.as_deref(),
        Some("matches @ember_lysk by name only")
    );

    for blank in ["", "   "] {
        assert!(matches!(
            e.w.confirm(&STAFF, c.id, blank).await,
            Err(Error::Refused(_))
        ));
    }
    let uri = format!("/api/team/wallets/{}/confirm", c.id);
    for body in [json!({ "where_when": "" }), json!({})] {
        let (status, text) = e.call(STAFF, "POST", &uri, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    }
    assert_eq!(e.status_of("ember-lysk", ADDR[3]).await, Hypothesis);
    assert_eq!(e.registry.main_of("ember-lysk"), None);

    // only staff decide
    let nobody = json!({ "where_when": "DM" });
    let (status, _) = e.call(Person::NOBODY, "POST", &uri, Some(nobody)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(matches!(
        e.w.confirm(&Person::NOBODY, c.id, "DM").await,
        Err(Error::Forbidden)
    ));

    let body = json!({ "where_when": "DM on X, 2 March 2026" });
    let (status, text) = e.call(STAFF, "POST", &uri, Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let done = e.cand("ember-lysk", ADDR[3]).await;
    assert_eq!((done.status, done.main), (Confirmed, true));
    assert_eq!(done.confirmation.as_deref(), Some("DM on X, 2 March 2026"));
    assert_eq!(
        done.source,
        CandidateSource::Solanatracker,
        "the source stays what found it"
    );
    assert_eq!(e.registry.main_of("ember-lysk").as_deref(), Some(ADDR[3]));
    let entry = e
        .journal
        .of("wallets", "ember-lysk")
        .into_iter()
        .find(|j| j.action == "confirm")
        .unwrap();
    assert_eq!(entry.person_id, Some(1));
    assert_eq!(entry.before.unwrap()["status"], "hypothesis");
    assert_eq!(
        entry.after.unwrap()["confirmation"],
        "DM on X, 2 March 2026"
    );

    let (again, _) = e
        .call(STAFF, "POST", &uri, Some(json!({ "where_when": "again" })))
        .await;
    assert_eq!(again, StatusCode::BAD_REQUEST, "already confirmed");
}

#[tokio::test]
async fn a_hypothesis_address_never_leaves_the_staff_area() {
    let e = env().await;
    let c = hypothesis(&e, "fable-moss", "fable_moss", ADDR[3]).await;
    assert_eq!(
        e.kol_status("fable-moss").await,
        KolWalletStatus::Hypothesis
    );

    let public = e.w.status(&Person::NOBODY, "fable-moss").await.unwrap();
    assert_eq!(
        (public.status, public.main_address.clone()),
        (KolWalletStatus::Hypothesis, None)
    );
    assert!(!serde_json::to_string(&public).unwrap().contains(ADDR[3]));
    let api = e
        .ctx
        .apis
        .get::<dyn WalletsApi>()
        .unwrap()
        .pick(&Person::NOBODY);
    let via_api = api.status(&Person::NOBODY, "fable-moss").await.unwrap();
    assert_eq!(via_api.main_address, None);
    assert_eq!(
        api.status(&Person::NOBODY, "not-in-the-network").await,
        Err(WalletsError::NotMember {
            kol: "not-in-the-network".into()
        })
    );
    assert!(
        !serde_json::to_string(&*e.registry.kols.lock().unwrap())
            .unwrap()
            .contains(ADDR[3])
    );
    assert!(e.registry.publishes.lock().unwrap().is_empty());

    assert!(matches!(
        e.w.overview(&Person::NOBODY).await,
        Err(Error::Forbidden)
    ));
    for (method, uri) in [
        ("GET", "/api/team/wallets".to_string()),
        ("POST", "/api/team/wallets/pass".to_string()),
        ("POST", "/api/team/wallets/propose".to_string()),
        ("POST", format!("/api/team/wallets/{}/confirm", c.id)),
        ("POST", format!("/api/team/wallets/{}/reject", c.id)),
        ("POST", format!("/api/team/wallets/{}/revoke", c.id)),
        ("POST", format!("/api/team/wallets/{}/main", c.id)),
    ] {
        let body =
            json!({ "kol": "fable-moss", "address": ADDR[3], "evidence": "x", "where_when": "y" });
        let (status, text) = e.call(Person::NOBODY, method, &uri, Some(body)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        assert!(!text.contains(ADDR[3]), "{uri}: {text}");
    }
    let (status, text) = e.call(STAFF, "GET", "/api/team/wallets", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        text.contains(ADDR[3]) && text.contains("hypothesis"),
        "staff see it: {text}"
    );
}

// ----- verified handles -----------------------------------------------------------------

#[tokio::test]
async fn a_verified_handle_confirms_a_label_match_and_a_name_match_stays_a_hypothesis() {
    let e = env().await;
    // Solana Tracker identity that also carries the FOMO handle
    e.add("gale-ostrin", "gale_ostrin", &[]);
    e.st.identity("gale_ostrin", ADDR[0], Some("@Gale_Ostrin"));
    e.pass().await;
    let c = e.cand("gale-ostrin", ADDR[0]).await;
    assert_eq!(
        (c.status, c.source),
        (Hypothesis, CandidateSource::Fomo),
        "handle not verified yet"
    );
    assert_eq!(e.registry.main_of("gale-ostrin"), None);

    e.roster.verify("gale-ostrin", "gale_ostrin");
    e.ctx
        .events
        .emit(&XHandleVerified {
            kol: "gale-ostrin".into(),
        })
        .await
        .unwrap();
    let c = e.cand("gale-ostrin", ADDR[0]).await;
    assert_eq!((c.status, c.main), (Confirmed, true));
    assert_eq!(
        c.reason.as_deref(),
        Some("verified X handle equals the wallet label")
    );
    assert_eq!(e.registry.main_of("gale-ostrin").as_deref(), Some(ADDR[0]));

    // KOL Explorer label with a verified handle: confirmed straight away
    e.add("hollis-brae", "hollis_brae", &[]);
    e.roster.verify("hollis-brae", "hollis_brae");
    e.ke.label("hollis_brae", ADDR[1]);
    e.ctx
        .events
        .emit(&XHandleVerified {
            kol: "hollis-brae".into(),
        })
        .await
        .unwrap();
    let c = e.cand("hollis-brae", ADDR[1]).await;
    assert_eq!(
        (c.status, c.source),
        (Confirmed, CandidateSource::Kolexplorer)
    );
    assert_eq!(
        e.registry.get("hollis-brae").main_wallet.unwrap().source,
        WalletSource::Kolexplorer
    );

    // not a Solana address: ignored
    e.add("indigo-tarn", "indigo_tarn", &[]);
    e.roster.verify("indigo-tarn", "indigo_tarn");
    e.ke.label("indigo_tarn", "0x5b9e3c1a7d2f4086be13a9c74d0e58f26a1b3c97");
    let _ = e
        .ctx
        .events
        .emit(&XHandleVerified {
            kol: "indigo-tarn".into(),
        })
        .await;
    assert!(e.all("indigo-tarn").await.is_empty());
}

#[tokio::test]
async fn the_event_handlers_respect_the_shared_lock() {
    let e = env().await;
    e.add("juno-skerry", "juno_skerry", &[ADDR[0]]);
    e.st.trades(ADDR[0]);

    // the roster emits table_read while it holds the lock: the pass must not wait for it
    let events = e.ctx.events.clone();
    run_locked(
        e.db.pool(),
        vec![LockSpec::try_once(DATA_LOCK)],
        async move {
            events.emit(&TableRead { date: date() }).await.unwrap();
        },
    )
    .await
    .unwrap();
    assert_eq!(e.status_of("juno-skerry", ADDR[0]).await, Confirmed);

    // a staff action from outside takes the lock, and a held lock answers busy
    e.roster.verify("juno-skerry", "juno_skerry");
    let events = e.ctx.events.clone();
    let failed = run_locked(
        e.db.pool(),
        vec![LockSpec::try_once(DATA_LOCK)],
        async move {
            events
                .emit(&XHandleVerified {
                    kol: "juno-skerry".into(),
                })
                .await
        },
    )
    .await
    .unwrap();
    let err = failed.unwrap_err();
    assert!(err.to_string().contains("collection running"), "{err}");

    let app = e.app.clone();
    let (status, text) = run_locked(
        e.db.pool(),
        vec![LockSpec::try_once(DATA_LOCK)],
        async move { send(app, STAFF, "POST", "/api/team/wallets/pass", None).await },
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{text}");
}

// ----- conflicts ------------------------------------------------------------------------

#[tokio::test]
async fn a_shared_wallet_is_a_conflict_for_both_and_neither_is_published() {
    let e = env().await;
    e.add("kestrel-dun", "kestrel_dun", &[ADDR[0]]);
    e.add("lark-ember", "lark_ember", &[ADDR[0]]);
    e.st.trades(ADDR[0]);
    e.pass().await;

    let (k, l) = (
        e.cand("kestrel-dun", ADDR[0]).await,
        e.cand("lark-ember", ADDR[0]).await,
    );
    assert_eq!((k.status, l.status), (Conflict, Conflict));
    assert_eq!(l.reason.as_deref(), Some("also claimed by kestrel-dun"));
    assert_eq!(k.reason.as_deref(), Some("also claimed by lark-ember"));
    assert_eq!(e.kol_status("kestrel-dun").await, KolWalletStatus::Conflict);
    assert_eq!(e.kol_status("lark-ember").await, KolWalletStatus::Conflict);
    assert_eq!(e.registry.main_of("kestrel-dun"), None);
    assert_eq!(e.registry.main_of("lark-ember"), None);
    assert_eq!(
        e.w.status(&Person::NOBODY, "lark-ember")
            .await
            .unwrap()
            .main_address,
        None
    );
    assert_eq!(
        e.actions("kestrel-dun"),
        ["found", "publish", "conflict", "publish"]
    );
    assert_eq!(e.actions("lark-ember"), ["found"]);

    // still claimed: staff cannot confirm it by hand, and a pass changes nothing
    assert!(matches!(
        e.w.confirm(&STAFF, l.id, "DM").await,
        Err(Error::Refused(_))
    ));
    e.pass().await;
    assert_eq!(e.status_of("lark-ember", ADDR[0]).await, Conflict);

    // staff decides for one of them
    e.w.reject(&STAFF, l.id, Some("belongs to kestrel-dun"))
        .await
        .unwrap();
    assert_eq!(e.status_of("lark-ember", ADDR[0]).await, Rejected);
    assert_eq!(e.status_of("kestrel-dun", ADDR[0]).await, Confirmed);
    assert_eq!(e.registry.main_of("kestrel-dun").as_deref(), Some(ADDR[0]));
    assert_eq!(e.registry.main_of("lark-ember"), None);
}

#[tokio::test]
async fn an_address_the_registry_already_gives_to_someone_else_is_a_conflict() {
    let e = env().await;
    e.registry
        .put("owner-one", "Owner One", Some("owner_one"), None);
    e.registry
        .set_main("owner-one", ADDR[0], WalletSource::Kolexplorer);
    e.add("mica-thorn", "mica_thorn", &[ADDR[0]]);
    e.st.trades(ADDR[0]);
    e.pass().await;
    let c = e.cand("mica-thorn", ADDR[0]).await;
    assert_eq!(c.status, Conflict);
    assert_eq!(c.reason.as_deref(), Some("also claimed by owner-one"));
    assert_eq!(
        e.registry.main_of("owner-one").as_deref(),
        Some(ADDR[0]),
        "their wallet is untouched"
    );
    assert_eq!(e.registry.main_of("mica-thorn"), None);
}

#[tokio::test]
async fn a_main_wallet_that_differs_from_kol_explorers_is_a_conflict() {
    let e = env().await;
    e.add("nox-fennel", "nox_fennel", &[ADDR[1]]);
    e.registry
        .set_main("nox-fennel", ADDR[0], WalletSource::Kolexplorer);
    e.st.trades(ADDR[1]);
    e.pass().await;
    let c = e.cand("nox-fennel", ADDR[1]).await;
    assert_eq!(c.status, Conflict);
    assert!(
        c.reason
            .unwrap()
            .contains("KOL Explorer publishes another main wallet")
    );
    assert_eq!(
        e.registry.main_of("nox-fennel").as_deref(),
        Some(ADDR[0]),
        "KOL Explorer's main stays"
    );

    // the same address as KOL Explorer's main: confirmed, nothing to overwrite
    e.add("olive-wren", "olive_wren", &[ADDR[2]]);
    e.registry
        .set_main("olive-wren", ADDR[2], WalletSource::Kolexplorer);
    e.st.trades(ADDR[2]);
    e.pass().await;
    assert_eq!(e.status_of("olive-wren", ADDR[2]).await, Confirmed);
    assert_eq!(
        e.registry.get("olive-wren").main_wallet.unwrap().source,
        WalletSource::Kolexplorer
    );
}

#[tokio::test]
async fn a_revoked_network_wallet_next_to_kol_explorers_main_leaves_the_registry() {
    let e = env().await;
    e.add("pia-larch", "pia_larch", &[ADDR[3], ADDR[4]]);
    e.registry
        .set_main("pia-larch", ADDR[3], WalletSource::Kolexplorer);
    e.st.trades(ADDR[3]);
    e.st.trades(ADDR[4]);
    e.pass().await;
    let k = e.registry.get("pia-larch");
    assert_eq!(k.main_wallet.unwrap().source, WalletSource::Kolexplorer);
    let other: Vec<_> = k.other_wallets.iter().map(|w| (w.address.as_str(), w.source)).collect();
    assert_eq!(other, [(ADDR[4], WalletSource::Network)]);

    let id = e.cand("pia-larch", ADDR[4]).await.id;
    e.w.revoke(&STAFF, id, "not his wallet").await.unwrap();
    let k = e.registry.get("pia-larch");
    assert_eq!(k.main_wallet.unwrap().address, ADDR[3]);
    assert!(k.other_wallets.is_empty(), "{:?}", k.other_wallets);
}

// ----- staff proposals and the main wallet -----------------------------------------------

#[tokio::test]
async fn an_address_the_kol_sent_is_checked_for_trades_like_a_sheet_address() {
    let e = env().await;
    e.add("pike-ash", "pike_ash", &[]);
    // the Wallets screen's «add address from KOL» (POST /api/team/wallets/propose)
    let api = e.w.clone();

    e.st.trades(ADDR[0]);
    let c = api
        .propose(&STAFF, "pike-ash", ADDR[0], "sent in a DM, 3 March")
        .await
        .unwrap();
    assert_eq!(
        (c.status, c.source, c.main),
        (Confirmed, CandidateSource::FromKol, true)
    );
    assert_eq!(c.sent_by.as_deref(), Some("staff:1"));
    assert_eq!(c.evidence.as_deref(), Some("sent in a DM, 3 March"));
    assert_eq!(e.registry.main_of("pike-ash").as_deref(), Some(ADDR[0]));

    e.st.no_trades(ADDR[1]);
    let c = api
        .propose(&STAFF, "pike-ash", ADDR[1], "sent in a DM, 4 March")
        .await
        .unwrap();
    assert_eq!(c.status, NotFound);
    assert_eq!(
        c.reason.as_deref(),
        Some("no trades found for this address")
    );
    assert_eq!(e.registry.others_of("pike-ash"), Vec::<String>::new());

    assert!(matches!(
        api.propose(&STAFF, "pike-ash", "nonsense", "x").await,
        Err(Error::InvalidAddress(_))
    ));
    assert!(matches!(
        api.propose(&STAFF, "pike-ash", ADDR[2], " ").await,
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        api.propose(&STAFF, "nobody-here", ADDR[2], "x").await,
        Err(Error::NotMember(_))
    ));
    assert!(matches!(
        api.propose(&Person::NOBODY, "pike-ash", ADDR[2], "x").await,
        Err(Error::Forbidden)
    ));

    // staff picks which confirmed wallet is the main one
    e.st.trades(ADDR[3]);
    api.propose(&STAFF, "pike-ash", ADDR[3], "second wallet")
        .await
        .unwrap();
    assert_eq!(e.registry.others_of("pike-ash"), [ADDR[3]]);
    let second = e.cand("pike-ash", ADDR[3]).await;
    let (status, text) = e
        .call(
            STAFF,
            "POST",
            &format!("/api/team/wallets/{}/main", second.id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(e.registry.main_of("pike-ash").as_deref(), Some(ADDR[3]));
    assert_eq!(e.registry.others_of("pike-ash"), [ADDR[0]]);
    assert!(e.actions("pike-ash").contains(&"main".to_string()));
    let hypothesis_row = e.cand("pike-ash", ADDR[1]).await;
    let (status, _) = e
        .call(
            STAFF,
            "POST",
            &format!("/api/team/wallets/{}/main", hypothesis_row.id),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "only a confirmed wallet can be main"
    );
}

/// The FOMO match proposes through the API: a hypothesis with source `fomo`, journaled,
/// never published; an address the KOL already has stays as it is.
#[tokio::test]
async fn a_fomo_match_adds_an_unpublished_hypothesis_through_the_api() {
    let e = env().await;
    e.add("tern-holt", "tern_holt", &[ADDR[1]]);
    e.st.trades(ADDR[1]);
    e.pass().await;
    let api = e.ctx.apis.get::<dyn WalletsApi>().unwrap().pick(&STAFF);

    let c = api
        .propose(&STAFF, "tern-holt", ADDR[0], "FOMO match: 3 positions")
        .await
        .unwrap();
    assert_eq!(
        (c.status, c.source, c.main),
        (Hypothesis, CandidateSource::Fomo, false)
    );
    assert_eq!(c.evidence.as_deref(), Some("FOMO match: 3 positions"));
    assert_eq!(c.sent_by.as_deref(), Some("staff:1"));
    assert!(e.actions("tern-holt").contains(&"fomo".to_string()));
    // never published: the sheet wallet stays the only published one
    assert_eq!(e.registry.main_of("tern-holt").as_deref(), Some(ADDR[1]));
    assert_eq!(e.registry.others_of("tern-holt"), Vec::<String>::new());
    assert_eq!(
        e.w.status(&STAFF, "tern-holt")
            .await
            .unwrap()
            .main_address
            .as_deref(),
        Some(ADDR[1])
    );

    // the same match again changes nothing
    let journal_len = e.actions("tern-holt").len();
    let again = api
        .propose(&STAFF, "tern-holt", ADDR[0], "FOMO match: 2 positions")
        .await
        .unwrap();
    assert_eq!((again.id, again.status), (c.id, Hypothesis));
    assert_eq!(e.actions("tern-holt").len(), journal_len);

    // a confirmed address is not turned back into a hypothesis
    let known = api
        .propose(&STAFF, "tern-holt", ADDR[1], "FOMO match: 2 positions")
        .await
        .unwrap();
    assert_eq!(
        (known.status, known.source),
        (Confirmed, CandidateSource::Sheet)
    );

    // the hypothesis follows the usual machine: the KOL confirms it
    let done =
        e.w.confirm(&STAFF, c.id, "Telegram, 9 March")
            .await
            .unwrap();
    assert_eq!(done.status, Confirmed);

    assert!(matches!(
        api.propose(&STAFF, "tern-holt", "nonsense", "x").await,
        Err(WalletsError::InvalidAddress { .. })
    ));
    assert!(matches!(
        api.propose(&STAFF, "nobody-here", ADDR[2], "x").await,
        Err(WalletsError::NotMember { .. })
    ));
    assert!(matches!(
        api.propose(&STAFF, "tern-holt", ADDR[2], "  ").await,
        Err(WalletsError::Internal { .. })
    ));
    assert!(matches!(
        api.propose(&Person::NOBODY, "tern-holt", ADDR[2], "x")
            .await,
        Err(WalletsError::Internal { .. })
    ));
    // the shared lock is held: busy, nothing written
    let api2 = api.clone();
    let busy = run_locked(
        e.db.pool(),
        vec![LockSpec::try_once(DATA_LOCK)],
        async move { api2.propose(&STAFF, "tern-holt", ADDR[2], "x").await },
    )
    .await
    .unwrap();
    assert!(matches!(busy, Err(WalletsError::Busy)), "{busy:?}");
    assert!(
        e.all("tern-holt")
            .await
            .iter()
            .all(|c| c.address != ADDR[2])
    );
}

#[tokio::test]
async fn the_staff_routes_show_the_overview_and_run_the_pass() {
    let e = env().await;
    e.add("quill-reed", "quill_reed", &[ADDR[0]]);
    e.add("rook-sable", "rook_sable", &[ADDR[1]]);
    e.st.trades(ADDR[0]);
    e.st.no_trades(ADDR[1]);
    let (status, text) = e.call(STAFF, "POST", "/api/team/wallets/pass", None).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let body: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["problems"], Value::Null);
    let counts = &body["overview"]["counts"];
    assert_eq!(
        (counts["confirmed"].as_u64(), counts["not_found"].as_u64()),
        (Some(1), Some(1))
    );
    let (status, text) = e.call(STAFF, "GET", "/api/team/wallets", None).await;
    assert_eq!(status, StatusCode::OK);
    let overview: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(overview["kols"].as_array().unwrap().len(), 2);
    assert_eq!(overview["kols"][0]["main_address"], ADDR[0]);
    let (status, _) = e
        .call(
            STAFF,
            "POST",
            "/api/team/wallets/9999/revoke",
            Some(json!({ "reason": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ----- day-0 data -----------------------------------------------------------------------

#[tokio::test]
async fn the_prototype_wallets_are_there_on_day_zero() {
    let e = env().await;
    let rows: Vec<(String, String, bool)> =
        sqlx::query_as("SELECT status, source, main FROM wallets.candidate")
            .fetch_all(e.db.pool())
            .await
            .unwrap();
    assert_eq!(rows.len(), 22);
    assert!(
        rows.iter()
            .all(|r| *r == ("confirmed".into(), "prototype".into(), true))
    );
    assert!(e.plugin.wallets().overview(&SYSTEM).await.is_ok());
}
