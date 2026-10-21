//! Auth routes, cookies, the Origin check and the person of a request.

use crate::service::{AuthService, LoginError, SESSION_DAYS, StaffError, parse_role};
use api_auth::{AuthApi, AuthError, Staff};
use arena_platform::{Person, PersonOfRequest, Route, Tag};
use async_trait::async_trait;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

/// Name of the session cookie.
pub const COOKIE: &str = "arena_session";

/// The session token in the request's `Cookie` headers, if any.
pub fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_string())
        .filter(|v| !v.is_empty())
}

/// `Set-Cookie` value for a new session: HttpOnly, Secure, SameSite=Strict, 14 days.
pub fn session_cookie(token: &str) -> String {
    format!(
        "{COOKIE}={token}; Path=/; Max-Age={}; HttpOnly; Secure; SameSite=Strict",
        SESSION_DAYS * 24 * 3600
    )
}

/// `Set-Cookie` value that removes the cookie.
pub fn clearing_cookie() -> String {
    format!("{COOKIE}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict")
}

/// Client address for the login rate limit. The server sits behind a proxy and does not
/// expose the peer address to plugins, so this reads the proxy's `X-Forwarded-For` (the last
/// entry: the one the nearest proxy added), then `X-Real-IP`. Without either, every client
/// shares the bucket `unknown`.
pub fn client_ip(headers: &HeaderMap) -> String {
    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let real = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty());
    forwarded
        .or(real)
        .map(|v| v.chars().take(64).collect())
        .unwrap_or_else(|| "unknown".into())
}

/// Whether the `Origin` header names this site: its host (and port) equals the request's
/// `X-Forwarded-Host` or `Host`. A missing or `null` Origin is not allowed.
pub fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let host = headers
        .get("x-forwarded-host")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .or_else(|| headers.get(header::HOST).and_then(|v| v.to_str().ok()))
        .map(|h| h.trim().to_ascii_lowercase());
    let Some(host) = host.filter(|h| !h.is_empty()) else {
        return false;
    };
    let authority = origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))
        .map(|rest| rest.trim_end_matches('/').to_ascii_lowercase());
    authority.is_some_and(|a| !a.contains('/') && a == host)
}

/// Middleware: every request that is not GET, HEAD or OPTIONS needs an `Origin` that names
/// this site; otherwise 403. Auth applies it to its own admin routes and to logout. The
/// server applies it to every `/api/admin` route of every plugin (see the report).
pub async fn origin_guard(req: Request, next: Next) -> Response {
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe && !origin_allowed(req.headers()) {
        return error(
            StatusCode::FORBIDDEN,
            "bad_origin",
            "Request origin not allowed",
        );
    }
    next.run(req).await
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

fn internal(what: &str, e: impl std::fmt::Display) -> Response {
    tracing::error!(error = %e, "auth: {what}");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal",
        "Something went wrong. Try again.",
    )
}

#[allow(clippy::result_large_err)] // the error is a ready HTTP response
fn parse<T: for<'de> Deserialize<'de>>(body: &Bytes) -> Result<T, Response> {
    serde_json::from_slice(body).map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "The request body is not valid",
        )
    })
}

fn staff_json(s: &Staff) -> Value {
    json!({ "id": s.id, "login": s.login, "role": s.role })
}

#[derive(Clone)]
struct HttpState {
    auth: Arc<AuthService>,
}

#[derive(Deserialize)]
struct LoginBody {
    login: String,
    password: String,
}

async fn login(State(st): State<HttpState>, headers: HeaderMap, body: Bytes) -> Response {
    let body: LoginBody = match parse(&body) {
        Ok(b) => b,
        Err(r) => return r,
    };
    match st
        .auth
        .login(&client_ip(&headers), &body.login, &body.password)
        .await
    {
        Ok((staff, token)) => {
            let mut res = Json(staff_json(&staff)).into_response();
            if let Ok(v) = HeaderValue::from_str(&session_cookie(&token)) {
                res.headers_mut().insert(header::SET_COOKIE, v);
            }
            res
        }
        Err(LoginError::BadCredentials) => error(
            StatusCode::UNAUTHORIZED,
            "bad_credentials",
            "Wrong login or password",
        ),
        Err(LoginError::Locked { minutes }) => {
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            let mut res = error(
                StatusCode::TOO_MANY_REQUESTS,
                "locked",
                &format!("Too many tries. Try again in {minutes} {unit}."),
            );
            if let Ok(v) = HeaderValue::from_str(&(minutes * 60).to_string()) {
                res.headers_mut().insert(header::RETRY_AFTER, v);
            }
            res
        }
        Err(LoginError::Internal(e)) => internal("login", e),
    }
}

#[allow(clippy::result_large_err)] // the error is a ready HTTP response
async fn current(st: &HttpState, headers: &HeaderMap) -> Result<(Staff, String), Response> {
    let token = session_token(headers)
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "signed_out", "Sign in required"))?;
    match st.auth.who(&token).await {
        Ok(Some(staff)) => Ok((staff, token)),
        Ok(None) => Err(error(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in required",
        )),
        Err(e) => Err(internal("who", e)),
    }
}

