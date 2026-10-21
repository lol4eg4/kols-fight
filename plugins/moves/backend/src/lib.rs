//! Backend of the `moves` plugin (layer 4): rank change and «Взлёт недели».
//!
//! `GET /api/moves?date=YYYY-MM-DD` (public) compares each KOL's rank on `date` with the rank
//! on the day before and on the day 7 days back. A change is given only when the KOL's data
//! is fresh on both days; a failed source carries old rows forward and must not move a place.
//! Days are calendar days: a missing snapshot on the reference day means no change, never a
//! comparison with some other day.
//!
//! Fresh: the KOL's metric row on a day was fetched on or after 00:00 UTC of that day. A row
//! carried forward from an earlier day keeps its old `fetched_at`, so it is not fresh.

use api_registry::RegistryApi;
use api_score::{ScoreApi, ScoreError};
use api_snapshots::{SnapshotsApi, SnapshotsError};
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
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "moves";

/// Migrations from `plugins/moves/migrations`, run in schema `moves`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// Days back for the «day» change.
pub const DAY_BACK: i64 = 1;
/// Days back for the «week» change.
pub const WEEK_BACK: i64 = 7;

/// KOL id (registry slug).
pub type KolId = String;

/// Whether a metric row fetched at `fetched_at` is fresh on `date`.
pub fn is_fresh(date: NaiveDate, fetched_at: DateTime<Utc>) -> bool {
    fetched_at >= date.and_time(NaiveTime::MIN).and_utc()
}

/// Rank change of one KOL. Positive = climbed, negative = fell, 0 = same place.
/// `None` = not known (a day is missing, or the KOL's data is not fresh on both days).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Change {
    pub kol: KolId,
    /// Against the day before.
    pub day: Option<i32>,
    /// Against 7 days back.
    pub week: Option<i32>,
}

/// A climb to show in the news strip.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Climb {
    pub kol: KolId,
    /// Registry name; `None` when the registry does not know the KOL.
    pub name: Option<String>,
    /// Places climbed, above zero.
    pub places: i32,
}

/// Body of `GET /api/moves`. Public: ids, places and names only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MovesView {
    pub date: NaiveDate,
    /// KOLs with at least one known change, by current rank.
    pub changes: Vec<Change>,
    /// Best climb against the day before.
    pub day_climb: Option<Climb>,
    /// «Взлёт недели»: best climb against 7 days back.
    pub week_climb: Option<Climb>,
}

/// One day's ranks and the KOLs whose data is fresh that day.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Day {
    pub ranks: HashMap<KolId, u32>,
    pub fresh: HashSet<KolId>,
}

/// Changes on the current day against the optional reference days. Only KOLs with at
/// least one known change are listed, by current rank.
pub fn changes(now: &Day, day: Option<&Day>, week: Option<&Day>) -> Vec<Change> {
    let diff = |kol: &str, then: Option<&Day>| -> Option<i32> {
        let then = then?;
        if !now.fresh.contains(kol) || !then.fresh.contains(kol) {
            return None;
        }
        let (a, b) = (*now.ranks.get(kol)?, *then.ranks.get(kol)?);
        Some(i64::from(b).saturating_sub(i64::from(a)) as i32)
    };
    let mut kols: Vec<(&KolId, u32)> = now.ranks.iter().map(|(k, r)| (k, *r)).collect();
    kols.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)));
    kols.into_iter()
        .filter_map(|(kol, _)| {
            let c = Change {
                kol: kol.clone(),
                day: diff(kol, day),
                week: diff(kol, week),
            };
            (c.day.is_some() || c.week.is_some()).then_some(c)
        })
        .collect()
}

/// The best climb by `pick`, above zero. `changes` is in rank order, so a tie goes to the
/// KOL with the better current rank.
fn best(changes: &[Change], pick: impl Fn(&Change) -> Option<i32>) -> Option<(KolId, i32)> {
    let mut best: Option<(KolId, i32)> = None;
    for c in changes {
        if let Some(n) = pick(c)
            && n > 0
            && best.as_ref().is_none_or(|(_, b)| n > *b)
        {
            best = Some((c.kol.clone(), n));
        }
    }
    best
}

/// Why `GET /api/moves` has no answer.
#[derive(Debug, PartialEq, Eq)]
pub enum MovesError {
    /// The date has no closed, ranked snapshot.
    NoData(NaiveDate),
    /// Score, snapshots or the registry is off or not built.
    Unavailable,
    Internal(String),
}

impl From<ScoreError> for MovesError {
    fn from(e: ScoreError) -> Self {
        match e {
            ScoreError::Off | ScoreError::NotBuilt => MovesError::Unavailable,
            e => MovesError::Internal(e.to_string()),
        }
    }
}

