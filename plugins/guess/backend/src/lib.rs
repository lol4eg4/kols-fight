//! Backend of the `guess` plugin (layer 4).
//!
//! Skeleton: mounts no routes yet. The plugin's lane builds it.

use arena_platform::{Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "guess";

/// Migrations from `plugins/guess/migrations`, run in schema `guess`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct GuessPlugin;

fn build(_ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    Ok(Arc::new(GuessPlugin))
}

impl Plugin for GuessPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
