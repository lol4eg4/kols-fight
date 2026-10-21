//! Backend of the `streaks` plugin (layer 4): badges for streaks.
//!
//! `GET /api/streaks?date=YYYY-MM-DD` (public; no `date` = the latest closed snapshot) lists,
//! per KOL on that day's board, the streak badges it holds:
//! - `trade_7d`, «7 days with a trade»: on each of the 7 days ending at `date` the KOL has a
//!   fresh metric row with `traded_24h` true.
//! - `profit_3w`, «3 weeks in profit»: on `date`, 7 and 14 days back the KOL has a fresh row
//!   with `d7` above zero (each `d7` covers one week, so the three weeks follow each other).
//!
//! A day without a closed snapshot, a KOL without a row that day, a row carried forward from
//! an earlier day (not fresh) or a missing value ends the streak. No badge without the whole
//! history. Fresh: fetched on or after 00:00 UTC of that day.

use api_snapshots::{SnapshotStatus, SnapshotsApi, SnapshotsError};
use arena_platform::{Api, Ctx, Person, Plugin, PluginEntry, Route, StartError, Tag};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::migrate::Migrator;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "streaks";

/// Migrations from `plugins/streaks/migrations`, run in schema `streaks`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// Days in the trade streak.
pub const TRADE_DAYS: i64 = 7;
/// Weeks in the profit streak.
pub const PROFIT_WEEKS: i64 = 3;

/// KOL id (registry slug).
pub type KolId = String;

/// A streak badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Streak {
    /// «7 days with a trade».
    #[serde(rename = "trade_7d")]
    Trade7d,
    /// «3 weeks in profit».
    #[serde(rename = "profit_3w")]
    Profit3w,
}

/// What a streak needs from one KOL's metric row on one day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DayRow {
    /// Fetched on that day (not carried forward).
    pub fresh: bool,
    pub traded_24h: Option<bool>,
    pub d7: Option<f64>,
}

/// Rows by day, then by KOL. Only closed days are in it.
pub type History = BTreeMap<NaiveDate, HashMap<KolId, DayRow>>;

/// One KOL's badges.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KolStreaks {
    pub kol: KolId,
    pub badges: Vec<Streak>,
}

/// Body of `GET /api/streaks`. Public: ids and badge names only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StreaksView {
    pub date: NaiveDate,
    /// KOLs with at least one badge, by id.
    pub streaks: Vec<KolStreaks>,
}

/// Whether a metric row fetched at `fetched_at` is fresh on `date`.
pub fn is_fresh(date: NaiveDate, fetched_at: DateTime<Utc>) -> bool {
    fetched_at >= date.and_time(NaiveTime::MIN).and_utc()
}

fn row<'a>(h: &'a History, date: NaiveDate, kol: &str) -> Option<&'a DayRow> {
    h.get(&date)?.get(kol).filter(|r| r.fresh)
}

/// The badges `kol` holds on `date`, given the history.
pub fn badges(h: &History, date: NaiveDate, kol: &str) -> Vec<Streak> {
    let mut out = Vec::new();
    let trade = (0..TRADE_DAYS)
        .all(|i| row(h, date - Duration::days(i), kol).is_some_and(|r| r.traded_24h == Some(true)));
    if trade {
        out.push(Streak::Trade7d);
    }
    let profit = (0..PROFIT_WEEKS).all(|w| {
        row(h, date - Duration::days(7 * w), kol).is_some_and(|r| r.d7.is_some_and(|v| v > 0.0))
    });
    if profit {
        out.push(Streak::Profit3w);
    }
    out
}

/// Every KOL on `date`'s board with its badges; those with none are left out.
pub fn all_badges(h: &History, date: NaiveDate) -> Vec<KolStreaks> {
    let Some(today) = h.get(&date) else {
        return Vec::new();
    };
    let kols: BTreeSet<&KolId> = today.keys().collect();
    kols.into_iter()
        .filter_map(|kol| {
            let b = badges(h, date, kol);
            (!b.is_empty()).then(|| KolStreaks {
                kol: kol.clone(),
                badges: b,
            })
        })
        .collect()
}

