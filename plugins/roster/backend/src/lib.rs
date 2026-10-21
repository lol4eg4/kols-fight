//! Backend of the `roster` plugin (layer 3).
//!
//! Skeleton: registers [`api_roster::RosterApi`] with «not built» answers and mounts no
//! routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod not_built;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "roster";

/// Migrations from `plugins/roster/migrations`, run in schema `roster`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[allow(dead_code)] // read once the lane builds the plugin
struct Uses {
    registry: Api<dyn api_registry::RegistryApi>,
    snapshots: Api<dyn api_snapshots::SnapshotsApi>,
    journal: Api<dyn api_journal::JournalApi>,
}

pub struct RosterPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
        snapshots: ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        journal: ctx.apis.require::<dyn api_journal::JournalApi>(NAME)?,
    };
    ctx.apis.provide::<dyn api_roster::RosterApi>(
        NAME,
        vec![("default", Arc::new(not_built::NotBuilt))],
        Arc::new(api_roster::stub::Stub),
    )?;
    Ok(Arc::new(RosterPlugin { uses }))
}

impl Plugin for RosterPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
