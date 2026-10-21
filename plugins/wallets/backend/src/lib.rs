//! Backend of the `wallets` plugin (layer 3): wallet candidates of network KOLs.
//!
//! State machine: see [`api_wallets`] and `machine.rs`. The pass runs when the roster has
//! read the sheet (`table_read`, under the roster's `arena_data` lock) and after staff
//! verifies an X handle. Only confirmed wallets reach the registry; a hypothesis lives in
//! `wallets.candidate` and the staff routes only. Every change is journaled.

use api_roster::{TableRead, XHandleVerified};
use api_wallets::{Candidate, WalletStatus, WalletsApi, WalletsError};
use arena_platform::{BoxError, Ctx, Person, Plugin, PluginEntry, Route, StartError};
use async_trait::async_trait;
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod error;
mod machine;
mod routes;
mod service;
mod store;

pub use error::Error;
pub use machine::{allowed, kol_status};
pub use service::{CandidateView, KolView, Overview, Wallets};

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "wallets";

/// Migrations from `plugins/wallets/migrations`, run in schema `wallets`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct WalletsPlugin {
    wallets: Arc<Wallets>,
}

impl WalletsPlugin {
    pub fn wallets(&self) -> Arc<Wallets> {
        self.wallets.clone()
    }
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    Ok(build_plugin(ctx)?)
}

/// Same as the entry's constructor, but hands back the concrete plugin (tests use it).
pub fn build_plugin(ctx: &Ctx) -> Result<Arc<WalletsPlugin>, StartError> {
    let wallets = Arc::new(Wallets {
        pool: ctx.pool.clone(),
        roster: ctx.apis.require::<dyn api_roster::RosterApi>(NAME)?,
        kolexplorer: ctx
            .apis
            .require::<dyn api_kolexplorer::KolexplorerApi>(NAME)?,
        solanatracker: ctx
            .apis
            .require::<dyn api_solanatracker::SolanatrackerApi>(NAME)?,
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
        journal: ctx.apis.require::<dyn api_journal::JournalApi>(NAME)?,
    });
    ctx.apis.provide::<dyn WalletsApi>(
        NAME,
        vec![("default", Arc::new(WalletsImpl(wallets.clone())))],
        Arc::new(api_wallets::stub::Stub),
    )?;

    // The roster holds the `arena_data` lock while it emits this: no second lock here.
    let w = wallets.clone();
    ctx.events.on::<TableRead, _, _>(NAME, move |_| {
        let w = w.clone();
        async move { w.match_all(&Person::SYSTEM).await.map_err(BoxError::from) }
    });
    // Staff verified a handle outside any collection: take the lock, try once.
    let w = wallets.clone();
    ctx.events.on::<XHandleVerified, _, _>(NAME, move |e| {
        let w = w.clone();
        async move {
            let me = w.clone();
            w.locked(async move { me.match_kol(&Person::SYSTEM, &e.kol).await })
                .await
                .map_err(BoxError::from)
        }
    });
    Ok(Arc::new(WalletsPlugin { wallets }))
}

impl Plugin for WalletsPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        routes::routes(self.wallets.clone())
    }
}

/// `WalletsApi` over the service.
pub struct WalletsImpl(pub Arc<Wallets>);

#[async_trait]
impl WalletsApi for WalletsImpl {
    async fn propose(
        &self,
        person: &Person,
        kol: &str,
        address: &str,
        evidence: &str,
    ) -> Result<Candidate, WalletsError> {
        let (me, person) = (self.0.clone(), *person);
        let (kol, address, evidence) = (kol.to_string(), address.to_string(), evidence.to_string());
        let inner = me.clone();
        me.locked(async move {
            inner
                .propose_hypothesis(&person, &kol, &address, &evidence)
                .await
        })
        .await
        .map_err(WalletsError::from)
    }

    async fn status(&self, person: &Person, kol: &str) -> Result<WalletStatus, WalletsError> {
        self.0.status(person, kol).await.map_err(WalletsError::from)
    }
}
