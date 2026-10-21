//! Auth tests: hashes, sessions, lockout, revoke, roles per tag, Origin check, first admin.
//! Database tests use a throwaway `TestDb`; the journal is a fake that remembers its calls.

use api_auth::AuthApi;
use api_journal::{Entry, EntryId, JournalApi, JournalError};
use arena_platform::{Config, Ctx, Person, PersonOfRequest, Role, Tag, testing::TestDb};
use argon2::Params;
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Duration, TimeZone, Utc};
use plugin_auth::{
    AuthService, LoginError, MIGRATOR, StaffError, http,
    service::{self, Clock},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const HOST: &str = "arena.test";
const ORIGIN: &str = "https://arena.test";

type Call = (
    Option<i64>,
    String,
    String,
    String,
    Option<Value>,
    Option<Value>,
);

#[derive(Default)]
struct FakeJournal {
    calls: Mutex<Vec<Call>>,
}

#[async_trait]
impl JournalApi for FakeJournal {
    async fn record(
        &self,
        person: &Person,
        block: &str,
        object: &str,
        action: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) -> Result<EntryId, JournalError> {
        let mut calls = self.calls.lock().unwrap();
        calls.push((
            person.id,
            block.into(),
            object.into(),
            action.into(),
            before,
            after,
        ));
        Ok(calls.len() as i64)
    }

    async fn history(&self, _: &Person, _: &str, _: &str) -> Result<Vec<Entry>, JournalError> {
        Ok(vec![])
    }
}

struct Env {
    db: TestDb,
    auth: Arc<AuthService>,
    journal: Arc<FakeJournal>,
    now: Arc<Mutex<DateTime<Utc>>>,
}

impl Env {
    async fn new() -> Env {
        let db = TestDb::with_plugins(&[("auth", &MIGRATOR)]).await;
        let journal = Arc::new(FakeJournal::default());
        let ctx = Ctx::new(db.pool().clone(), Config::default(), "test").unwrap();
        ctx.apis
            .provide::<dyn JournalApi>(
                "journal",
                vec![("default", journal.clone() as Arc<dyn JournalApi>)],
                Arc::new(api_journal::stub::Stub),
            )
            .unwrap();
        let api = ctx.apis.require::<dyn JournalApi>("auth").unwrap();
        let now = Arc::new(Mutex::new(
            Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap(),
        ));
        let clock: Clock = {
            let now = now.clone();
            Arc::new(move || *now.lock().unwrap())
        };
        // Cheap argon2 parameters keep debug-build tests fast; production uses Params::DEFAULT.
        let params = Params::new(8, 1, 1, None).unwrap();
        let auth = Arc::new(AuthService::with(db.pool().clone(), api, clock, params));
        Env {
            db,
            auth,
            journal,
            now,
        }
    }

    fn advance(&self, d: Duration) {
        *self.now.lock().unwrap() += d;
    }

    fn pool(&self) -> &PgPool {
        self.db.pool()
    }

    fn actions(&self) -> Vec<String> {
        self.journal
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.3.clone())
            .collect()
    }

    /// An admin and a staff member, created straight through the service.
    async fn people(&self) -> (i64, i64) {
        let admin = self
            .auth
            .ensure_first_admin("boss", "boss-password-1")
            .await
            .unwrap()
            .unwrap();
        let staff = self
            .auth
            .add_staff(
                &Person::admin(admin),
                "anna",
                Role::Staff,
                "anna-password-1",
            )
            .await
            .unwrap()
            .id;
        (admin, staff)
    }

    async fn session_count(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM auth.session")
            .fetch_one(self.pool())
            .await
            .unwrap()
    }

    /// The routes behind a guard that works like the server's tag guard.
    fn router(&self) -> Router {
        let mut router = Router::default();
        for route in http::routes(self.auth.clone()) {
            let guard = middleware::from_fn_with_state((self.auth.clone(), route.tag), tag_guard);
            router = router.route(&route.path, route.handler.layer(guard));
        }
        router
    }
}