/// Why `GET /api/streaks` has no answer.
#[derive(Debug, PartialEq, Eq)]
pub enum StreaksError {
    /// The date has no closed snapshot.
    NoData(NaiveDate),
    /// Snapshots is off or not built.
    Unavailable,
    Internal(String),
}

impl From<SnapshotsError> for StreaksError {
    fn from(e: SnapshotsError) -> Self {
        match e {
            SnapshotsError::Off | SnapshotsError::NotBuilt => StreaksError::Unavailable,
            e => StreaksError::Internal(e.to_string()),
        }
    }
}

/// Days of history a badge may need before `date` (the longest streak, in days, minus one).
fn reach_back() -> i64 {
    (TRADE_DAYS - 1).max(7 * (PROFIT_WEEKS - 1))
}

/// The streak badges of `date`.
pub async fn streaks(
    snapshots: &dyn SnapshotsApi,
    person: &Person,
    date: NaiveDate,
) -> Result<StreaksView, StreaksError> {
    let closed = snapshots.dates(person).await?;
    if !closed.contains(&date) {
        return Err(StreaksError::NoData(date));
    }
    let mut history = History::new();
    for back in 0..=reach_back() {
        let day = date - Duration::days(back);
        if !closed.contains(&day) {
            continue;
        }
        let view = match snapshots.snapshot(person, day).await {
            Ok(v) => v,
            Err(SnapshotsError::NoSnapshot { .. }) => continue,
            Err(e) => return Err(e.into()),
        };
        if view.snapshot.status != SnapshotStatus::Closed {
            continue;
        }
        let rows = view
            .members
            .into_iter()
            .filter_map(|m| {
                let r = m.row?;
                Some((
                    m.kol,
                    DayRow {
                        fresh: is_fresh(day, r.fetched_at),
                        traded_24h: r.metrics.traded_24h,
                        d7: r.metrics.d7,
                    },
                ))
            })
            .collect();
        history.insert(day, rows);
    }
    if !history.contains_key(&date) {
        return Err(StreaksError::NoData(date));
    }
    Ok(StreaksView {
        date,
        streaks: all_badges(&history, date),
    })
}

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[derive(Clone)]
struct Uses {
    snapshots: Api<dyn SnapshotsApi>,
}

pub struct StreaksPlugin {
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        snapshots: ctx.apis.require::<dyn SnapshotsApi>(NAME)?,
    };
    Ok(Arc::new(StreaksPlugin { uses }))
}

impl Plugin for StreaksPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        vec![Route::new(
            Tag::Public,
            "/api/streaks",
            get(get_streaks).with_state(self.uses.clone()),
        )]
    }
}

#[derive(Deserialize)]
struct DateQuery {
    date: Option<String>,
}

fn problem(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

async fn get_streaks(
    State(uses): State<Uses>,
    person: Person,
    Query(q): Query<DateQuery>,
) -> Response {
    let parsed = match q.date.as_deref() {
        None => None,
        Some(d) => match NaiveDate::parse_from_str(d, "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(_) => {
                return problem(
                    StatusCode::BAD_REQUEST,
                    "bad_date",
                    "date must be YYYY-MM-DD",
                );
            }
        },
    };
    let snapshots = uses.snapshots.pick(&person);
    let date = match parsed {
        Some(d) => d,
        None => match snapshots.latest_date(&person).await {
            Ok(Some(d)) => d,
            Ok(None) => {
                return problem(StatusCode::NOT_FOUND, "no_snapshot", "no snapshot yet");
            }
            Err(e) => return failure(StreaksError::from(e)),
        },
    };
    match streaks(&*snapshots, &person, date).await {
        Ok(view) => Json(view).into_response(),
        Err(e) => failure(e),
    }
}

fn failure(e: StreaksError) -> Response {
    match e {
        StreaksError::NoData(d) => problem(
            StatusCode::NOT_FOUND,
            "no_snapshot",
            &format!("no closed snapshot for {d}"),
        ),
        StreaksError::Unavailable => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "streaks are not available now",
        ),
        StreaksError::Internal(message) => {
            tracing::error!(%message, "streaks failed");
            problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "streaks failed",
            )
        }
    }
}
