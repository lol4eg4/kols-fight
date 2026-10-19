//! Backend of the `wallets` plugin (layer 3).
//!
//! Skeleton: registers [`api_wallets::WalletsApi`] with «not built» answers and mounts no
//! routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod not_built;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "wallets";

/// Migrations from `plugins/wallets/migrations`, run in schema `wallets`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[allow(dead_code)] // read once the lane builds the plugin
struct Uses {
    roster: Api<dyn api_roster::RosterApi>,
    kolexplorer: Api<dyn api_kolexplorer::KolexplorerApi>,
    solanatracker: Api<dyn api_solanatracker::SolanatrackerApi>,
    registry: Api<dyn api_registry::RegistryApi>,
    journal: Api<dyn api_journal::JournalApi>,
}

pub struct WalletsPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        roster: ctx.apis.require::<dyn api_roster::RosterApi>(NAME)?,
        kolexplorer: ctx.apis.require::<dyn api_kolexplorer::KolexplorerApi>(NAME)?,
        solanatracker: ctx.apis.require::<dyn api_solanatracker::SolanatrackerApi>(NAME)?,
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
        journal: ctx.apis.require::<dyn api_journal::JournalApi>(NAME)?,
    };
    ctx.apis.provide::<dyn api_wallets::WalletsApi>(
        NAME,
        vec![("default", Arc::new(not_built::NotBuilt))],
        Arc::new(api_wallets::stub::Stub),
    )?;
    Ok(Arc::new(WalletsPlugin { uses }))
}

impl Plugin for WalletsPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