async fn tag_guard(
    State((auth, tag)): State<(Arc<AuthService>, Tag)>,
    mut req: Request,
    next: Next,
) -> Response {
    let person = http::SessionPerson(auth).person(req.headers()).await;
    if !tag.admits(&person) {
        let status = if person.is_staff() {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::UNAUTHORIZED
        };
        return (status, "closed").into_response();
    }
    req.extensions_mut().insert(person);
    next.run(req).await
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

impl Reply {
    fn code(&self) -> String {
        self.body["error"]["code"]
            .as_str()
            .unwrap_or("")
            .to_string()
    }

    fn token(&self) -> String {
        let cookie = self
            .headers
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        cookie
            .strip_prefix("arena_session=")
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }
}

async fn call(
    router: &Router,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> Reply {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, HOST);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let body = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let res = router
        .clone()
        .oneshot(req.body(body).unwrap())
        .await
        .unwrap();
    let (parts, body) = res.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    Reply {
        status: parts.status,
        headers: parts.headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

async fn login(router: &Router, ip: &str, login: &str, password: &str) -> Reply {
    call(
        router,
        "POST",
        "/api/auth/login",
        &[("x-forwarded-for", ip)],
        Some(json!({"login": login, "password": password})),
    )
    .await
}

fn cookie(token: &str) -> String {
    format!("arena_session={token}")
}

// ---- hashes -----------------------------------------------------------------------------

#[test]
fn argon2id_hash_and_verify() {
    let params = Params::new(8, 1, 1, None).unwrap();
    let hash = service::hash_password(&params, "correct horse battery").unwrap();
    assert!(hash.starts_with("$argon2id$"), "{hash}");
    assert!(!hash.contains("correct horse"));
    assert!(service::verify_password(&hash, "correct horse battery"));
    assert!(!service::verify_password(&hash, "correct horse batter"));
    assert!(!service::verify_password(&hash, ""));
    assert!(!service::verify_password(
        "not a hash",
        "correct horse battery"
    ));
    // A fresh salt per hash.
    assert_ne!(
        hash,
        service::hash_password(&params, "correct horse battery").unwrap()
    );
    // Production parameters are Argon2id too.
    let prod = service::hash_password(&Params::DEFAULT, "x").unwrap();
    assert!(
        prod.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
        "{prod}"
    );
}

#[test]
fn tokens_are_32_random_bytes() {
    let (a, b) = (service::new_token(), service::new_token());
    assert_eq!(a.len(), 64);
    assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a, b);
}

// ---- login and session -------------------------------------------------------------------

#[tokio::test]
async fn login_sets_the_cookie_and_stores_only_its_hash() {
    let env = Env::new().await;
    env.people().await;
    let router = env.router();
    let res = login(&router, "10.0.0.1", "Anna", "anna-password-1").await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["login"], "anna");
    assert_eq!(res.body["role"], "staff");
    let set_cookie = res
        .headers
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    for flag in [
        "HttpOnly",
        "Secure",
        "SameSite=Strict",
        "Max-Age=1209600",
        "Path=/",
    ] {
        assert!(set_cookie.contains(flag), "{flag} missing in {set_cookie}");
    }
    let token = res.token();
    assert_eq!(token.len(), 64);
    let stored: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM auth.session")
        .fetch_all(env.pool())
        .await
        .unwrap();
    let digest: String = Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(stored, [digest]);
    assert!(!stored[0].contains(&token));

    let me = call(
        &router,
        "GET",
        "/api/auth/me",
        &[("cookie", &cookie(&token))],
        None,
    )
    .await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["login"], "anna");
    assert!(env.actions().contains(&"login".to_string()));
    env.db.close().await;
}

