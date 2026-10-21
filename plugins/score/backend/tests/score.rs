//! Score over fake snapshots and registry APIs (a plugin backend never depends on another
//! plugin's backend). The day-0 snapshot is the fixture the snapshots tests check the seed
//! migration against, so this ranks exactly what the seeded database holds.

use api_registry::{
    KolId, KolLinks, KolProfile, KolUpsert, Reach, RegistryApi, RegistryError, WalletSource,
};
use api_score::{Badge, PartKind, PartStatus, ScoreApi, ScoreError, Tier};
use api_snapshots::{
    Member, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotStatus, SnapshotView,
    SnapshotsApi, SnapshotsError,
};
use arena_platform::{Config, Ctx, Person, testing::TestDb};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use plugin_score::{DAY0, MIGRATOR, NAME};
use std::{collections::HashMap, sync::Arc};

const P: Person = Person::SYSTEM;
const DAY0_VIEW: &str =
    include_str!("../../../snapshots/backend/tests/fixtures/day0_2026-10-18.json");
const PROTOTYPE: &str = include_str!("../../../../data/prototype/arena_data_v2.json");
const EXPECTED: &str = include_str!("../../../../data/prototype/expected_ranking_2026-10-18.json");

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
        Err(SnapshotsError::NotBuilt)
    }
    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
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

/// Registry that knows names only.
struct FakeRegistry(HashMap<String, String>);

