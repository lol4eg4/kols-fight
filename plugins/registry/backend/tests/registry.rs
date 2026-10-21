use api_registry::{KolLinks, KolUpsert, Reach, RegistryApi, RegistryError, WalletSource};
use arena_platform::{Person, testing::TestDb};
use chrono::NaiveDate;
use plugin_registry::{MIGRATOR, NAME, Registry};

const P: Person = Person::SYSTEM;
const TEKKERRSS_WALLET: &str = "7UBjMaYsE3tKcanfgGrtso6aHAMhVNSVHkkX5j3t7f5y";
const FRESH_A: &str = "11111111111111111111111111111111";
const FRESH_B: &str = "4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud8";

async fn setup() -> (TestDb, Registry) {
    let db = TestDb::with_plugins(&[(NAME, &MIGRATOR)]).await;
    let reg = Registry::new(db.pool().clone());
    (db, reg)
}

#[tokio::test]
async fn seed_holds_the_prototype_kols() {
    let (db, reg) = setup().await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM registry.kol")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 122);
    let t = reg.get_kol(&P, "tekkerrss").await.unwrap().unwrap();
    assert_eq!(t.name, "Tekkerrss");
    assert_eq!(t.x_handle.as_deref(), Some("Tekkerrss"));
    assert!(t.links.solanatracker);
    assert_eq!(t.main_wallet.as_ref().unwrap().address, TEKKERRSS_WALLET);
    assert_eq!(
        t.main_wallet.as_ref().unwrap().source,
        WalletSource::Prototype
    );
    let reach = t.reach.unwrap();
    assert_eq!(reach.reach.x_followers, Some(9060));
    assert_eq!(reach.source, "prototype");
    assert_eq!(t.formats, vec!["post".to_string()]);

    // Wallet from the saved KE page, matched by X handle.
    let decu = reg.get_kol(&P, "decu").await.unwrap().unwrap();
    let w = decu.main_wallet.unwrap();
    assert_eq!(w.address, "4vw54BmAogeRV3vPKWyFet5yf8DTLcREzdSzx4rw9Ud9");
    assert_eq!(w.source, WalletSource::Kolexplorer);
    assert_eq!(decu.links.kolexplorer.as_deref(), Some("decu"));

    // Other wallets.
    let j = reg.get_kol(&P, "j7").await.unwrap().unwrap();
    assert_eq!(j.other_wallets.len(), 1);
    assert_eq!(j.links.telegram.as_deref(), Some("J7tradingjournal"));

    // Every KOL has a main wallet on day 0; no wallet has two owners (primary key).
    let without: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM registry.kol k WHERE NOT EXISTS \
         (SELECT 1 FROM registry.wallet w WHERE w.kol = k.id AND w.main)",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(without, 0);

    // The seed is idempotent.
    let seed = include_str!("../../migrations/0003_seed.sql");
    sqlx::raw_sql(seed).execute(db.pool()).await.unwrap();
    let again: i64 = sqlx::query_scalar("SELECT count(*) FROM registry.wallet")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(again, 126);
    db.close().await;
}

#[tokio::test]
async fn finds_by_handle_and_wallet() {
    let (db, reg) = setup().await;
    assert_eq!(
        reg.find_by_handle(&P, "@tekkerrss")
            .await
            .unwrap()
            .as_deref(),
        Some("tekkerrss")
    );
    assert_eq!(reg.find_by_handle(&P, "nobody_here").await.unwrap(), None);
    assert_eq!(
        reg.find_by_wallet(&P, TEKKERRSS_WALLET)
            .await
            .unwrap()
            .as_deref(),
        Some("tekkerrss")
    );
    assert_eq!(reg.find_by_wallet(&P, FRESH_A).await.unwrap(), None);
    let list = reg
        .list_kols(&P, &["peter".into(), "no-such-kol".into(), "decu".into()])
        .await
        .unwrap();
    let ids: Vec<&str> = list.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(ids, ["peter", "decu"]);
    db.close().await;
}

#[tokio::test]
async fn upsert_makes_deduplicated_slugs_and_updates() {
    let (db, reg) = setup().await;
    let new = |name: &str| KolUpsert {
        id: None,
        name: name.into(),
        x_handle: Some("@newkol".into()),
        links: KolLinks::default(),
    };
    // "Decu" exists from the seed.
    assert_eq!(reg.upsert_kol(&P, &new("Decu")).await.unwrap(), "decu-2");
    assert_eq!(reg.upsert_kol(&P, &new("decu")).await.unwrap(), "decu-3");
    assert_eq!(
        reg.upsert_kol(&P, &new("New Kol!")).await.unwrap(),
        "new-kol"
    );
    let k = reg.get_kol(&P, "new-kol").await.unwrap().unwrap();
    assert_eq!(k.x_handle.as_deref(), Some("newkol"));

    let mut update = new("New Kol Renamed");
    update.id = Some("new-kol".into());
    update.links.fomo = Some("newfomo".into());
    assert_eq!(reg.upsert_kol(&P, &update).await.unwrap(), "new-kol");
    let k = reg.get_kol(&P, "new-kol").await.unwrap().unwrap();
    assert_eq!(k.name, "New Kol Renamed");
    assert_eq!(k.links.fomo.as_deref(), Some("newfomo"));

    update.id = Some("no-such-kol".into());
    assert!(matches!(
        reg.upsert_kol(&P, &update).await,
        Err(RegistryError::UnknownKol { .. })
    ));
    db.close().await;
}

