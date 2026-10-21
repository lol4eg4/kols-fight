//! Server assembly against a real throwaway database, plus a fake plugin for tag guards
//! and page meta.

use arena_platform::{
    Config, Ctx, Person, PersonOfRequest, Plugin, PluginEntry, Route, StartError, Tag,
    page::{MetaTag, PageMeta},
    testing::TestDb,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use serde_json::{Value, json};
use server::{APP, AppState, PLUGINS, TAGS};
use sqlx::migrate::Migrator;
use std::{path::PathBuf, sync::Arc};
use tower::ServiceExt;

fn dist(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("server-it-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("index.html"),
        "<html><head><title>KOL Arena</title></head><body></body></html>",
    )
    .unwrap();
    dir
}

fn remove_dist(dir: &PathBuf) {
    std::fs::remove_file(dir.join("index.html")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

async fn call(app: &Router, req: Request) -> (StatusCode, String) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn all_plugins_migrate_and_health_reports_them() {
    let db = TestDb::new().await;
    let pool = db.pool().clone();
    let ctx = Ctx::new(pool.clone(), Config::default(), APP).unwrap();
    let plugins = server::build_plugins(&ctx, &PLUGINS).unwrap();
    let dir = dist("health");
    let state = AppState {
        pool: pool.clone(),
        commit: None,
        plugins: Arc::new(plugins.clone()),
        dist: dir.clone(),
    };
    let app = server::app(state.clone(), Router::new());

    // Before migrating: every plugin has its migrations pending.
    let req = Request::get("/api/health").body(Body::empty()).unwrap();
    let (status, body) = call(&app, req).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");

    server::migrate(&pool, &plugins).await.unwrap();
    server::install(&ctx, &plugins).await.unwrap();
    // Running again changes nothing.
    server::migrate(&pool, &plugins).await.unwrap();
    let (routes, _) = server::plugin_routes(&ctx, &plugins, &TAGS).unwrap();
    assert!(server::jobs(&ctx, &plugins).is_ok());
    let app = server::app(state, routes);

    let req = Request::get("/api/health").body(Body::empty()).unwrap();
    let (status, body) = call(&app, req).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["ok"], json!(true));
    assert_eq!(body["db"], json!(true));
    assert_eq!(body["migration"]["pending"], json!(0));
    let per_plugin = body["migration"]["plugins"].as_object().unwrap();
    assert_eq!(per_plugin.len(), 21);
    for (name, v) in per_plugin {
        assert_eq!(v["pending"], json!(0), "{name}");
    }
    // Each plugin got its own schema.
    let schemas: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_name = '_sqlx_migrations' \
         AND table_schema <> 'public'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(schemas, 21);

    remove_dist(&dir);
    db.close().await;
}

// Fake plugin "demo": one route per tag, page meta for /demo.

const DEMO_MANIFEST: &str = r#"
name = "demo"
layer = 0
version = "0.1.0"
tags = ["public", "staff", "admin"]
uses = []
emits = []
listens = []
slots = []
fills = []
widgets = []
screens = []
"#;

const BAD_MANIFEST: &str = r#"
name = "bad"
layer = 0
version = "0.1.0"
tags = ["public"]
uses = []
emits = []
listens = []
slots = []
fills = []
widgets = []
screens = []
"#;

struct Demo;

/// Fake «person of request»: header `x-test-role`.
struct HeaderPerson;

#[async_trait]
impl PersonOfRequest for HeaderPerson {
    async fn person(&self, headers: &HeaderMap) -> Person {
        match headers.get("x-test-role").and_then(|v| v.to_str().ok()) {
            Some("staff") => Person::staff(1),
            Some("admin") => Person::admin(2),
            _ => Person::NOBODY,
        }
    }
}

#[async_trait]
impl Plugin for Demo {
    fn name(&self) -> &'static str {
        "demo"
    }
    fn migrations(&self) -> &'static Migrator {
        &server::MIGRATOR
    }
    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        vec![
            Route::new(Tag::Public, "/api/demo/open", get(|| async { "open" })),
            Route::new(
                Tag::Staff,
                "/api/demo/team",
                get(|person: Person| async move { format!("team {:?}", person.id) }),
            ),
            Route::new(Tag::Admin, "/api/demo/admin", get(|| async { "admin" })),
        ]
    }
    async fn page_meta(&self, path: &str, query: Option<&str>) -> Option<PageMeta> {
        (path == "/demo").then(|| PageMeta {
            title: Some(format!("Demo <{}>", query.unwrap_or(""))),
            tags: vec![MetaTag::property("og:title", "Demo & co")],
        })
    }
}

