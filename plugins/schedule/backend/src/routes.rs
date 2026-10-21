//! Staff and admin routes of the collection screen.

use crate::{
    service::{Schedule, StartError},
    store::{self, RunRow, SourceStep, StepRow},
};
use arena_platform::{Person, Route, Tag};
use axum::{
    Json,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// How many runs the screen lists.
const RUNS_SHOWN: i64 = 30;

/// The sources the screen reports, in the order of the run.
const SOURCES: [&str; 2] = ["kolexplorer", "solanatracker"];

#[derive(Debug, Serialize)]
pub struct StepView {
    pub step: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub rows: Option<i32>,
    pub error: Option<String>,
    pub note: Option<String>,
}

impl From<StepRow> for StepView {
    fn from(s: StepRow) -> Self {
        StepView {
            step: s.step,
            status: s.status,
            started_at: s.started_at,
            finished_at: s.finished_at,
            rows: s.rows,
            error: s.error,
            note: s.note,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RunView {
    pub id: i64,
    pub date: NaiveDate,
    pub trigger: String,
    /// Staff member who started a manual run.
    pub person_id: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub error: Option<String>,
    pub steps: Vec<StepView>,
}

impl RunView {
    fn new(r: RunRow, steps: Vec<StepRow>) -> Self {
        RunView {
            id: r.id,
            date: r.date,
            trigger: r.trigger,
            person_id: r.person_id,
            started_at: r.started_at,
            finished_at: r.finished_at,
            status: r.status,
            error: r.error,
            steps: steps.into_iter().map(StepView::from).collect(),
        }
    }
}

/// One read of a source in one run.
#[derive(Debug, Serialize)]
pub struct SourceRunView {
    pub run_id: i64,
    pub date: NaiveDate,
    pub status: String,
    pub at: DateTime<Utc>,
    pub rows: Option<i32>,
    pub error: Option<String>,
    pub note: Option<String>,
}

impl From<SourceStep> for SourceRunView {
    fn from(s: SourceStep) -> Self {
        SourceRunView {
            run_id: s.run_id,
            date: s.date,
            status: s.status,
            at: s.finished_at,
            rows: s.rows,
            error: s.error,
            note: s.note,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SourceView {
    /// `kolexplorer` or `solanatracker`.
    pub source: &'static str,
    /// Latest read that fully succeeded.
    pub last_ok: Option<SourceRunView>,
    /// Latest read that failed or was partial.
    pub last_error: Option<SourceRunView>,
}

#[derive(Debug, Serialize)]
pub struct CollectionView {
    pub today: NaiveDate,
    /// A run is going on now.
    pub running: bool,
    pub sources: Vec<SourceView>,
    /// Newest first.
    pub runs: Vec<RunView>,
}

#[derive(Debug, Deserialize)]
struct RunQuery {
    date: Option<String>,
}

fn error(status: StatusCode, code: &str, message: impl ToString) -> Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message.to_string() } })),
    )
        .into_response()
}

fn internal(e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "collection route failed");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal",
        "could not read the run history",
    )
}

pub async fn collection(s: &Schedule) -> Result<CollectionView, sqlx::Error> {
    let pool = s.pool();
    let mut sources = Vec::new();
    for source in SOURCES {
        sources.push(SourceView {
            source,
            last_ok: store::latest_step(pool, source, &["ok"])
                .await?
                .map(Into::into),
            last_error: store::latest_step(pool, source, &["failed", "partial"])
                .await?
                .map(Into::into),
        });
    }
    let mut runs = Vec::new();
    for r in store::list_runs(pool, RUNS_SHOWN).await? {
        let steps = store::steps_of(pool, r.id).await?;
        runs.push(RunView::new(r, steps));
    }
    Ok(CollectionView {
        today: s.today(),
        running: store::any_running(pool).await?,
        sources,
        runs,
    })
}

async fn run_view(s: &Schedule, id: i64) -> Result<Option<RunView>, sqlx::Error> {
    let Some(run) = store::get_run(s.pool(), id).await? else {
        return Ok(None);
    };
    let steps = store::steps_of(s.pool(), id).await?;
    Ok(Some(RunView::new(run, steps)))
}

pub fn routes(s: &Schedule) -> Vec<Route> {
    let read = s.clone();
    let run = s.clone();
    vec![
        Route::new(
            Tag::Staff,
            "/api/admin/collection",
            get(move || {
                let s = read.clone();
                async move {
                    match collection(&s).await {
                        Ok(v) => Json(v).into_response(),
                        Err(e) => internal(e),
                    }
                }
            }),
        ),
        Route::new(
            Tag::Admin,
            "/api/admin/collection/run",
            post(move |person: Person, Query(q): Query<RunQuery>| {
                let s = run.clone();
                async move { run_now(&s, person, q).await }
            }),
        ),
    ]
}

async fn run_now(s: &Schedule, person: Person, q: RunQuery) -> Response {
    let date = match q.date.as_deref() {
        None | Some("") => s.today(),
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
    };
    match s.start_manual(person, date).await {
        Ok(started) => match run_view(s, started.run_id).await {
            Ok(Some(v)) => (StatusCode::ACCEPTED, Json(v)).into_response(),
            Ok(None) => internal("run row missing"),
            Err(e) => internal(e),
        },
        Err(e @ StartError::Busy(_)) => error(StatusCode::CONFLICT, "busy", e),
        Err(e @ (StartError::UnknownDate(_) | StartError::FutureDate(_))) => {
            error(StatusCode::BAD_REQUEST, "bad_date", e)
        }
        Err(StartError::Internal(m)) => internal(m),
    }
}
