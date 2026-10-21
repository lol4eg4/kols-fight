//! Backend of the `schedule` plugin (layer 2).
//!
//! Runs the daily collection at 06:00 UTC: open the snapshot, emit «snapshot opened», read
//! KOL Explorer, read Solana Tracker, close the snapshot, rank it. Keeps a row per run and
//! per step, serves the staff history, and lets an admin run it again for any date.
//! The plugin has no API; other plugins react to its event.

use arena_platform::{BoxError, Ctx, Job, Plugin, PluginEntry, Route, StartError};
use async_trait::async_trait;
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod routes;
mod service;
mod store;

pub use routes::{
    CollectionView, RunView, SourceRunView, SourceView, StepView, collection, routes,
};
pub use service::{JOB, Options, Schedule, StartError as RunStartError, Started};

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "schedule";

/// Migrations from `plugins/schedule/migrations`, run in schema `schedule`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct SchedulePlugin {
    schedule: Schedule,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let schedule = Schedule::new(
        ctx.pool.clone(),
        ctx.events.clone(),
        ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        ctx.apis
            .require::<dyn api_kolexplorer::KolexplorerApi>(NAME)?,
        ctx.apis
            .require::<dyn api_solanatracker::SolanatrackerApi>(NAME)?,
        ctx.apis.require::<dyn api_score::ScoreApi>(NAME)?,
        ctx.apis.require::<dyn api_journal::JournalApi>(NAME)?,
        Options::default(),
    );
    Ok(Arc::new(SchedulePlugin { schedule }))
}

#[async_trait]
impl Plugin for SchedulePlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        routes::routes(&self.schedule)
    }

    /// Closes and ranks an open snapshot older than today that a stopped run left behind.
    async fn install(&self, _ctx: &Ctx) -> Result<(), BoxError> {
        self.schedule.recover().await;
        Ok(())
    }

    fn jobs(&self, _ctx: &Ctx) -> Vec<Job> {
        vec![self.schedule.job()]
    }
}
