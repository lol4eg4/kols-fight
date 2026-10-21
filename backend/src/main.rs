use arena_platform::{Config, Ctx, Scheduler};
use server::{APP, AppState, PLUGINS, TAGS};
use sqlx::postgres::PgPoolOptions;
use std::{path::PathBuf, process::exit, sync::Arc};

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        eprintln!("server: env {name} is required");
        exit(1)
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let port: u16 = required("PORT")
        .parse()
        .map_err(|_| "env PORT must be a port number")?;
    let database_url = required("DATABASE_URL");
    let dist = PathBuf::from(required("WEB_DIST"));
    let config = Config::from_env()?;
    tracing::info!(?config, "config");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    let ctx = Ctx::new(pool.clone(), config, APP)?;
    let plugins = server::build_plugins(&ctx, &PLUGINS)?;
    server::migrate(&pool, &plugins).await?;
    server::install(&ctx, &plugins).await?;
    let (routes, mounted) = server::plugin_routes(&ctx, &plugins, &TAGS)?;
    for m in &mounted {
        tracing::info!(plugin = %m.plugin, tag = m.tag.as_str(), path = %m.path, "route");
    }
    let jobs = server::jobs(&ctx, &plugins)?;
    let _scheduler = Scheduler::start(pool.clone(), jobs);
    let commit = std::env::var("COMMIT").ok().filter(|c| !c.is_empty());
    let state = AppState {
        pool,
        commit,
        plugins: Arc::new(plugins),
        dist,
    };
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    tracing::info!(port, plugins = PLUGINS.len(), "listening");
    axum::serve(listener, server::app(state, routes)).await?;
    Ok(())
}
