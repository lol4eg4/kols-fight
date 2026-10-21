//! Backend of the `kolexplorer` plugin (layer 2).
//!
//! Reads the public KOL Explorer leaderboard and the profile pages of its top 100, writes
//! the top 100 into the snapshot and their KE wallets into the registry. Mounts no routes
//! and has no schedule: the schedule plugin calls [`api_kolexplorer::KolexplorerApi::run`].

use arena_platform::{Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

pub mod http;
pub mod parse;
mod service;
mod store;

pub use http::Options;
pub use service::Kolexplorer;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "kolexplorer";

/// Migrations from `plugins/kolexplorer/migrations`, run in schema `kolexplorer`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct KolexplorerPlugin;

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let build_err = |message: String| StartError::Build {
        plugin: NAME.into(),
        message,
    };
    let service = Kolexplorer::new(
        ctx.pool.clone(),
        ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
        ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        Options::default(),
    )
    .map_err(build_err)?;
    ctx.apis.provide::<dyn api_kolexplorer::KolexplorerApi>(
        NAME,
        vec![("default", Arc::new(service))],
        Arc::new(api_kolexplorer::stub::Stub),
    )?;
    Ok(Arc::new(KolexplorerPlugin))
}

impl Plugin for KolexplorerPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