struct Bad;

#[async_trait]
impl Plugin for Bad {
    fn name(&self) -> &'static str {
        "bad"
    }
    fn migrations(&self) -> &'static Migrator {
        &server::MIGRATOR
    }
    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        vec![Route::new(Tag::Admin, "/api/bad", get(|| async { "x" }))]
    }
}

fn build_demo(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    ctx.slots
        .fill::<dyn PersonOfRequest>("demo", Arc::new(HeaderPerson))?;
    Ok(Arc::new(Demo))
}

fn build_bad(_ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    Ok(Arc::new(Bad))
}

fn lazy_ctx() -> Ctx {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://x@127.0.0.1:1/x")
        .unwrap();
    Ctx::new(pool, Config::default(), APP).unwrap()
}

fn demo_app(tags: &[Tag], dir: PathBuf) -> (Router, Vec<server::Mounted>) {
    let ctx = lazy_ctx();
    let entries = [PluginEntry {
        manifest: DEMO_MANIFEST,
        build: build_demo,
    }];
    let plugins = server::build_plugins(&ctx, &entries).unwrap();
    let (routes, mounted) = server::plugin_routes(&ctx, &plugins, tags).unwrap();
    let state = AppState {
        pool: ctx.pool.clone(),
        commit: None,
        plugins: Arc::new(plugins),
        dist: dir,
    };
    (server::app(state, routes), mounted)
}

fn get_as(path: &str, role: Option<&str>) -> Request {
    let mut req = Request::get(path);
    if let Some(role) = role {
        req = req.header("x-test-role", role);
    }
    req.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn tag_guard_checks_person() {
    let dir = dist("guard");
    let (app, mounted) = demo_app(&TAGS, dir.clone());
    assert_eq!(mounted.len(), 3);
    let cases = [
        ("/api/demo/open", None, 200),
        ("/api/demo/team", None, 401),
        ("/api/demo/team", Some("staff"), 200),
        ("/api/demo/team", Some("admin"), 200),
        ("/api/demo/admin", None, 401),
        ("/api/demo/admin", Some("staff"), 403),
        ("/api/demo/admin", Some("admin"), 200),
    ];
    for (path, role, status) in cases {
        let (got, body) = call(&app, get_as(path, role)).await;
        assert_eq!(got.as_u16(), status, "{path} as {role:?}: {body}");
    }
    let (_, body) = call(&app, get_as("/api/demo/team", Some("staff"))).await;
    assert_eq!(body, "team Some(1)");
    remove_dist(&dir);
}

#[tokio::test]
async fn only_served_tags_are_mounted() {
    let dir = dist("tags");
    let (app, mounted) = demo_app(&[Tag::Public], dir.clone());
    assert_eq!(mounted.len(), 1);
    assert_eq!(mounted[0].path, "/api/demo/open");
    let (status, _) = call(&app, get_as("/api/demo/admin", Some("admin"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    remove_dist(&dir);
}

#[tokio::test]
async fn route_tag_outside_manifest_fails_start() {
    let ctx = lazy_ctx();
    let entries = [PluginEntry {
        manifest: BAD_MANIFEST,
        build: build_bad,
    }];
    let plugins = server::build_plugins(&ctx, &entries).unwrap();
    let err = server::plugin_routes(&ctx, &plugins, &TAGS).unwrap_err();
    assert!(err.to_string().contains("plugin bad"), "{err}");
}

#[tokio::test]
async fn spa_fallback_injects_page_meta() {
    let dir = dist("meta");
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("assets/app.js"), "console.log(1)").unwrap();
    let (app, _) = demo_app(&TAGS, dir.clone());

    let (status, body) = call(&app, get_as("/demo?x=%3Cb%3E", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("<title>Demo &lt;x=%3Cb%3E&gt;</title>"),
        "{body}"
    );
    assert!(
        body.contains("<meta property=\"og:title\" content=\"Demo &amp; co\">"),
        "{body}"
    );

    let (status, body) = call(&app, get_as("/other/path", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<title>KOL Arena</title>") && !body.contains("og:title"));

    let (status, body) = call(&app, get_as("/assets/app.js", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "console.log(1)");

    let (status, _) = call(&app, get_as("/api/demo/nope", None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    std::fs::remove_file(dir.join("assets/app.js")).unwrap();
    std::fs::remove_dir(dir.join("assets")).unwrap();
    remove_dist(&dir);
}
