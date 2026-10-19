//! Backend of the `score` plugin (layer 1).
//!
//! Skeleton: registers [`api_score::ScoreApi`] with «not built» answers and mounts no
//! routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod not_built;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "score";

/// Migrations from `plugins/score/migrations`, run in schema `score`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[allow(dead_code)] // read once the lane builds the plugin
struct Uses {
    snapshots: Api<dyn api_snapshots::SnapshotsApi>,
    registry: Api<dyn api_registry::RegistryApi>,
}

pub struct ScorePlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        snapshots: ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
    };
    ctx.apis.provide::<dyn api_score::ScoreApi>(
        NAME,
        vec![("default", Arc::new(not_built::NotBuilt))],
        Arc::new(api_score::stub::Stub),
    )?;
    Ok(Arc::new(ScorePlugin { uses }))
}

impl Plugin for ScorePlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