#[tokio::test]
async fn wrong_password_and_unknown_login_get_the_same_answer() {
    let env = Env::new().await;
    env.people().await;
    let router = env.router();
    let bad = login(&router, "10.0.0.2", "anna", "nope-nope-nope").await;
    let ghost = login(&router, "10.0.0.2", "ghost", "nope-nope-nope").await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert_eq!(bad.body, ghost.body);
    assert_eq!(bad.body["error"]["message"], "Wrong login or password");
    assert_eq!(bad.code(), "bad_credentials");
    assert!(bad.headers.get(header::SET_COOKIE).is_none());
    let garbage = call(
        &router,
        "POST",
        "/api/auth/login",
        &[],
        Some(json!({"login": 1})),
    )
    .await;
    assert_eq!(garbage.status, StatusCode::BAD_REQUEST);
    env.db.close().await;
}

#[tokio::test]
async fn five_failures_lock_the_address_for_fifteen_minutes() {
    let env = Env::new().await;
    env.people().await;
    let router = env.router();
    for _ in 0..5 {
        let r = login(&router, "10.0.0.3", "anna", "wrong-wrong-wrong").await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    // Even the right password is refused now, with the time left.
    let locked = login(&router, "10.0.0.3", "anna", "anna-password-1").await;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(locked.code(), "locked");
    assert_eq!(
        locked.body["error"]["message"],
        "Too many tries. Try again in 15 minutes."
    );
    assert_eq!(locked.headers.get(header::RETRY_AFTER).unwrap(), "900");
    // Another address is not affected.
    assert_eq!(
        login(&router, "10.0.0.4", "anna", "anna-password-1")
            .await
            .status,
        StatusCode::OK
    );

    env.advance(Duration::minutes(14));
    let still = login(&router, "10.0.0.3", "anna", "anna-password-1").await;
    assert_eq!(still.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        still.body["error"]["message"],
        "Too many tries. Try again in 1 minute."
    );

    env.advance(Duration::minutes(1) + Duration::seconds(1));
    assert_eq!(
        login(&router, "10.0.0.3", "anna", "anna-password-1")
            .await
            .status,
        StatusCode::OK
    );
    env.db.close().await;
}

#[tokio::test]
async fn failures_older_than_fifteen_minutes_do_not_count() {
    let env = Env::new().await;
    env.people().await;
    let router = env.router();
    for _ in 0..4 {
        login(&router, "10.0.0.5", "anna", "wrong-wrong-wrong").await;
    }
    env.advance(Duration::minutes(16));
    for _ in 0..4 {
        login(&router, "10.0.0.5", "anna", "wrong-wrong-wrong").await;
    }
    assert_eq!(
        login(&router, "10.0.0.5", "anna", "anna-password-1")
            .await
            .status,
        StatusCode::OK
    );
    env.db.close().await;
}

#[tokio::test]
async fn sessions_expire_after_fourteen_days() {
    let env = Env::new().await;
    env.people().await;
    let router = env.router();
    let token = login(&router, "10.0.0.6", "anna", "anna-password-1")
        .await
        .token();
    let headers = [("cookie", cookie(&token))];
    let me = |r: &Router| {
        let r = r.clone();
        let c = headers[0].1.clone();
        async move {
            call(&r, "GET", "/api/auth/me", &[("cookie", &c)], None)
                .await
                .status
        }
    };
    assert_eq!(me(&router).await, StatusCode::OK);
    env.advance(Duration::days(14) - Duration::seconds(1));
    assert_eq!(me(&router).await, StatusCode::OK);
    assert!(env.auth.who(&token).await.unwrap().is_some());
    env.advance(Duration::seconds(2));
    assert_eq!(me(&router).await, StatusCode::UNAUTHORIZED);
    assert!(env.auth.who(&token).await.unwrap().is_none());
    // The next login clears expired rows.
    login(&router, "10.0.0.6", "anna", "anna-password-1").await;
    assert_eq!(env.session_count().await, 1);
    env.db.close().await;
}

#[tokio::test]
async fn logout_ends_the_session_and_clears_the_cookie() {
    let env = Env::new().await;
    env.people().await;
    let router = env.router();
    let token = login(&router, "10.0.0.7", "anna", "anna-password-1")
        .await
        .token();
    let c = cookie(&token);
    let no_origin = call(&router, "POST", "/api/auth/logout", &[("cookie", &c)], None).await;
    assert_eq!(no_origin.status, StatusCode::FORBIDDEN);
    let out = call(
        &router,
        "POST",
        "/api/auth/logout",
        &[("cookie", &c), ("origin", ORIGIN)],
        None,
    )
    .await;
    assert_eq!(out.status, StatusCode::OK);
    let cleared = out
        .headers
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        cleared.starts_with("arena_session=;") && cleared.contains("Max-Age=0"),
        "{cleared}"
    );
    assert_eq!(
        call(&router, "GET", "/api/auth/me", &[("cookie", &c)], None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(env.session_count().await, 0);
    assert!(env.actions().contains(&"logout".to_string()));
    env.db.close().await;
}

// ---- staff management --------------------------------------------------------------------

#[tokio::test]
async fn revoke_deletes_that_persons_sessions_at_once() {
    let env = Env::new().await;
    let (admin, staff) = env.people().await;
    let router = env.router();
    let a1 = login(&router, "10.0.0.8", "anna", "anna-password-1")
        .await
        .token();
    let a2 = login(&router, "10.0.0.9", "anna", "anna-password-1")
        .await
        .token();
    let boss = login(&router, "10.0.0.10", "boss", "boss-password-1")
        .await
        .token();
    assert_eq!(env.session_count().await, 3);

    let path = format!("/api/admin/staff/{staff}/revoke");
    let c = cookie(&boss);
    let res = call(
        &router,
        "POST",
        &path,
        &[("cookie", &c), ("origin", ORIGIN)],
        None,
    )
    .await;
    assert_eq!(res.status, StatusCode::OK, "{:?}", res.body);
    // Only the admin's own session is left.
    assert_eq!(env.session_count().await, 1);
    for t in [&a1, &a2] {
        assert!(env.auth.who(t).await.unwrap().is_none());
        assert_eq!(
            call(
                &router,
                "GET",
                "/api/auth/me",
                &[("cookie", &cookie(t))],
                None
            )
            .await
            .status,
            StatusCode::UNAUTHORIZED
        );
    }
    assert!(env.auth.who(&boss).await.unwrap().is_some());
    // The revoked person cannot sign in again, and the login stays reserved.
    assert_eq!(
        login(&router, "10.0.0.11", "anna", "anna-password-1")
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    let again = call(
        &router,
        "POST",
        &path,
        &[("cookie", &c), ("origin", ORIGIN)],
        None,
    )
    .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    let taken = call(
        &router,
        "POST",
        "/api/admin/staff",
        &[("cookie", &c), ("origin", ORIGIN)],
        Some(json!({"login": "anna", "role": "staff", "password": "another-password"})),
    )
    .await;
    assert_eq!(taken.status, StatusCode::CONFLICT);
    // Nobody revokes themselves; unknown ids are 404.
    let own = call(
        &router,
        "POST",
        &format!("/api/admin/staff/{admin}/revoke"),
        &[("cookie", &c), ("origin", ORIGIN)],
        None,
    )
    .await;
    assert_eq!(own.code(), "self_revoke");
    let none = call(
        &router,
        "POST",
        "/api/admin/staff/99999/revoke",
        &[("cookie", &c), ("origin", ORIGIN)],
        None,
    )
    .await;
    assert_eq!(none.status, StatusCode::NOT_FOUND);

    let calls = env.journal.calls.lock().unwrap().clone();
    let revoke = calls.iter().find(|c| c.3 == "revoke").unwrap();
    assert_eq!(
        (revoke.0, revoke.1.as_str(), revoke.2.clone()),
        (Some(admin), "auth", staff.to_string())
    );
    assert_eq!(revoke.5.as_ref().unwrap()["revoked"], true);
    drop(calls);
    env.db.close().await;
}

#[tokio::test]
async fn admin_adds_staff_and_the_journal_never_sees_the_password() {
    let env = Env::new().await;
    env.people().await;
    let router = env.router();
    let boss = cookie(
        &login(&router, "10.0.0.12", "boss", "boss-password-1")
            .await
            .token(),
    );
    let h = [("cookie", boss.as_str()), ("origin", ORIGIN)];

    let ok = call(
        &router,
        "POST",
        "/api/admin/staff",
        &h,
        Some(json!({"login": " Maria.K ", "role": "admin", "password": "temporary-pass-1"})),
    )
    .await;
    assert_eq!(ok.status, StatusCode::CREATED);
    assert_eq!(
        (ok.body["login"].as_str(), ok.body["role"].as_str()),
        (Some("maria.k"), Some("admin"))
    );
    assert!(ok.body.get("password_hash").is_none());
    assert_eq!(
        login(&router, "10.0.0.13", "maria.k", "temporary-pass-1")
            .await
            .body["role"],
        "admin"
    );

    for (body, why) in [
        (
            json!({"login": "ab", "role": "staff", "password": "temporary-pass-1"}),
            "short login",
        ),
        (
            json!({"login": "bad login", "role": "staff", "password": "temporary-pass-1"}),
            "space",
        ),
        (
            json!({"login": "okay", "role": "boss", "password": "temporary-pass-1"}),
            "role",
        ),
        (
            json!({"login": "okay", "role": "staff", "password": "short"}),
            "short password",
        ),
    ] {
        let r = call(&router, "POST", "/api/admin/staff", &h, Some(body)).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{why}");
    }

    let list = call(
        &router,
        "GET",
        "/api/admin/staff",
        &[("cookie", &boss)],
        None,
    )
    .await;
    assert_eq!(list.status, StatusCode::OK);
    let logins: Vec<&str> = list
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["login"].as_str().unwrap())
        .collect();
    assert_eq!(logins, ["boss", "anna", "maria.k"]);
    assert!(!list.body.to_string().contains("argon2"));

    let calls = env.journal.calls.lock().unwrap().clone();
    let add = calls.iter().rev().find(|c| c.3 == "add").unwrap();
    assert_eq!(add.5, Some(json!({"login": "maria.k", "role": "admin"})));
    assert!(!format!("{calls:?}").contains("temporary-pass"));
    drop(calls);
    env.db.close().await;
}

#[tokio::test]
async fn first_admin_only_when_no_active_admin_exists() {
    let env = Env::new().await;
    let id = env
        .auth
        .ensure_first_admin("Root", "first-admin-pass")
        .await
        .unwrap()
        .unwrap();
    // An admin exists: nothing changes, even with other settings.
    assert_eq!(
        env.auth
            .ensure_first_admin("other", "other-password-1")
            .await
            .unwrap(),
        None
    );
    let rows = env.auth.list_staff().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].login.as_str(), rows[0].role),
        ("root", Role::Admin)
    );
    let (staff, _) = env
        .auth
        .login("1.1.1.1", "root", "first-admin-pass")
        .await
        .unwrap();
    assert_eq!(staff.id, id);
    assert!(
        env.journal
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.3 == "first_admin" && c.0.is_none())
    );
    // Invalid login from the settings is an error, not a silent skip.
    sqlx::query("DELETE FROM auth.staff")
        .execute(env.pool())
        .await
        .unwrap();
    assert!(matches!(
        env.auth.ensure_first_admin("x", "first-admin-pass").await,
        Err(StaffError::Invalid(_))
    ));
    env.db.close().await;
}

