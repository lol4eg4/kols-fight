//! Backend of the `network` plugin (layer 3).
//!
//! `GET /api/network?date=` (public): the network KOLs on the board (Score, tier, PnL 30
//! days, volume, days in market) and the waiting list with a safe reason only. No rate,
//! group, note, contact, referral link or address ever leaves this plugin. No tables.

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

pub use service::{Network, NetworkError, NetworkView, OnBoard, WaitReason, Waiting};

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "network";

/// Migrations from `plugins/network/migrations`, run in schema `network`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct NetworkPlugin {
    network: Network,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let network = Network::new(
        ctx.apis.require::<dyn api_roster::RosterApi>(NAME)?,
        ctx.apis.require::<dyn api_wallets::WalletsApi>(NAME)?,
        ctx.apis.require::<dyn api_score::ScoreApi>(NAME)?,
        ctx.apis.require::<dyn api_snapshots::SnapshotsApi>(NAME)?,
        ctx.apis.require::<dyn api_registry::RegistryApi>(NAME)?,
    );
    Ok(Arc::new(NetworkPlugin { network }))
}

impl Plugin for NetworkPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        routes(&self.network)
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

/// `GET /api/network?date=`: public.
pub fn routes(network: &Network) -> Vec<Route> {
    let network = network.clone();
    vec![Route::new(
        Tag::Public,
        "/api/network",
        get(move |_who: Person, Query(q): Query<DateQuery>| {
            let network = network.clone();
            async move { read(&network, q).await }
        }),
    )]
}

async fn read(network: &Network, q: DateQuery) -> Response {
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
        None => match network.latest_date().await {
            Ok(Some(d)) => d,
            Ok(None) => return error(StatusCode::NOT_FOUND, "no_snapshot", "no snapshot yet"),
            Err(e) => return unavailable(e),
        },
    };
    match network.view(date).await {
        Ok(v) => Json(v).into_response(),
        Err(NetworkError::NoSnapshot(_)) => error(
            StatusCode::NOT_FOUND,
            "no_snapshot",
            "no snapshot for that date",
        ),
        Err(e) => unavailable(e),
    }
}

fn unavailable(e: NetworkError) -> Response {
    tracing::error!(error = ?e, "network section failed");
    error(
        StatusCode::BAD_GATEWAY,
        "unavailable",
        "the network section could not be read",
    )
}
