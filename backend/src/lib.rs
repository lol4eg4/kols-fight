//! Server assembly: plugin list, migrations, apps by tag, jobs, SPA fallback with page meta,
//! health. Business code lives in `plugins/*/backend`; this crate only puts them together.

use arena_platform::{
    BoxError, Built, Ctx, Job, Person, PersonOfRequest, PluginEntry, StartError, Tag, assemble,
    migrate::{migrate_plugin, pending_migrations},
    page,
};
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, Uri},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{any, get},
};
use serde_json::{Map, Value, json};
use sqlx::{PgPool, migrate::Migrator};
use std::{collections::HashSet, path::PathBuf, sync::Arc};
use tower_http::services::ServeDir;

/// App name for implementation rules (`ARENA_IMPL_RULES`).
pub const APP: &str = "kol-arena";

/// Tags this server serves. The client page [public] and the team section
/// [public, staff, admin] run on one server and one address.
pub const TAGS: [Tag; 3] = Tag::ALL;

/// Every plugin. [`assemble`] builds them by layer, then by `uses` inside a layer.
pub const PLUGINS: [PluginEntry; 21] = [
    plugin_auth::ENTRY,
    plugin_journal::ENTRY,
    plugin_registry::ENTRY,
    plugin_snapshots::ENTRY,
    plugin_score::ENTRY,
    plugin_board::ENTRY,
    plugin_fight::ENTRY,
    plugin_kolexplorer::ENTRY,
    plugin_solanatracker::ENTRY,
    plugin_schedule::ENTRY,
    plugin_radar::ENTRY,
    plugin_roster::ENTRY,
    plugin_wallets::ENTRY,
    plugin_fomo::ENTRY,
    plugin_network::ENTRY,
    plugin_picker::ENTRY,
    plugin_moves::ENTRY,
    plugin_duel::ENTRY,
    plugin_guess::ENTRY,
    plugin_tournament::ENTRY,
    plugin_streaks::ENTRY,
];

/// App migrations in schema `public` (kept for `/api/health`).
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Builds every plugin of `entries` in construction order.
pub fn build_plugins(ctx: &Ctx, entries: &[PluginEntry]) -> Result<Vec<Built>, StartError> {
    assemble(ctx, entries)
}

/// Runs the app migrations, then each plugin's migrations in its own schema, in order.
pub async fn migrate(pool: &PgPool, plugins: &[Built]) -> Result<(), BoxError> {
    MIGRATOR.run(pool).await?;
    for b in plugins {
        migrate_plugin(pool, b.plugin.name(), b.plugin.migrations()).await?;
    }
    Ok(())
}

/// Runs every plugin's `install` in construction order.
pub async fn install(ctx: &Ctx, plugins: &[Built]) -> Result<(), BoxError> {
    for b in plugins {
        b.plugin
            .install(ctx)
            .await
            .map_err(|e| format!("plugin {}: install: {e}", b.manifest.name))?;
    }
    Ok(())
}

/// Every job of every plugin. Job names are unique.
pub fn jobs(ctx: &Ctx, plugins: &[Built]) -> Result<Vec<Job>, StartError> {
    let mut seen = HashSet::new();
    let mut all = Vec::new();
    for b in plugins {
        for job in b.plugin.jobs(ctx) {
            if !seen.insert(job.name) {
                return Err(StartError::Build {
                    plugin: b.manifest.name.clone(),
                    message: format!("job {} is defined twice", job.name),
                });
            }
            all.push(job);
        }
    }
    Ok(all)
}

/// One mounted route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mounted {
    pub plugin: String,
    pub tag: Tag,
    pub path: String,
}

