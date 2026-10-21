//! Backend of the `picker` plugin (layer 3): staff filters the network by Score, tier,
//! rate and format, checks KOLs and exports them for a client (public fields only) or
//! with rates (staff). Rates come only through the roster API.

pub mod csv;
mod routes;
pub mod service;

use arena_platform::{Ctx, Plugin, PluginEntry, Route, StartError};
pub use service::{Export, Filters, PickList, Picker, PublicRow, Row};
use sqlx::migrate::Migrator;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "picker";

/// Migrations from `plugins/picker/migrations`, run in schema `picker`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct PickerPlugin {
    picker: Arc<Picker>,
}

impl PickerPlugin {
    pub fn picker(&self) -> Arc<Picker> {
        self.picker.clone()
    }
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    Ok(build_plugin(ctx)?)
}

/// Same as the entry's constructor, but hands back the concrete plugin (tests use it).
pub fn build_plugin(ctx: &Ctx) -> Result<Arc<PickerPlugin>, StartError> {
    let picker = Arc::new(Picker {
        roster: ctx.apis.require::<dyn api_roster::RosterApi>(NAME)?,
        score: ctx.apis.require::<dyn api_score::ScoreApi>(NAME)?,
        registry: ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
    });
    Ok(Arc::new(PickerPlugin { picker }))
}

impl Plugin for PickerPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        routes::routes(self.picker.clone())
    }
}