#[tokio::test]
async fn a_revoked_only_admin_is_restored_from_the_settings() {
    let env = Env::new().await;
    let (admin, _) = env.people().await;
    let other = env
        .auth
        .add_staff(
            &Person::admin(admin),
            "deputy",
            Role::Admin,
            "deputy-password-1",
        )
        .await
        .unwrap()
        .id;
    env.auth
        .revoke_staff(&Person::admin(other), admin)
        .await
        .unwrap();
    env.auth
        .revoke_staff(&Person::admin(admin + 1000), other)
        .await
        .unwrap();
    assert!(matches!(
        env.auth.login("1.1.1.2", "boss", "boss-password-1").await,
        Err(LoginError::BadCredentials)
    ));
    let back = env
        .auth
        .ensure_first_admin("boss", "new-boss-password")
        .await
        .unwrap();
    assert_eq!(back, Some(admin));
    assert!(
        env.auth
            .login("1.1.1.2", "boss", "new-boss-password")
            .await
            .is_ok()
    );
    env.db.close().await;
}

// ---- roles per tag and the person of a request -------------------------------------------

#[tokio::test]
async fn role_per_tag() {
    let env = Env::new().await;
    let (admin, staff) = env.people().await;
    let router = env.router();
    let anna = cookie(
        &login(&router, "10.0.1.1", "anna", "anna-password-1")
            .await
            .token(),
    );
    let boss = cookie(
        &login(&router, "10.0.1.2", "boss", "boss-password-1")
            .await
            .token(),
    );

    // Person of request, through the platform slot implementation.
    let person = |c: Option<&str>| {
        let auth = env.auth.clone();
        let mut h = HeaderMap::new();
        if let Some(c) = c {
            h.insert(header::COOKIE, c.parse().unwrap());
        }
        async move { http::SessionPerson(auth).person(&h).await }
    };
    assert_eq!(person(None).await, Person::NOBODY);
    assert_eq!(person(Some("arena_session=deadbeef")).await, Person::NOBODY);
    assert_eq!(
        person(Some(&format!("a=b; {anna}; c=d"))).await,
        Person::staff(staff)
    );
    assert_eq!(person(Some(&boss)).await, Person::admin(admin));

    // Tags: public for all, staff routes need a session, admin routes need an admin.
    let origin = ("origin", ORIGIN);
    let add = json!({"login": "newbie", "role": "staff", "password": "newbie-password"});
    let status = |r: Reply| r.status;
    assert_eq!(
        status(call(&router, "GET", "/api/auth/me", &[], None).await),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(call(&router, "GET", "/api/auth/me", &[("cookie", &anna)], None).await),
        StatusCode::OK
    );
    assert_eq!(
        status(call(&router, "GET", "/api/admin/staff", &[], None).await),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(
            call(
                &router,
                "GET",
                "/api/admin/staff",
                &[("cookie", &anna)],
                None
            )
            .await
        ),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status(
            call(
                &router,
                "POST",
                "/api/admin/staff",
                &[("cookie", &anna), origin],
                Some(add.clone())
            )
            .await
        ),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status(
            call(
                &router,
                "POST",
                &format!("/api/admin/staff/{admin}/revoke"),
                &[("cookie", &anna), origin],
                None
            )
            .await
        ),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status(
            call(
                &router,
                "GET",
                "/api/admin/staff",
                &[("cookie", &boss)],
                None
            )
            .await
        ),
        StatusCode::OK
    );
    assert_eq!(
        status(
            call(
                &router,
                "POST",
                "/api/admin/staff",
                &[("cookie", &boss), origin],
                Some(add)
            )
            .await
        ),
        StatusCode::CREATED
    );

    // The handlers refuse on their own when mounted without the guard.
    let bare = http::routes(env.auth.clone())
        .into_iter()
        .fold(Router::default(), |r, route| {
            r.route(&route.path, route.handler)
        });
    assert_eq!(
        status(call(&bare, "GET", "/api/admin/staff", &[], None).await),
        StatusCode::UNAUTHORIZED
    );

    // Through the API trait.
    let token = login(&router, "10.0.1.3", "anna", "anna-password-1")
        .await
        .token();
    let api = http::AuthImpl(env.auth.clone());
    let who = api.who(&Person::NOBODY, &token).await.unwrap().unwrap();
    assert_eq!(
        (who.login.as_str(), who.person()),
        ("anna", Person::staff(staff))
    );
    assert!(api.who(&Person::NOBODY, "nope").await.unwrap().is_none());
    env.db.close().await;
}

