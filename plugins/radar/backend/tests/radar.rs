use api_registry::{KolProfile, KolUpsert, Reach, RegistryApi, RegistryError, WalletSource};
use api_snapshots::{
    Member, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotStatus, SnapshotView,
    SnapshotsApi, SnapshotsError,
};
use api_solanatracker::{
    KolList, ListedKol, SolanatrackerApi, StError, StRun, StWallet, WalletSummary,
};
use arena_platform::{Api, Apis, Person, Registry, Route, Tag};
use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use chrono::{DateTime, NaiveDate, Utc};
use plugin_radar::{Radar, routes};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

const W_DECU: &str = "4vw54BmAogeRV3vPKWyFjzo2i2uLyqPEKvmbWX7DHZRe";
const W_ANSEM: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const W_NEW: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
const W_NEW2: &str = "DfMxre4cKmvogbLrPigxmibVTTQDuzjdXojWzjCXXhzj";
const W_NOHANDLE: &str = "GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ";

struct FakeSt {
    list: Mutex<Option<KolList>>,
}

#[async_trait]
impl SolanatrackerApi for FakeSt {
    async fn run(&self, _: &Person, _: NaiveDate) -> Result<StRun, StError> {
        Err(StError::NotBuilt)
    }
    async fn kol_list(&self, _: &Person, date: NaiveDate) -> Result<Option<KolList>, StError> {
        Ok(self.list.lock().unwrap().clone().filter(|l| l.date <= date))
    }
    async fn wallet_by_handle(&self, _: &Person, _: &str) -> Result<Option<StWallet>, StError> {
        Ok(None)
    }
    async fn wallet_summary(&self, _: &Person, _: &str) -> Result<WalletSummary, StError> {
        Err(StError::NotBuilt)
    }
}

struct FakeSnapshots {
    members: Vec<&'static str>,
    dates: Vec<NaiveDate>,
}

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
        _: &[String],
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
        if !self.dates.contains(&date) {
            return Err(SnapshotsError::NoSnapshot { date });
        }
        Ok(SnapshotView {
            snapshot: Snapshot {
                date,
                status: SnapshotStatus::Closed,
                opened_at: Utc::now(),
                closed_at: Some(Utc::now()),
            },
            members: self
                .members
                .iter()
                .map(|k| Member {
                    kol: k.to_string(),
                    reasons: vec![MemberReason::Top100],
                    row: None::<MetricRow>,
                })
                .collect(),
        })
    }
    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Ok(self.dates.last().copied())
    }
    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Ok(self.dates.clone())
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

/// Knows which wallet and handle belong to which KOL id.
struct FakeRegistry;

fn off() -> RegistryError {
    RegistryError::NotBuilt
}

#[async_trait]
impl RegistryApi for FakeRegistry {
    async fn upsert_kol(&self, _: &Person, _: &KolUpsert) -> Result<String, RegistryError> {
        Err(off())
    }
    async fn get_kol(&self, _: &Person, _: &str) -> Result<Option<KolProfile>, RegistryError> {
        Err(off())
    }
    async fn list_kols(&self, _: &Person, _: &[String]) -> Result<Vec<KolProfile>, RegistryError> {
        Err(off())
    }
    async fn find_by_handle(&self, _: &Person, x: &str) -> Result<Option<String>, RegistryError> {
        Ok(match x.to_lowercase().as_str() {
            "ansem" => Some("ansem".to_string()),
            _ => None,
        })
    }
    async fn find_by_wallet(&self, _: &Person, a: &str) -> Result<Option<String>, RegistryError> {
        Ok((a == W_DECU).then(|| "decu".to_string()))
    }
    async fn publish_wallets(
        &self,
        _: &Person,
        _: &str,
        _: Option<&str>,
        _: &[String],
        _: WalletSource,
    ) -> Result<(), RegistryError> {
        Err(off())
    }
    async fn set_reach(
        &self,
        _: &Person,
        _: &str,
        _: &Reach,
        _: &str,
        _: NaiveDate,
    ) -> Result<(), RegistryError> {
        Err(off())
    }
    async fn set_formats(&self, _: &Person, _: &str, _: &[String]) -> Result<(), RegistryError> {
        Err(off())
    }
}

fn listed(name: Option<&str>, handle: Option<&str>, wallet: &str) -> ListedKol {
    ListedKol {
        name: name.map(str::to_string),
        x_handle: handle.map(str::to_string),
        wallet: wallet.to_string(),
    }
}

fn list_of(date: NaiveDate, kols: Vec<ListedKol>) -> Option<KolList> {
    Some(KolList {
        date,
        fetched_at: Utc::now(),
        kols,
    })
}