#[async_trait]
impl RegistryApi for FakeRegistry {
    async fn upsert_kol(&self, _: &Person, _: &KolUpsert) -> Result<KolId, RegistryError> {
        Err(RegistryError::NotBuilt)
    }
    async fn get_kol(&self, _: &Person, _: &str) -> Result<Option<KolProfile>, RegistryError> {
        Err(RegistryError::NotBuilt)
    }
    async fn list_kols(&self, _: &Person, ids: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        Ok(ids
            .iter()
            .filter_map(|id| {
                self.0.get(id).map(|name| KolProfile {
                    id: id.clone(),
                    name: name.clone(),
                    x_handle: None,
                    links: KolLinks::default(),
                    main_wallet: None,
                    other_wallets: Vec::new(),
                    reach: None,
                    formats: Vec::new(),
                })
            })
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

fn day0_view() -> SnapshotView {
    serde_json::from_str(DAY0_VIEW).unwrap()
}

/// Prototype slug -> name.
fn prototype_names() -> HashMap<String, String> {
    let v: serde_json::Value = serde_json::from_str(PROTOTYPE).unwrap();
    v["kols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            (
                k["slug"].as_str().unwrap().to_string(),
                k["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// The score plugin built on a fresh test database with the given fake views and names,
/// as the server builds it. Returns the database, context, plugin and its API.
async fn setup(
    views: Vec<SnapshotView>,
    names: HashMap<String, String>,
) -> (
    TestDb,
    Ctx,
    Arc<dyn arena_platform::Plugin>,
    Arc<dyn ScoreApi>,
) {
    let db = TestDb::with_plugins(&[(NAME, &MIGRATOR)]).await;
    let ctx = Ctx::new(db.pool().clone(), Config::default(), "test").unwrap();
    let views = views.into_iter().map(|v| (v.snapshot.date, v)).collect();
    ctx.apis
        .provide::<dyn SnapshotsApi>(
            "snapshots",
            vec![("default", Arc::new(FakeSnapshots(views)))],
            Arc::new(api_snapshots::stub::Stub),
        )
        .unwrap();
    ctx.apis
        .provide::<dyn RegistryApi>(
            "registry",
            vec![("default", Arc::new(FakeRegistry(names)))],
            Arc::new(api_registry::stub::Stub),
        )
        .unwrap();
    let plugin = (plugin_score::ENTRY.build)(&ctx).unwrap();
    let api = ctx.apis.require::<dyn ScoreApi>("test").unwrap().pick(&P);
    (db, ctx, plugin, api)
}

#[tokio::test]
async fn day0_ranking_equals_the_pinned_fixture() {
    let names = prototype_names();
    let (db, ctx, plugin, api) = setup(vec![day0_view()], names.clone()).await;
    plugin.install(&ctx).await.unwrap();
    let ranking = api.ranking(&P, DAY0).await.unwrap();
    assert_eq!(ranking.rows.len(), 122);

    // Full 122-row ranking.
    let expected: Vec<serde_json::Value> = serde_json::from_str(EXPECTED).unwrap();
    assert_eq!(expected.len(), 122);
    for (row, want) in ranking.rows.iter().zip(&expected) {
        let tier = match row.card.tier {
            Tier::Goat => "GOAT",
            Tier::Diamond => "Diamond",
            Tier::Sendoor => "Sendoor",
            Tier::Degen => "Degen",
        };
        let got = serde_json::json!({
            "rank": row.rank,
            "slug": row.kol,
            "name": names[&row.kol],
            "score": row.card.score,
            "tier": tier,
        });
        assert_eq!(&got, want);
    }

    // Control example.
    let t = ranking.rows.iter().find(|r| r.kol == "tekkerrss").unwrap();
    assert_eq!((t.rank, t.card.score, t.card.tier), (18, 55, Tier::Diamond));

    // The 25 values from the prototype screenshots (rank, name, Score).
    let screenshots: [(u32, &str, u8); 25] = [
        (1, "Decu", 95),
        (2, "Cooker", 88),
        (3, "Trunoest", 85),
        (18, "Tekkerrss", 55),
        (37, "Peter", 47),
        (52, "Levis", 41),
        (56, "Iced", 39),
        (64, "laseasy10", 36),
        (66, "BTCFORLIFE", 36),
        (67, "juicy", 35),
        (73, "Naka", 31),
        (74, "trancey", 31),
        (93, "shah", 23),
        (101, "kook", 15),
        (102, "babatooonde", 12),
        (103, "TheSOLstice", 12),
        (104, "Mrpunk.eth", 11),
        (105, "BrazilZZ", 11),
        (107, "ocr", 10),
        (108, "Smokey", 9),
        (109, "Maurits", 8),
        (110, "Zagrebian", 8),
        (111, "J777Crypto", 8),
        (117, "pow", 5),
        (120, "NDF_Sonar_trade", 0),
    ];
    for (rank, name, score) in screenshots {
        let row = &ranking.rows[rank as usize - 1];
        assert_eq!(
            (row.rank, names[&row.kol].as_str(), row.card.score),
            (rank, name, score)
        );
    }

    // Parts survive storage, with their «no data» marks: Tekkerrss has no d1/wr30 and
    // its accuracy is the all-time win rate.
    let card = api
        .score(
            &P,
            &day0_view()
                .members
                .iter()
                .find(|m| m.kol == "tekkerrss")
                .unwrap()
                .row
                .as_ref()
                .unwrap()
                .metrics,
        )
        .await
        .unwrap();
    assert_eq!(t.card, card);
    assert!(t.card.parts[1].all_time);
    assert_eq!(t.card.parts[1].kind, PartKind::Accuracy);
    assert_eq!(t.card.badges, [Badge::Whale]);
    db.close().await;
}

#[tokio::test]
async fn install_ranks_day0_only_once() {
    let (db, ctx, plugin, api) = setup(vec![day0_view()], prototype_names()).await;
    assert_eq!(
        api.ranking(&P, DAY0).await,
        Err(ScoreError::NotRanked { date: DAY0 })
    );
    plugin.install(&ctx).await.unwrap();
    let first = api.ranking(&P, DAY0).await.unwrap();
    plugin.install(&ctx).await.unwrap();
    let second = api.ranking(&P, DAY0).await.unwrap();
    assert_eq!(first.ranked_at, second.ranked_at);
    // rank() itself re-ranks and replaces.
    api.rank(&P, DAY0).await.unwrap();
    let third = api.ranking(&P, DAY0).await.unwrap();
    assert!(third.ranked_at >= first.ranked_at);
    assert_eq!(third.rows, first.rows);
    db.close().await;
}

fn member(kol: &str, metrics: Option<Metrics>, date: NaiveDate) -> Member {
    Member {
        kol: kol.into(),
        reasons: vec![MemberReason::Top100],
        row: metrics.map(|metrics| MetricRow {
            date,
            kol: kol.into(),
            source: MetricSource::Kolexplorer,
            fetched_at: "2026-10-20T06:00:00Z".parse().unwrap(),
            metrics,
        }),
    }
}

fn view(date: NaiveDate, status: SnapshotStatus, members: Vec<Member>) -> SnapshotView {
    let t: DateTime<Utc> = "2026-10-20T06:00:00Z".parse().unwrap();
    SnapshotView {
        snapshot: Snapshot {
            date,
            status,
            opened_at: t,
            closed_at: Some(t),
        },
        members,
    }
}

#[tokio::test]
async fn tie_order_score_then_d30_then_name() {
    let date = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
    // Activity alone gives 15 points; d30 below $1,000 adds nothing to Score but orders ties.
    let m = |d30: Option<f64>| Metrics {
        d30,
        last_min_at_fetch: Some(1),
        ..Metrics::default()
    };
    let members = vec![
        member("a-low", Some(m(Some(10.0))), date),
        member("b-none", Some(m(None)), date),
        member("c-zed", Some(m(Some(500.0))), date),
        member("d-alpha", Some(m(Some(500.0))), date),
        member(
            "e-top",
            Some(Metrics {
                d30: Some(1_000_000.0),
                ..m(None)
            }),
            date,
        ),
        member("f-norow", None, date),
    ];
    let names = [
        ("a-low", "Low"),
        ("b-none", "None"),
        ("c-zed", "Zed"),
        ("d-alpha", "alpha"),
        ("e-top", "Top"),
        ("f-norow", "Nobody"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    let (db, _ctx, _plugin, api) =
        setup(vec![view(date, SnapshotStatus::Closed, members)], names).await;
    api.rank(&P, date).await.unwrap();
    let r = api.ranking(&P, date).await.unwrap();
    let got: Vec<(&str, u8)> = r
        .rows
        .iter()
        .map(|r| (r.kol.as_str(), r.card.score))
        .collect();
    assert_eq!(
        got,
        [
            ("e-top", 55),
            // Equal Score and d30: name ascending, case-insensitive.
            ("d-alpha", 15),
            ("c-zed", 15),
            ("a-low", 15),
            ("b-none", 15),
            // A member without a row ranks with no data.
            ("f-norow", 0),
        ]
    );
    let ranks: Vec<u32> = r.rows.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, [1, 2, 3, 4, 5, 6]);
    let norow = &r.rows[5].card;
    assert!(norow.parts.iter().all(|p| p.status == PartStatus::NoData));
    db.close().await;
}

#[tokio::test]
async fn rank_needs_a_closed_snapshot() {
    let date = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
    let (db, _ctx, _plugin, api) = setup(
        vec![view(date, SnapshotStatus::Open, vec![])],
        HashMap::new(),
    )
    .await;
    assert_eq!(
        api.rank(&P, date).await,
        Err(ScoreError::NotClosed { date })
    );
    let other = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    assert_eq!(
        api.rank(&P, other).await,
        Err(ScoreError::NoSnapshot { date: other })
    );
    assert_eq!(
        api.ranking(&P, date).await,
        Err(ScoreError::NotRanked { date })
    );
    db.close().await;
}
