//! Backend of the `network` plugin (layer 3).
//!
//! Skeleton: mounts no routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "network";

/// Migrations from `plugins/network/migrations`, run in schema `network`.
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
    wallets: Api<dyn api_wallets::WalletsApi>,
    score: Api<dyn api_score::ScoreApi>,
    snapshots: Api<dyn api_snapshots::SnapshotsApi>,
    registry: Api<dyn api_registry::RegistryApi>,
}

pub struct NetworkPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        roster: ctx.apis.require::<dyn api_roster::RosterApi>(NAME)?,
        wallets: ctx.apis.require::<dyn api_wallets::WalletsApi>(NAME)?,
        score: ctx.apis.require::<dyn api_score::ScoreApi>(NAME)?,
        snapshots: ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
    };
    Ok(Arc::new(NetworkPlugin { uses }))
}

impl Plugin for NetworkPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