fn route(list: Option<KolList>) -> Route {
    let apis = Apis::new(Registry::new("test", &[]));
    apis.provide::<dyn SolanatrackerApi>(
        "solanatracker",
        vec![(
            "default",
            Arc::new(FakeSt {
                list: Mutex::new(list),
            }) as Arc<dyn SolanatrackerApi>,
        )],
        Arc::new(api_solanatracker::stub::Stub),
    )
    .unwrap();
    apis.provide::<dyn SnapshotsApi>(
        "snapshots",
        vec![(
            "default",
            Arc::new(FakeSnapshots {
                members: vec!["decu", "ansem"],
                dates: vec![day(19), day(20)],
            }) as Arc<dyn SnapshotsApi>,
        )],
        Arc::new(api_snapshots::stub::Stub),
    )
    .unwrap();
    apis.provide::<dyn RegistryApi>(
        "registry",
        vec![("default", Arc::new(FakeRegistry) as Arc<dyn RegistryApi>)],
        Arc::new(api_registry::stub::Stub),
    )
    .unwrap();
    fn get<T: ?Sized + Send + Sync + 'static>(a: &Apis) -> Api<T> {
        a.get().unwrap()
    }
    let radar = Radar::new(get(&apis), get(&apis), get(&apis));
    routes(&radar).remove(0)
}

async fn get_json(route: &Route, uri: &str) -> (StatusCode, Value) {
    let mut req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    req.extensions_mut().insert(Person::NOBODY);
    let res = route.handler.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn names(v: &Value) -> Vec<&str> {
    v["kols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn lists_todays_kols_that_are_not_on_the_board() {
    let r = route(list_of(
        day(20),
        vec![
            listed(Some("Decu"), Some("decu"), W_DECU), // on the board by wallet
            listed(Some("Ansem"), Some("@Ansem"), W_ANSEM), // on the board by handle, any case
            listed(Some("Newcomer"), Some("newcomer"), W_NEW),
            listed(None, Some("second_new"), W_NEW2), // no name: handle
            listed(None, None, W_NOHANDLE),           // neither: short address
            listed(Some("Newcomer again"), None, W_NEW), // same wallet twice
            listed(Some("Broken"), None, "not-an-address"),
        ],
    ));

    let (status, v) = get_json(&r, "/api/radar?date=2026-10-20").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "ok");
    assert_eq!(v["date"], "2026-10-20");
    assert_eq!(v["list_date"], "2026-10-20");
    assert_eq!(names(&v), ["Newcomer", "@second_new", "GThU…hFMJ"]);
    assert_eq!(v["kols"][0]["wallet"], W_NEW);
    assert_eq!(v["kols"][0]["x_handle"], "newcomer");
}

#[tokio::test]
async fn the_date_defaults_to_the_latest_snapshot() {
    let r = route(list_of(
        day(20),
        vec![listed(Some("Newcomer"), Some("newcomer"), W_NEW)],
    ));
    let (status, v) = get_json(&r, "/api/radar").await;
    assert_eq!(
        (status, v["date"].as_str()),
        (StatusCode::OK, Some("2026-10-20"))
    );
}

#[tokio::test]
async fn a_missing_list_for_the_day_says_not_updated_with_the_last_list_date() {
    let r = route(list_of(
        day(18),
        vec![listed(Some("Newcomer"), Some("newcomer"), W_NEW)],
    ));

    let (status, v) = get_json(&r, "/api/radar?date=2026-10-20").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["status"], "not_updated");
    assert_eq!(v["list_date"], "2026-10-18");
    assert!(
        v["kols"].as_array().unwrap().is_empty(),
        "no stale KOLs shown"
    );
}

#[tokio::test]
async fn no_list_at_all_says_not_updated_without_a_date() {
    let r = route(None);
    let (_, v) = get_json(&r, "/api/radar?date=2026-10-20").await;
    assert_eq!(v["status"], "not_updated");
    assert!(v["list_date"].is_null());
}

#[tokio::test]
async fn everyone_on_the_board_gives_an_empty_ok_radar() {
    let r = route(list_of(
        day(20),
        vec![listed(Some("Decu"), Some("decu"), W_DECU)],
    ));
    let (_, v) = get_json(&r, "/api/radar?date=2026-10-20").await;
    assert_eq!(v["status"], "ok");
    assert!(v["kols"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn bad_input_is_refused() {
    let r = route(None);
    let (status, v) = get_json(&r, "/api/radar?date=20.10.2026").await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_date"))
    );
    let (status, v) = get_json(&r, "/api/radar?date=2026-10-01").await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("no_snapshot"))
    );
}

#[tokio::test]
async fn the_route_is_public_and_returns_only_public_keys() {
    let r = route(list_of(
        day(20),
        vec![listed(Some("Newcomer"), Some("newcomer"), W_NEW)],
    ));
    assert_eq!(r.tag, Tag::Public);
    let (_, v) = get_json(&r, "/api/radar?date=2026-10-20").await;
    let keys = |o: &Value| -> BTreeSet<String> { o.as_object().unwrap().keys().cloned().collect() };
    let set = |a: &[&str]| -> BTreeSet<String> { a.iter().map(|s| s.to_string()).collect() };
    assert_eq!(keys(&v), set(&["date", "status", "list_date", "kols"]));
    assert_eq!(
        keys(&v["kols"][0]),
        set(&["id", "name", "x_handle", "wallet"])
    );
}
