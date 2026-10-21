//! Backend of the `registry` plugin (layer 1).
//!
//! Skeleton: registers [`api_registry::RegistryApi`] with «not built» answers and mounts no
//! routes yet. The plugin's lane builds it.

use arena_platform::{Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod not_built;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "registry";

/// Migrations from `plugins/registry/migrations`, run in schema `registry`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct RegistryPlugin;

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    ctx.apis.provide::<dyn api_registry::RegistryApi>(
        NAME,
        vec![("default", Arc::new(not_built::NotBuilt))],
        Arc::new(api_registry::stub::Stub),
    )?;
    Ok(Arc::new(RegistryPlugin))
}

impl Plugin for RegistryPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
