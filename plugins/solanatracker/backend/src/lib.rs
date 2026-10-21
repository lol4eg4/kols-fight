//! Backend of the `solanatracker` plugin (layer 2).
//!
//! Reads the day's KOL list and PnL of member KOLs from the Solana Tracker Data API with the
//! company key (`SOLANA_TRACKER_API_KEY`). Mounts no routes and has no schedule: the schedule
//! plugin calls [`api_solanatracker::SolanatrackerApi::run`].

use arena_platform::{Ctx, Plugin, PluginEntry, Setting, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

pub mod client;
pub mod metrics;
pub mod models;
mod service;
mod store;

pub use client::Options;
pub use service::Solanatracker;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "solanatracker";

/// Migrations from `plugins/solanatracker/migrations`, run in schema `solanatracker`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct SolanatrackerPlugin;

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    // A missing key is not a start-up error: every call then reports «not configured».
    let key = ctx
        .config
        .get(Setting::SolanaTrackerApiKey)
        .ok()
        .map(str::to_string);
    let service = Solanatracker::new(
        ctx.pool.clone(),
        ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
        ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        key,
        Options::default(),
    )
    .map_err(|message| StartError::Build {
        plugin: NAME.into(),
        message,
    })?;
    ctx.apis
        .provide::<dyn api_solanatracker::SolanatrackerApi>(
            NAME,
            vec![("default", Arc::new(service))],
            Arc::new(api_solanatracker::stub::Stub),
        )?;
    Ok(Arc::new(SolanatrackerPlugin))
}

impl Plugin for SolanatrackerPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
