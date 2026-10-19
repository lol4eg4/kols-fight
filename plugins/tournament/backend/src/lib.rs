//! Backend of the `tournament` plugin (layer 4).
//!
//! Skeleton: mounts no routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "tournament";

/// Migrations from `plugins/tournament/migrations`, run in schema `tournament`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[allow(dead_code)] // read once the lane builds the plugin
struct Uses {
    fight: Api<dyn api_fight::FightApi>,
    score: Api<dyn api_score::ScoreApi>,
    roster: Api<dyn api_roster::RosterApi>,
}

pub struct TournamentPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        fight: ctx.apis.require::<dyn api_fight::FightApi>(NAME)?,
        score: ctx.apis.require::<dyn api_score::ScoreApi>(NAME)?,
        roster: ctx.apis.require::<dyn api_roster::RosterApi>(NAME)?,
    };
    Ok(Arc::new(TournamentPlugin { uses }))
}

impl Plugin for TournamentPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
