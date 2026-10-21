//! Backend of the `radar` plugin (layer 2).
//!
//! `GET /api/radar?date=` lists the KOLs of the day's Solana Tracker list that are not on the
//! board. Public DTOs only. No tables: the list lives in solanatracker, the board in snapshots.

use arena_platform::{Ctx, Person, Plugin, PluginEntry, Route, StartError, Tag};
use axum::{
    Json,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;
use sqlx::migrate::Migrator;
use std::sync::Arc;

mod service;

pub use service::{Radar, RadarError, RadarKol, RadarStatus, RadarView};

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "radar";

/// Migrations from `plugins/radar/migrations`, run in schema `radar`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct RadarPlugin {
    radar: Radar,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let radar = Radar::new(
        ctx.apis
            .require::<dyn api_solanatracker::SolanatrackerApi>(NAME)?,
        ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
    );
    Ok(Arc::new(RadarPlugin { radar }))
}

impl Plugin for RadarPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        routes(&self.radar)
    }
}

#[derive(Debug, Deserialize)]
struct DateQuery {
    date: Option<String>,
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

/// `GET /api/radar?date=`: public.
pub fn routes(radar: &Radar) -> Vec<Route> {
    let radar = radar.clone();
    vec![Route::new(
        Tag::Public,
        "/api/radar",
        get(move |_who: Person, Query(q): Query<DateQuery>| {
            let radar = radar.clone();
            async move { read(&radar, q).await }
        }),
    )]
}

async fn read(radar: &Radar, q: DateQuery) -> Response {
    let date = match q.date.as_deref().filter(|d| !d.is_empty()) {
        Some(raw) => match NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "bad_date",
                    "date must be YYYY-MM-DD",
                );
            }
        },
        None => match radar.latest_date().await {
            Ok(Some(d)) => d,
            Ok(None) => return error(StatusCode::NOT_FOUND, "no_snapshot", "no snapshot yet"),
            Err(e) => return unavailable(e),
        },
    };
    match radar.view(date).await {
        Ok(v) => Json(v).into_response(),
        Err(RadarError::NoSnapshot(_)) => error(
            StatusCode::NOT_FOUND,
            "no_snapshot",
            "no snapshot for that date",
        ),
        Err(e) => unavailable(e),
    }
}

fn unavailable(e: RadarError) -> Response {
    tracing::error!(error = ?e, "radar failed");
    error(
        StatusCode::BAD_GATEWAY,
        "unavailable",
        "the radar could not be read",
    )
}
