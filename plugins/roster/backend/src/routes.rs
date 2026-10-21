//! Team and admin routes. The roster has no public route: rates, groups and notes never
//! leave through one.

use crate::{
    roster::{MemberPatch, OpError, Roster},
    store::{self, AdminMember},
};
use arena_platform::{Person, Route, Tag};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde_json::json;
use std::sync::Arc;

type Shared = State<Arc<Roster>>;

fn fail(status: StatusCode, text: impl ToString) -> Response {
    (status, Json(json!({ "error": text.to_string() }))).into_response()
}

impl IntoResponse for OpError {
    fn into_response(self) -> Response {
        let status = match &self {
            OpError::Busy => StatusCode::CONFLICT,
            OpError::NotMember(_) => StatusCode::NOT_FOUND,
            OpError::Invalid(_) => StatusCode::BAD_REQUEST,
            OpError::Failed(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        fail(status, self)
    }
}

/// Handlers check the person themselves as well: the tag guard is not the only wall in
/// front of rates.
fn denied(tag: Tag, person: &Person) -> Option<Response> {
    (!tag.admits(person)).then(|| fail(StatusCode::FORBIDDEN, "forbidden"))
}

async fn list(State(r): Shared, person: Person) -> Response {
    if let Some(e) = denied(Tag::Staff, &person) {
        return e;
    }
    match store::members(&r.pool, true).await {
        Ok(rows) => {
            let members: Vec<AdminMember> = rows.iter().map(AdminMember::from).collect();
            let last = store::last_sync(&r.pool).await.ok().flatten();
            Json(json!({ "members": members, "last_sync": last })).into_response()
        }
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn patch_member(
    State(r): Shared,
    person: Person,
    Path(kol): Path<String>,
    Json(body): Json<MemberPatch>,
) -> Response {
    if let Some(e) = denied(Tag::Staff, &person) {
        return e;
    }
    match r.patch(&person, &kol, &body).await {
        Ok((row, warning)) => {
            Json(json!({ "member": AdminMember::from(&row), "warning": warning })).into_response()
        }
        Err(e) => e.into_response(),
    }
}

async fn last_run(State(r): Shared, person: Person) -> Response {
    if let Some(e) = denied(Tag::Admin, &person) {
        return e;
    }
    match store::last_sync(&r.pool).await {
        Ok(run) => Json(json!({ "last_sync": run })).into_response(),
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn sync(State(r): Shared, person: Person) -> Response {
    if let Some(e) = denied(Tag::Admin, &person) {
        return e;
    }
    match r.sync_now(&person).await {
        Ok(report) => Json(report).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn export(State(r): Shared, person: Person) -> Response {
    if let Some(e) = denied(Tag::Admin, &person) {
        return e;
    }
    match r.export_csv().await {
        Ok(csv) => (
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/csv; charset=utf-8"),
                ),
                (
                    header::CONTENT_DISPOSITION,
                    HeaderValue::from_static("attachment; filename=\"network.csv\""),
                ),
            ],
            csv,
        )
            .into_response(),
        Err(e) => fail(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

pub(crate) fn routes(roster: Arc<Roster>) -> Vec<Route> {
    vec![
        Route::new(
            Tag::Staff,
            "/api/team/network",
            get(list).with_state(roster.clone()),
        ),
        Route::new(
            Tag::Staff,
            "/api/team/network/{kol}",
            patch(patch_member).with_state(roster.clone()),
        ),
        Route::new(
            Tag::Admin,
            "/api/admin/network/sync",
            post(sync).get(last_run).with_state(roster.clone()),
        ),
        Route::new(
            Tag::Admin,
            "/api/admin/network/export",
            get(export).with_state(roster),
        ),
    ]
}
