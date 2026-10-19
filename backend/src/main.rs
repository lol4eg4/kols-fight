use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{any, get},
};
use serde_json::{Value, json};
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};
use std::{path::PathBuf, process::exit};
use tower_http::services::{ServeDir, ServeFile};

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        eprintln!("server: env {name} is required");
        exit(1)
    })
}

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    commit: Option<String>,
}

async fn migration_state(pool: &PgPool) -> Result<Value, sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success")
            .fetch_all(pool)
            .await?;
    let pending = MIGRATOR
        .migrations
        .iter()
        .filter(|m| !applied.contains(&m.version))
        .count();
    Ok(json!({"latest": applied.iter().max(), "pending": pending}))
}

async fn health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match migration_state(&state.pool).await {
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

fn app(state: AppState, dist: PathBuf) -> Router {
    let spa = ServeDir::new(&dist).fallback(ServeFile::new(dist.join("index.html")));
    Router::new()
        .route("/api/health", get(health))
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*rest}", any(api_not_found))
        .fallback_service(spa)
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = required("PORT")
        .parse()
        .map_err(|_| "env PORT must be a port number")?;
    let database_url = required("DATABASE_URL");
    let dist = PathBuf::from(required("WEB_DIST"));
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    MIGRATOR.run(&pool).await?;
    let commit = std::env::var("COMMIT").ok().filter(|c| !c.is_empty());
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    axum::serve(listener, app(AppState { pool, commit }, dist)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use std::time::Duration;
    use tower::ServiceExt;

    fn test_app(dist: PathBuf) -> Router {
        // Lazy pool to a closed port: no database needed.
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(300))
            .connect_lazy("postgres://x@127.0.0.1:1/x")
            .unwrap();
        let commit = Some("abc123".to_string());
        app(AppState { pool, commit }, dist)
    }

    #[tokio::test]
    async fn api_404_and_spa_fallback() {
        let dist = std::env::temp_dir().join("server-test-dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.html"), "<html></html>").unwrap();
        for (path, status) in [
            ("/api", 404),
            ("/api/", 404),
            ("/api/nope", 404),
            ("/api/health/x", 404),
            ("/some/spa/path", 200),
        ] {
            let req = Request::get(path).body(Body::empty()).unwrap();
            let res = test_app(dist.clone()).oneshot(req).await.unwrap();
            assert_eq!(res.status().as_u16(), status, "{path}");
        }
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
}