// ---- Origin check ------------------------------------------------------------------------

#[tokio::test]
async fn origin_check_on_non_get_admin_requests() {
    let env = Env::new().await;
    let (_, staff) = env.people().await;
    let router = env.router();
    let boss = cookie(
        &login(&router, "10.0.2.1", "boss", "boss-password-1")
            .await
            .token(),
    );
    let add = || Some(json!({"login": "carol", "role": "staff", "password": "carol-password-1"}));
    let revoke = format!("/api/admin/staff/{staff}/revoke");

    for headers in [
        vec![("cookie", boss.as_str())],
        vec![
            ("cookie", boss.as_str()),
            ("origin", "https://evil.example"),
        ],
        vec![("cookie", boss.as_str()), ("origin", "null")],
        vec![
            ("cookie", boss.as_str()),
            ("origin", "https://arena.test.evil.example"),
        ],
    ] {
        let r = call(&router, "POST", "/api/admin/staff", &headers, add()).await;
        assert_eq!(
            (r.status, r.code().as_str()),
            (StatusCode::FORBIDDEN, "bad_origin"),
            "{headers:?}"
        );
        let r = call(&router, "POST", &revoke, &headers, None).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{headers:?}");
    }
    // Nothing was written by the refused requests.
    assert_eq!(env.auth.list_staff().await.unwrap().len(), 2);

    // GET needs no Origin.
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/admin/staff",
            &[("cookie", &boss)],
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    // The site's own Origin passes, also behind a proxy that forwards the public host.
    let ok = call(
        &router,
        "POST",
        "/api/admin/staff",
        &[("cookie", &boss), ("origin", ORIGIN)],
        add(),
    )
    .await;
    assert_eq!(ok.status, StatusCode::CREATED);
    let proxied = call(
        &router,
        "POST",
        &revoke,
        &[
            ("cookie", &boss),
            ("origin", "https://kols.example.com"),
            ("x-forwarded-host", "kols.example.com"),
        ],
        None,
    )
    .await;
    assert_eq!(proxied.status, StatusCode::OK);
    env.db.close().await;
}

