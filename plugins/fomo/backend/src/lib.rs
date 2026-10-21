//! Backend of the `fomo` plugin (layer 3): the FOMO match helper.
//!
//! Staff enters 2–3 positions from a KOL's FOMO profile; the public Solana node lists each
//! token's holders; exactly one wallet holding two positions or more becomes a hypothesis
//! in Wallets. Every run is journaled. FOMO and the explorers are links only.

pub mod base58;
pub mod matching;
mod routes;
pub mod rpc;
pub mod service;

use arena_platform::{Ctx, Plugin, PluginEntry, Route, StartError};
pub use rpc::Options;
pub use service::{Fomo, MatchRequest, MatchResult, Outcome};
use sqlx::migrate::Migrator;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "fomo";

/// Migrations from `plugins/fomo/migrations`, run in schema `fomo`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct FomoPlugin {
    fomo: Arc<Fomo>,
}

impl FomoPlugin {
    pub fn fomo(&self) -> Arc<Fomo> {
        self.fomo.clone()
    }
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    Ok(build_plugin(ctx, Options::default())?)
}

/// The entry's constructor with explicit RPC limits (tests shorten them).
pub fn build_plugin(ctx: &Ctx, opts: Options) -> Result<Arc<FomoPlugin>, StartError> {
    let rpc = rpc::Rpc::new(&opts).map_err(|message| StartError::Build {
        plugin: NAME.into(),
        message,
    })?;
    let fomo = Arc::new(Fomo {
        rpc,
        config: ctx.config.clone(),
        wallets: ctx.apis.require::<dyn api_wallets::WalletsApi>(NAME)?,
        journal: ctx.apis.require::<dyn api_journal::JournalApi>(NAME)?,
    });
    Ok(Arc::new(FomoPlugin { fomo }))
}

impl Plugin for FomoPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        routes::routes(self.fomo.clone())
    }
}
