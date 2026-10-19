//! Backend of the `radar` plugin (layer 2).
//!
//! Skeleton: mounts no routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "radar";

/// Migrations from `plugins/radar/migrations`, run in schema `radar`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[allow(dead_code)] // read once the lane builds the plugin
struct Uses {
    solanatracker: Api<dyn api_solanatracker::SolanatrackerApi>,
    snapshots: Api<dyn api_snapshots::SnapshotsApi>,
}

pub struct RadarPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        solanatracker: ctx.apis.require::<dyn api_solanatracker::SolanatrackerApi>(NAME)?,
        snapshots: ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
    };
    Ok(Arc::new(RadarPlugin { uses }))
}

impl Plugin for RadarPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
