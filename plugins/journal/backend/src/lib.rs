//! Backend of the `journal` plugin (layer 0).
//!
//! Skeleton: registers [`api_journal::JournalApi`] with «not built» answers and mounts no
//! routes yet. The plugin's lane builds it.

use arena_platform::{Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod not_built;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "journal";

/// Migrations from `plugins/journal/migrations`, run in schema `journal`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct JournalPlugin;

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    ctx.apis.provide::<dyn api_journal::JournalApi>(
        NAME,
        vec![("default", Arc::new(not_built::NotBuilt))],
        Arc::new(api_journal::stub::Stub),
    )?;
    Ok(Arc::new(JournalPlugin))
}

impl Plugin for JournalPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