#[test]
fn origin_allowed_compares_the_authority() {
    let h = |pairs: &[(&str, &str)]| {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        m
    };
    assert!(http::origin_allowed(&h(&[
        ("host", "a.test"),
        ("origin", "https://a.test")
    ])));
    assert!(http::origin_allowed(&h(&[
        ("host", "a.test:8080"),
        ("origin", "http://A.test:8080")
    ])));
    assert!(!http::origin_allowed(&h(&[
        ("host", "a.test"),
        ("origin", "http://a.test:8080")
    ])));
    assert!(!http::origin_allowed(&h(&[("host", "a.test")])));
    assert!(!http::origin_allowed(&h(&[("origin", "https://a.test")])));
    assert!(!http::origin_allowed(&h(&[
        ("host", "a.test"),
        ("origin", "null")
    ])));
    assert!(!http::origin_allowed(&h(&[
        ("host", "a.test"),
        ("origin", "https://a.test/path")
    ])));
    assert!(!http::origin_allowed(&h(&[
        ("host", "a.test"),
        ("origin", "ftp://a.test")
    ])));
}

#[test]
fn cookie_and_address_parsing() {
    let mut h = HeaderMap::new();
    h.append(
        header::COOKIE,
        "theme=dark; arena_session=abc123".parse().unwrap(),
    );
    assert_eq!(http::session_token(&h).as_deref(), Some("abc123"));
    let mut h = HeaderMap::new();
    h.append(header::COOKIE, "other=1".parse().unwrap());
    h.append(
        header::COOKIE,
        "xarena_session=no; arena_session=".parse().unwrap(),
    );
    assert_eq!(http::session_token(&h), None);

    let mut h = HeaderMap::new();
    assert_eq!(http::client_ip(&h), "unknown");
    h.insert("x-real-ip", "9.9.9.9".parse().unwrap());
    assert_eq!(http::client_ip(&h), "9.9.9.9");
    h.insert("x-forwarded-for", "6.6.6.6, 7.7.7.7".parse().unwrap());
    assert_eq!(http::client_ip(&h), "7.7.7.7");
}
