//! Backend of the `roster` plugin (layer 3): the network sheet, membership, the X handle.
//!
//! The sheet is the main list (read daily at 05:45 UTC and on the admin button). Group,
//! rates and notes live only in the roster tables; they leave through the staff-only
//! [`api_roster::RosterApi::rates`] and the team and admin routes. There is no public route.

use api_roster::RosterApi;
use api_schedule::SnapshotOpened;
use arena_platform::{BoxError, Ctx, Job, Plugin, PluginEntry, Route, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod http;
mod roster;
mod routes;
mod sheet;
mod store;

pub use http::{HttpSheetFetch, Limits as SheetLimits, USER_AGENT as SHEET_USER_AGENT};
pub use roster::{
    InNetworkFiller, MemberPatch, OpError, ReachInput, Roster, RosterImpl, SheetFetch, SyncReport,
};
pub use sheet::{Parsed, SheetError, SheetRow, Skipped, formats_of, parse as parse_sheet};

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "roster";

/// Migrations from `plugins/roster/migrations`, run in schema `roster`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct RosterPlugin {
    roster: Arc<Roster>,
}

impl RosterPlugin {
    pub fn roster(&self) -> Arc<Roster> {
        self.roster.clone()
    }
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let fetcher = HttpSheetFetch::new().map_err(|message| StartError::Build {
        plugin: NAME.into(),
        message,
    })?;
    Ok(build_with(ctx, Arc::new(fetcher))?)
}

/// Builds the plugin with the given sheet reader (tests give a fake one).
pub fn build_with(
    ctx: &Ctx,
    fetcher: Arc<dyn SheetFetch>,
) -> Result<Arc<RosterPlugin>, StartError> {
    let roster = Arc::new(Roster {
        pool: ctx.pool.clone(),
        config: ctx.config.clone(),
        events: ctx.events.clone(),
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
        snapshots: ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        journal: ctx.apis.require::<dyn api_journal::JournalApi>(NAME)?,
        fetcher,
    });
    ctx.apis.provide::<dyn RosterApi>(
        NAME,
        vec![("default", Arc::new(RosterImpl(roster.clone())))],
        Arc::new(api_roster::stub::Stub),
    )?;
    ctx.slots
        .fill::<dyn api_board::InNetwork>(NAME, Arc::new(InNetworkFiller(roster.clone())))?;
    let on_open = roster.clone();
    ctx.events.on::<SnapshotOpened, _, _>(NAME, move |e| {
        let roster = on_open.clone();
        async move { roster.add_to_snapshot(e.date).await.map_err(BoxError::from) }
    });
    Ok(Arc::new(RosterPlugin { roster }))
}

impl Plugin for RosterPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        routes::routes(self.roster.clone())
    }

    fn jobs(&self, _ctx: &Ctx) -> Vec<Job> {
        self.roster.jobs()
    }
}