#[tokio::test]
async fn publish_wallets_refuses_a_wallet_of_another_kol() {
    let (db, reg) = setup().await;
    let before = reg.get_kol(&P, "peter").await.unwrap().unwrap();
    let err = reg
        .publish_wallets(
            &P,
            "peter",
            Some((FRESH_A, WalletSource::Network)),
            &[(TEKKERRSS_WALLET.to_string(), WalletSource::Network)],
        )
        .await
        .unwrap_err();
    assert_eq!(
        err,
        RegistryError::WalletTaken {
            address: TEKKERRSS_WALLET.into(),
            owner: "tekkerrss".into()
        }
    );
    // Nothing changed.
    assert_eq!(reg.get_kol(&P, "peter").await.unwrap().unwrap(), before);
    assert_eq!(reg.find_by_wallet(&P, FRESH_A).await.unwrap(), None);

    // Bad address and unknown KOL.
    assert!(matches!(
        reg.publish_wallets(&P, "peter", Some(("0xdead", WalletSource::Network)), &[])
            .await,
        Err(RegistryError::InvalidAddress { .. })
    ));
    assert!(matches!(
        reg.publish_wallets(
            &P,
            "no-such-kol",
            Some((FRESH_A, WalletSource::Network)),
            &[]
        )
        .await,
        Err(RegistryError::UnknownKol { .. })
    ));
    db.close().await;
}

#[tokio::test]
async fn publish_wallets_replaces_the_kols_wallets() {
    let (db, reg) = setup().await;
    let old = reg
        .get_kol(&P, "tekkerrss")
        .await
        .unwrap()
        .unwrap()
        .main_wallet
        .unwrap();
    // Old main becomes another wallet and keeps its publish time.
    reg.publish_wallets(
        &P,
        "tekkerrss",
        Some((FRESH_A, WalletSource::Network)),
        &[
            (TEKKERRSS_WALLET.to_string(), WalletSource::Kolexplorer),
            (FRESH_B.to_string(), WalletSource::Network),
        ],
    )
    .await
    .unwrap();
    let t = reg.get_kol(&P, "tekkerrss").await.unwrap().unwrap();
    assert_eq!(t.main_wallet.as_ref().unwrap().address, FRESH_A);
    assert_eq!(
        t.main_wallet.as_ref().unwrap().source,
        WalletSource::Network
    );
    let others: Vec<&str> = t.other_wallets.iter().map(|w| w.address.as_str()).collect();
    assert_eq!(others, [TEKKERRSS_WALLET, FRESH_B]);
    assert_eq!(t.other_wallets[0].published_at, old.published_at);
    // Each wallet carries its own source.
    assert_eq!(t.other_wallets[0].source, WalletSource::Kolexplorer);
    assert_eq!(t.other_wallets[1].source, WalletSource::Network);

    // Now another KOL cannot take any of them; dropped ones are free again.
    reg.publish_wallets(&P, "tekkerrss", Some((FRESH_A, WalletSource::Network)), &[])
        .await
        .unwrap();
    assert!(matches!(
        reg.publish_wallets(&P, "peter", Some((FRESH_A, WalletSource::Network)), &[])
            .await,
        Err(RegistryError::WalletTaken { .. })
    ));
    reg.publish_wallets(
        &P,
        "peter",
        None,
        &[(FRESH_B.to_string(), WalletSource::Network)],
    )
    .await
    .unwrap();
    assert_eq!(
        reg.find_by_wallet(&P, FRESH_B).await.unwrap().as_deref(),
        Some("peter")
    );

    // None and no others unpublishes.
    reg.publish_wallets(&P, "tekkerrss", None, &[])
        .await
        .unwrap();
    let t = reg.get_kol(&P, "tekkerrss").await.unwrap().unwrap();
    assert!(t.main_wallet.is_none() && t.other_wallets.is_empty());
    db.close().await;
}

#[tokio::test]
async fn reach_and_formats() {
    let (db, reg) = setup().await;
    let date = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
    let reach = Reach {
        x_followers: Some(10_000),
        telegram_subscribers: None,
        pumpfun_followers: Some(5),
    };
    reg.set_reach(&P, "decu", &reach, "X profile", date)
        .await
        .unwrap();
    reg.set_formats(&P, "decu", &["post".into(), "stream".into()])
        .await
        .unwrap();
    let d = reg.get_kol(&P, "decu").await.unwrap().unwrap();
    let r = d.reach.unwrap();
    assert_eq!(
        (r.reach, r.source.as_str(), r.date),
        (reach, "X profile", date)
    );
    assert_eq!(d.formats, ["post", "stream"]);
    assert!(matches!(
        reg.set_formats(&P, "no-such-kol", &[]).await,
        Err(RegistryError::UnknownKol { .. })
    ));
    db.close().await;
}
