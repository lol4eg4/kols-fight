//! Backend of the `auth` plugin (layer 0).
//!
//! Skeleton: registers [`api_auth::AuthApi`] with «not built» answers and mounts no
//! routes yet. The plugin's lane builds it.

use arena_platform::{Api, Ctx, Plugin, PluginEntry, StartError};
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod not_built;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "auth";

/// Migrations from `plugins/auth/migrations`, run in schema `auth`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[allow(dead_code)] // read once the lane builds the plugin
struct Uses {
    journal: Api<dyn api_journal::JournalApi>,
}

pub struct AuthPlugin {
    #[allow(dead_code)] // read once the lane builds the plugin
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        journal: ctx.apis.require::<dyn api_journal::JournalApi>(NAME)?,
    };
    ctx.apis.provide::<dyn api_auth::AuthApi>(
        NAME,
        vec![("default", Arc::new(not_built::NotBuilt))],
        Arc::new(api_auth::stub::Stub),
    )?;
    Ok(Arc::new(AuthPlugin { uses }))
}

impl Plugin for AuthPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }
}
