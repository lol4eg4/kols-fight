//! Backend of the `fight` plugin (layer 1).
//!
//! Skeleton: registers [`api_fight::FightApi`] with «not built» answers and mounts no
//! routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod not_built;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "fight";

/// Migrations from `plugins/fight/migrations`, run in schema `fight`.
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
    score: Api<dyn api_score::ScoreApi>,
    registry: Api<dyn api_registry::RegistryApi>,
}

pub struct FightPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        snapshots: ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        score: ctx.apis.require::<dyn api_score::ScoreApi>(NAME)?,
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
    };
    ctx.apis.provide::<dyn api_fight::FightApi>(
        NAME,
        vec![("default", Arc::new(not_built::NotBuilt))],
        Arc::new(api_fight::stub::Stub),
    )?;
    Ok(Arc::new(FightPlugin { uses }))
}

impl Plugin for FightPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