impl From<SnapshotsError> for MovesError {
    fn from(e: SnapshotsError) -> Self {
        match e {
            SnapshotsError::Off | SnapshotsError::NotBuilt => MovesError::Unavailable,
            e => MovesError::Internal(e.to_string()),
        }
    }
}

/// Ranks and fresh KOLs of `date`; `None` when the day has no closed, ranked snapshot.
async fn load_day(
    score: &dyn ScoreApi,
    snapshots: &dyn SnapshotsApi,
    person: &Person,
    date: NaiveDate,
) -> Result<Option<Day>, MovesError> {
    let ranking = match score.ranking(person, date).await {
        Ok(r) => r,
        Err(
            ScoreError::NoSnapshot { .. }
            | ScoreError::NotClosed { .. }
            | ScoreError::NotRanked { .. },
        ) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let view = match snapshots.snapshot(person, date).await {
        Ok(v) => v,
        Err(SnapshotsError::NoSnapshot { .. }) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let ranks = ranking
        .rows
        .iter()
        .map(|r| (r.kol.clone(), r.rank))
        .collect();
    let fresh = view
        .members
        .iter()
        .filter(|m| m.row.as_ref().is_some_and(|r| is_fresh(date, r.fetched_at)))
        .map(|m| m.kol.clone())
        .collect();
    Ok(Some(Day { ranks, fresh }))
}

/// The moves of `date`.
pub async fn moves(
    score: &dyn ScoreApi,
    snapshots: &dyn SnapshotsApi,
    registry: &dyn RegistryApi,
    person: &Person,
    date: NaiveDate,
) -> Result<MovesView, MovesError> {
    let now = load_day(score, snapshots, person, date)
        .await?
        .ok_or(MovesError::NoData(date))?;
    let day = load_day(score, snapshots, person, date - Duration::days(DAY_BACK)).await?;
    let week = load_day(score, snapshots, person, date - Duration::days(WEEK_BACK)).await?;
    let changes = changes(&now, day.as_ref(), week.as_ref());

    let day_best = best(&changes, |c| c.day);
    let week_best = best(&changes, |c| c.week);
    let mut ids: Vec<KolId> = [&day_best, &week_best]
        .into_iter()
        .flatten()
        .map(|(k, _)| k.clone())
        .collect();
    ids.sort();
    ids.dedup();
    let names: HashMap<KolId, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        registry
            .list_kols(person, &ids)
            .await
            .map_err(|e| match e {
                api_registry::RegistryError::Off | api_registry::RegistryError::NotBuilt => {
                    MovesError::Unavailable
                }
                e => MovesError::Internal(e.to_string()),
            })?
            .into_iter()
            .map(|p| (p.id, p.name))
            .collect()
    };
    let climb = |b: Option<(KolId, i32)>| {
        b.map(|(kol, places)| Climb {
            name: names.get(&kol).cloned(),
            kol,
            places,
        })
    };
    Ok(MovesView {
        date,
        changes,
        day_climb: climb(day_best),
        week_climb: climb(week_best),
    })
}

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
#[derive(Clone)]
struct Uses {
    score: Api<dyn ScoreApi>,
    snapshots: Api<dyn SnapshotsApi>,
    registry: Api<dyn RegistryApi>,
}

pub struct MovesPlugin {
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        score: ctx.apis.require::<dyn ScoreApi>(NAME)?,
        snapshots: ctx.apis.require::<dyn SnapshotsApi>(NAME)?,
        registry: ctx.apis.require::<dyn RegistryApi>(NAME)?,
    };
    Ok(Arc::new(MovesPlugin { uses }))
}

impl Plugin for MovesPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        vec![Route::new(
            Tag::Public,
            "/api/moves",
            get(get_moves).with_state(self.uses.clone()),
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

async fn get_moves(
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
    let score = uses.score.pick(&person);
    let snapshots = uses.snapshots.pick(&person);
    let registry = uses.registry.pick(&person);
    let date = match parsed {
        Some(d) => d,
        None => match snapshots.latest_date(&person).await {
            Ok(Some(d)) => d,
            Ok(None) => {
                return problem(StatusCode::NOT_FOUND, "no_snapshot", "no snapshot yet");
            }
            Err(e) => return failure(MovesError::from(e)),
        },
    };
    match moves(&*score, &*snapshots, &*registry, &person, date).await {
        Ok(view) => Json(view).into_response(),
        Err(e) => failure(e),
    }
}

fn failure(e: MovesError) -> Response {
    match e {
        MovesError::NoData(d) => problem(
            StatusCode::NOT_FOUND,
            "no_snapshot",
            &format!("no ranked snapshot for {d}"),
        ),
        MovesError::Unavailable => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "moves are not available now",
        ),
        MovesError::Internal(message) => {
            tracing::error!(%message, "moves failed");
            problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "moves failed",
            )
        }
    }
}