/// Plugin routes with the tags in `tags`, each behind its tag guard. A route whose tag is not
/// in its plugin's `plugin.toml` `tags` is a start-up error.
pub fn plugin_routes(
    ctx: &Ctx,
    plugins: &[Built],
    tags: &[Tag],
) -> Result<(Router, Vec<Mounted>), StartError> {
    let mut router = Router::new();
    let mut mounted = Vec::new();
    for b in plugins {
        for route in b.plugin.routes(ctx) {
            if !b.manifest.tags.contains(&route.tag) {
                return Err(StartError::Build {
                    plugin: b.manifest.name.clone(),
                    message: format!(
                        "route {} has tag {}, not in plugin.toml tags",
                        route.path,
                        route.tag.as_str()
                    ),
                });
            }
            if !tags.contains(&route.tag) {
                continue;
            }
            let guard = middleware::from_fn_with_state((ctx.clone(), route.tag), tag_guard);
            router = router.route(&route.path, route.handler.layer(guard));
            mounted.push(Mounted {
                plugin: b.manifest.name.clone(),
                tag: route.tag,
                path: route.path,
            });
        }
    }
    Ok((router, mounted))
}

/// Finds the person of the request (platform slot, filled by auth) and checks the route tag.
/// Nobody on a closed route gets 401; a staff member on an admin route gets 403.
async fn tag_guard(State((ctx, tag)): State<(Ctx, Tag)>, mut req: Request, next: Next) -> Response {
    let person = match ctx.slots.get::<dyn PersonOfRequest>() {
        Some(slot) => slot.get().person(req.headers()).await,
        None => Person::NOBODY,
    };
    if !tag.admits(&person) {
        let (status, error) = if person.is_staff() {
            (StatusCode::FORBIDDEN, "forbidden")
        } else {
            (StatusCode::UNAUTHORIZED, "sign in required")
        };
        return (status, Json(json!({ "error": error }))).into_response();
    }
    req.extensions_mut().insert(person);
    next.run(req).await
}

/// What the HTTP layer needs.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub commit: Option<String>,
    pub plugins: Arc<Vec<Built>>,
    pub dist: PathBuf,
}

async fn migration_state(state: &AppState) -> Result<Value, sqlx::Error> {
    let pool = &state.pool;
    sqlx::query("SELECT 1").execute(pool).await?;
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM public._sqlx_migrations WHERE success")
            .fetch_all(pool)
            .await?;
    let pending = MIGRATOR
        .migrations
        .iter()
        .filter(|m| !applied.contains(&m.version))
        .count();
    let mut plugins = Map::new();
    for b in state.plugins.iter() {
        let name = b.plugin.name();
        let n = pending_migrations(pool, name, b.plugin.migrations()).await?;
        plugins.insert(name.to_string(), json!({ "pending": n }));
    }
    Ok(json!({"latest": applied.iter().max(), "pending": pending, "plugins": plugins}))
}

async fn health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match migration_state(&state).await {
        Ok(migration) => (
            StatusCode::OK,
            Json(json!({"ok": true, "db": true, "commit": state.commit, "migration": migration})),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"ok": false, "db": false, "commit": state.commit})),
        ),
    }
}

async fn api_not_found() -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({"error": "not found"})))
}

/// `https://host` the visitor used: `X-Forwarded-Proto`/`X-Forwarded-Host` from the proxy,
/// else `Host`. None when the host has characters a host name cannot have.
fn request_origin(headers: &HeaderMap) -> Option<String> {
    let first = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(',').next().unwrap_or("").trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let host = first("x-forwarded-host").or_else(|| first("host"))?;
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
    {
        return None;
    }
    let proto = match first("x-forwarded-proto").as_deref() {
        Some("https") => "https",
        _ => "http",
    };
    Some(format!("{proto}://{host}"))
}

