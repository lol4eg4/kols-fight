//! Backend of the `board` plugin (layer 1).
//!
//! Skeleton: mounts no routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "board";

/// Migrations from `plugins/board/migrations`, run in schema `board`.
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
    score: Api<dyn api_score::ScoreApi>,
}

pub struct BoardPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
        snapshots: ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        score: ctx.apis.require::<dyn api_score::ScoreApi>(NAME)?,
    };
    ctx.slots.declare::<dyn api_board::InNetwork>(
        NAME,
        api_board::IN_NETWORK_SLOT,
        Arc::new(api_board::NotInNetwork),
    )?;
    Ok(Arc::new(BoardPlugin { uses }))
}

impl Plugin for BoardPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
