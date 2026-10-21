//! `POST /api/team/fomo/match`: staff only.

use crate::service::{Error, Fomo, MatchRequest};
use arena_platform::{Person, Route, Tag};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
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
            Error::NotMember(_) => StatusCode::NOT_FOUND,
            Error::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Error::Journal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        fail(status, self)
    }
}

/// Every outcome of a run answers 200 with the result; only a request that was not run
/// answers an error.
async fn run(
    State(f): State<Arc<Fomo>>,
    person: Person,
    body: Result<Json<MatchRequest>, JsonRejection>,
) -> Response {
    if !Tag::Staff.admits(&person) {
        return Error::Forbidden.into_response();
    }
    let Json(req) = match body {
        Ok(b) => b,
        Err(e) => return fail(StatusCode::BAD_REQUEST, e.body_text()),
    };
    match f.run(&person, &req).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => e.into_response(),
    }
}

pub(crate) fn routes(fomo: Arc<Fomo>) -> Vec<Route> {
    vec![Route::new(
        Tag::Staff,
        "/api/team/fomo/match",
        post(run).with_state(fomo),
    )]
}
