//! Backend of the `fomo` plugin (layer 3).
//!
//! Skeleton: mounts no routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
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

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[allow(dead_code)] // read once the lane builds the plugin
struct Uses {
    wallets: Api<dyn api_wallets::WalletsApi>,
    journal: Api<dyn api_journal::JournalApi>,
}

pub struct FomoPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        wallets: ctx.apis.require::<dyn api_wallets::WalletsApi>(NAME)?,
        journal: ctx.apis.require::<dyn api_journal::JournalApi>(NAME)?,
    };
    Ok(Arc::new(FomoPlugin { uses }))
}

impl Plugin for FomoPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
