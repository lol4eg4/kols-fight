//! Staff routes of the Wallets screen. There is no public route: a hypothesis address never
//! leaves the staff area.

use crate::{error::Error, service::Shared};
use arena_platform::{Person, Route, Tag};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Error::Busy => StatusCode::CONFLICT,
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::NotMember(_) | Error::NoCandidate(_) => StatusCode::NOT_FOUND,
            Error::InvalidAddress(_) | Error::Refused(_) => StatusCode::BAD_REQUEST,
            Error::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Deserialize)]
struct Confirm {
    /// Where and when the KOL confirmed the wallet.
    #[serde(default)]
    where_when: String,
}

#[derive(Deserialize, Default)]
struct Reason {
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
struct Propose {
    kol: String,
    address: String,
    #[serde(default)]
    evidence: String,
}

async fn overview(State(w): State<Shared>, person: Person) -> Result<Response> {
    Ok(Json(w.overview(&person).await?).into_response())
}

/// «Run the wallet pass now»: the same pass the sheet read triggers.
async fn run_pass(State(w): State<Shared>, person: Person) -> Result<Response> {
    if !person.is_staff() {
        return Err(Error::Forbidden);
    }
    let me = w.clone();
    let result = w
        .locked(async move { Ok(me.match_all(&person).await) })
        .await?;
    let overview = w.overview(&person).await?;
    Ok(Json(json!({
        "problems": result.err().map(|e| e.to_string()),
        "overview": overview,
    }))
    .into_response())
}

async fn confirm(
    State(w): State<Shared>,
    person: Person,
    Path(id): Path<i64>,
    Json(body): Json<Confirm>,
) -> Result<Response> {
    let me = w.clone();
    let c = w
        .locked(async move { me.confirm(&person, id, &body.where_when).await })
        .await?;
    Ok(Json(c).into_response())
}

async fn reject(
    State(w): State<Shared>,
    person: Person,
    Path(id): Path<i64>,
    body: Option<Json<Reason>>,
) -> Result<Response> {
    let reason = body.and_then(|Json(b)| b.reason);
    let me = w.clone();
    let c = w
        .locked(async move { me.reject(&person, id, reason.as_deref()).await })
        .await?;
    Ok(Json(c).into_response())
}

async fn revoke(
    State(w): State<Shared>,
    person: Person,
    Path(id): Path<i64>,
    body: Option<Json<Reason>>,
) -> Result<Response> {
    let reason = body.and_then(|Json(b)| b.reason).unwrap_or_default();
    let me = w.clone();
    let c = w
        .locked(async move { me.revoke(&person, id, &reason).await })
        .await?;
    Ok(Json(c).into_response())
}

async fn make_main(
    State(w): State<Shared>,
    person: Person,
    Path(id): Path<i64>,
) -> Result<Response> {
    let me = w.clone();
    let c = w
        .locked(async move { me.set_main(&person, id).await })
        .await?;
    Ok(Json(c).into_response())
}

async fn propose(
    State(w): State<Shared>,
    person: Person,
    Json(b): Json<Propose>,
) -> Result<Response> {
    let me = w.clone();
    let c = w
        .locked(async move { me.propose(&person, &b.kol, &b.address, &b.evidence).await })
        .await?;
    Ok(Json(c).into_response())
}

pub(crate) fn routes(w: Shared) -> Vec<Route> {
    let at = |path: &str, h: axum::routing::MethodRouter<Shared>| {
        Route::new(Tag::Staff, path, h.with_state(w.clone()))
    };
    vec![
        at("/api/team/wallets", get(overview)),
        at("/api/team/wallets/pass", post(run_pass)),
        at("/api/team/wallets/propose", post(propose)),
        at("/api/team/wallets/{id}/confirm", post(confirm)),
        at("/api/team/wallets/{id}/reject", post(reject)),
        at("/api/team/wallets/{id}/revoke", post(revoke)),
        at("/api/team/wallets/{id}/main", post(make_main)),
    ]
}
