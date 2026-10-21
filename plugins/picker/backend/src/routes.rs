//! `GET /api/team/picker` and `GET /api/team/picker/export`: staff only.

use crate::service::{Error, Filters, Picker};
use arena_platform::{Person, Route, Tag};
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

fn fail(status: StatusCode, text: impl ToString) -> Response {
    (status, Json(json!({ "error": text.to_string() }))).into_response()
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::Invalid(_) => StatusCode::BAD_REQUEST,
            Error::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        };
        fail(status, self)
    }
}

async fn list(
    State(p): State<Arc<Picker>>,
    person: Person,
    q: Result<Query<Filters>, QueryRejection>,
) -> Response {
    if !Tag::Staff.admits(&person) {
        return Error::Forbidden.into_response();
    }
    let Query(filters) = match q {
        Ok(q) => q,
        Err(e) => return fail(StatusCode::BAD_REQUEST, e.body_text()),
    };
    match p.list(&person, &filters, Utc::now().date_naive()).await {
        Ok(list) => Json(list).into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Deserialize)]
struct ExportQuery {
    /// Comma-separated KOL ids.
    kols: Option<String>,
    /// `1` or `true`: with rates.
    rates: Option<String>,
    date: Option<String>,
}

async fn export(
    State(p): State<Arc<Picker>>,
    person: Person,
    q: Result<Query<ExportQuery>, QueryRejection>,
) -> Response {
    if !Tag::Staff.admits(&person) {
        return Error::Forbidden.into_response();
    }
    let Query(q) = match q {
        Ok(q) => q,
        Err(e) => return fail(StatusCode::BAD_REQUEST, e.body_text()),
    };
    let kols: Vec<String> = q
        .kols
        .unwrap_or_default()
        .split(',')
        .map(str::to_string)
        .collect();
    let with_rates = matches!(q.rates.as_deref(), Some("1" | "true"));
    match p
        .export(&person, &kols, with_rates, &q.date, Utc::now().date_naive())
        .await
    {
        Ok(file) => {
            let disposition =
                HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file.filename))
                    .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
            (
                [
                    (
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("text/csv; charset=utf-8"),
                    ),
                    (header::CONTENT_DISPOSITION, disposition),
                    (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
                ],
                file.csv,
            )
                .into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub(crate) fn routes(picker: Arc<Picker>) -> Vec<Route> {
    vec![
        Route::new(
            Tag::Staff,
            "/api/team/picker",
            get(list).with_state(picker.clone()),
        ),
        Route::new(
            Tag::Staff,
            "/api/team/picker/export",
            get(export).with_state(picker),
        ),
    ]
}