/// SPA fallback: `index.html` with the first plugin page meta for this path. Meta values
/// that are site paths (`/api/fight/image?...`) become absolute URLs: X needs them so.
async fn spa_index(State(state): State<AppState>, uri: Uri, headers: HeaderMap) -> Response {
    let html = match tokio::fs::read_to_string(state.dist.join("index.html")).await {
        Ok(html) => html,
        Err(_) => return (StatusCode::NOT_FOUND, "index.html not found").into_response(),
    };
    for b in state.plugins.iter() {
        if let Some(mut meta) = b.plugin.page_meta(uri.path(), uri.query()).await {
            if let Some(origin) = request_origin(&headers) {
                for t in &mut meta.tags {
                    if t.content.starts_with('/') && !t.content.starts_with("//") {
                        t.content = format!("{origin}{}", t.content);
                    }
                }
            }
            return Html(page::inject(&html, &meta)).into_response();
        }
    }
    Html(html).into_response()
}

/// The whole HTTP app: health, plugin routes, `/api` 404s, static files and the SPA fallback.
pub fn app(state: AppState, plugin_routes: Router) -> Router {
    let spa = get(spa_index).with_state(state.clone());
    let files = ServeDir::new(&state.dist)
        .append_index_html_on_directories(false)
        .fallback(spa.clone());
    Router::new()
        .route("/api/health", get(health))
        .with_state(state)
        .merge(plugin_routes)
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found))
        .route("/", spa.clone())
        .route("/index.html", spa)
        .fallback_service(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_platform::Config;
    use axum::body::Body;
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;
    use tower::ServiceExt;

    fn lazy_pool() -> PgPool {
        // Lazy pool to a closed port: no database needed.
        PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(300))
            .connect_lazy("postgres://x@127.0.0.1:1/x")
            .unwrap()
    }

    fn test_app(dist: PathBuf) -> Router {
        let pool = lazy_pool();
        let ctx = Ctx::new(pool.clone(), Config::default(), APP).unwrap();
        let plugins = build_plugins(&ctx, &PLUGINS).unwrap();
        let (routes, _) = plugin_routes(&ctx, &plugins, &TAGS).unwrap();
        let state = AppState {
            pool,
            commit: Some("abc123".to_string()),
            plugins: Arc::new(plugins),
            dist,
        };
        app(state, routes)
    }

    #[tokio::test]
    async fn api_404_and_spa_fallback() {
        let dist = std::env::temp_dir().join(format!("server-test-dist-{}", std::process::id()));
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "<html></html>").unwrap();
        for (path, status) in [
            ("/api", 404),
            ("/api/", 404),
            ("/api/nope", 404),
            ("/api/health/x", 404),
            ("/some/spa/path", 200),
            ("/", 200),
        ] {
            let req = Request::get(path).body(Body::empty()).unwrap();
            let res = test_app(dist.clone()).oneshot(req).await.unwrap();
            assert_eq!(res.status().as_u16(), status, "{path}");
        }
        std::fs::remove_file(dist.join("index.html")).unwrap();
        std::fs::remove_dir(&dist).unwrap();
    }

    #[tokio::test]
    async fn health_is_503_without_database() {
        let req = Request::get("/api/health").body(Body::empty()).unwrap();
        let res = test_app("dist".into()).oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!({"ok": false, "db": false, "commit": "abc123"}));
    }

    #[test]
    fn every_plugin_builds_in_layer_order() {
        let ctx = Ctx::new(lazy_pool_sync(), Config::default(), APP).unwrap();
        let plugins = build_plugins(&ctx, &PLUGINS).unwrap();
        let names: Vec<&str> = plugins.iter().map(|b| b.manifest.name.as_str()).collect();
        assert_eq!(names.len(), 21);
        let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
        assert!(pos("journal") < pos("auth"));
        assert!(pos("snapshots") < pos("score") && pos("score") < pos("board"));
        assert!(pos("roster") < pos("wallets") && pos("wallets") < pos("fomo"));
        let layers: Vec<u8> = plugins.iter().map(|b| b.manifest.layer).collect();
        assert!(layers.windows(2).all(|w| w[0] <= w[1]), "{names:?}");
    }

    fn lazy_pool_sync() -> PgPool {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = rt.enter();
        lazy_pool()
    }
}