async fn logout(State(st): State<HttpState>, headers: HeaderMap) -> Response {
    let (staff, token) = match current(&st, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(e) = st.auth.logout(&staff, &token).await {
        return internal("logout", e);
    }
    let mut res = Json(json!({})).into_response();
    if let Ok(v) = HeaderValue::from_str(&clearing_cookie()) {
        res.headers_mut().insert(header::SET_COOKIE, v);
    }
    res
}

async fn me(State(st): State<HttpState>, headers: HeaderMap) -> Response {
    match current(&st, &headers).await {
        Ok((staff, _)) => Json(staff_json(&staff)).into_response(),
        Err(r) => r,
    }
}

/// Admin handlers check the role themselves too, in case a route is ever mounted without
/// the server's tag guard.
fn require_admin(person: &Person) -> Option<Response> {
    if person.is_admin() {
        None
    } else if person.is_staff() {
        Some(error(StatusCode::FORBIDDEN, "forbidden", "Admins only"))
    } else {
        Some(error(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in required",
        ))
    }
}

async fn list_staff(State(st): State<HttpState>, person: Person) -> Response {
    if let Some(r) = require_admin(&person) {
        return r;
    }
    match st.auth.list_staff().await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => internal("list staff", e),
    }
}

#[derive(Deserialize)]
struct AddBody {
    login: String,
    role: String,
    password: String,
}

fn staff_error(e: StaffError) -> Response {
    match e {
        StaffError::Invalid(m) => error(StatusCode::BAD_REQUEST, "invalid", m),
        StaffError::Taken => error(StatusCode::CONFLICT, "taken", "This login is already used"),
        StaffError::NotFound => error(StatusCode::NOT_FOUND, "not_found", "No such staff member"),
        StaffError::SelfRevoke => error(
            StatusCode::BAD_REQUEST,
            "self_revoke",
            "You cannot revoke your own access",
        ),
        StaffError::AlreadyRevoked => error(
            StatusCode::CONFLICT,
            "already_revoked",
            "Access is already revoked",
        ),
        StaffError::JournalFailed => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "journal_failed",
            "The change was saved, but writing it to the journal failed",
        ),
        StaffError::Internal(e) => internal("staff", e),
    }
}

async fn add_staff(State(st): State<HttpState>, person: Person, body: Bytes) -> Response {
    if let Some(r) = require_admin(&person) {
        return r;
    }
    let body: AddBody = match parse(&body) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let Some(role) = parse_role(&body.role) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Role must be staff or admin",
        );
    };
    match st
        .auth
        .add_staff(&person, &body.login, role, &body.password)
        .await
    {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(e) => staff_error(e),
    }
}

async fn revoke_staff(
    State(st): State<HttpState>,
    person: Person,
    Path(id): Path<i64>,
) -> Response {
    if let Some(r) = require_admin(&person) {
        return r;
    }
    match st.auth.revoke_staff(&person, id).await {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => staff_error(e),
    }
}

/// The plugin's routes with their tags.
pub fn routes(auth: Arc<AuthService>) -> Vec<Route> {
    let st = HttpState { auth };
    let guard = || middleware::from_fn(origin_guard);
    vec![
        Route::new(
            Tag::Public,
            "/api/auth/login",
            post(login).with_state(st.clone()),
        ),
        Route::new(
            Tag::Staff,
            "/api/auth/logout",
            post(logout).with_state(st.clone()).layer(guard()),
        ),
        Route::new(Tag::Staff, "/api/auth/me", get(me).with_state(st.clone())),
        Route::new(
            Tag::Admin,
            "/api/admin/staff",
            get(list_staff)
                .post(add_staff)
                .with_state(st.clone())
                .layer(guard()),
        ),
        Route::new(
            Tag::Admin,
            "/api/admin/staff/{id}/revoke",
            post(revoke_staff).with_state(st).layer(guard()),
        ),
    ]
}

/// Fills `platform/person_of_request`: the person behind the session cookie, else nobody.
pub struct SessionPerson(pub Arc<AuthService>);

#[async_trait]
impl PersonOfRequest for SessionPerson {
    async fn person(&self, headers: &HeaderMap) -> Person {
        let Some(token) = session_token(headers) else {
            return Person::NOBODY;
        };
        match self.0.who(&token).await {
            Ok(Some(staff)) => staff.person(),
            Ok(None) => Person::NOBODY,
            Err(e) => {
                tracing::error!(error = %e, "auth: session lookup failed");
                Person::NOBODY
            }
        }
    }
}

/// The [`AuthApi`] other plugins call.
pub struct AuthImpl(pub Arc<AuthService>);

#[async_trait]
impl AuthApi for AuthImpl {
    async fn who(&self, _person: &Person, session: &str) -> Result<Option<Staff>, AuthError> {
        self.0.who(session).await.map_err(|e| AuthError::Internal {
            message: e.to_string(),
        })
    }
}
