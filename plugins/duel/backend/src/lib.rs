//! Backend of the `duel` plugin (layer 4).
//!
//! Skeleton: mounts no routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "duel";

/// Migrations from `plugins/duel/migrations`, run in schema `duel`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[allow(dead_code)] // read once the lane builds the plugin
struct Uses {
    score: Api<dyn api_score::ScoreApi>,
    fight: Api<dyn api_fight::FightApi>,
}

pub struct DuelPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        score: ctx.apis.require::<dyn api_score::ScoreApi>(NAME)?,
        fight: ctx.apis.require::<dyn api_fight::FightApi>(NAME)?,
    };
    Ok(Arc::new(DuelPlugin { uses }))
}

impl Plugin for DuelPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
