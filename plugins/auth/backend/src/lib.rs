//! Backend of the `auth` plugin (layer 0): staff, roles, sessions.
//!
//! - Passwords: Argon2id hashes only. Session: 32 random bytes in the `arena_session` cookie
//!   (HttpOnly, Secure, SameSite=Strict, 14 days); only its SHA-256 is stored.
//! - Login rate limit: 5 failures per client address in 15 minutes lock it for 15 minutes.
//! - First admin from `FIRST_ADMIN_LOGIN` / `FIRST_ADMIN_PASSWORD` when no admin exists.
//! - Fills `platform/person_of_request`; implements [`api_auth::AuthApi`].
//! - Routes: `POST /api/auth/login` (public), `POST /api/auth/logout` and `GET /api/auth/me`
//!   (staff), `GET|POST /api/admin/staff` and `POST /api/admin/staff/{id}/revoke` (admin).
//!   Non-GET requests to the ones that change state need a same-site `Origin`.

use api_auth::AuthApi;
use api_journal::JournalApi;
use arena_platform::{
    Api, BoxError, Ctx, PersonOfRequest, Plugin, PluginEntry, Route, Setting, StartError,
};
use async_trait::async_trait;
use sqlx::migrate::Migrator;
use std::sync::Arc;

pub mod http;
pub mod service;

pub use http::{clearing_cookie, origin_allowed, origin_guard, session_cookie};
pub use service::{AuthService, LoginError, StaffError};

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "auth";

/// Migrations from `plugins/auth/migrations`, run in schema `auth`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// APIs this plugin calls (`uses` in plugin.toml), taken from `Ctx` at construction.
struct Uses {
    journal: Api<dyn JournalApi>,
}

pub struct AuthPlugin {
    auth: Arc<AuthService>,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        journal: ctx.apis.require::<dyn JournalApi>(NAME)?,
    };
    let auth = Arc::new(AuthService::new(ctx.pool.clone(), uses.journal));
    ctx.apis.provide::<dyn AuthApi>(
        NAME,
        vec![("default", Arc::new(http::AuthImpl(auth.clone())))],
        Arc::new(api_auth::stub::Stub),
    )?;
    ctx.slots
        .fill::<dyn PersonOfRequest>(NAME, Arc::new(http::SessionPerson(auth.clone())))?;
    Ok(Arc::new(AuthPlugin { auth }))
}

#[async_trait]
impl Plugin for AuthPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        http::routes(self.auth.clone())
    }

    async fn install(&self, ctx: &Ctx) -> Result<(), BoxError> {
        let login = ctx.config.get(Setting::FirstAdminLogin);
        let password = ctx.config.get(Setting::FirstAdminPassword);
        let (Ok(login), Ok(password)) = (login, password) else {
            tracing::warn!(
                "auth: FIRST_ADMIN_LOGIN / FIRST_ADMIN_PASSWORD not configured; no first admin is created"
            );
            return Ok(());
        };
        match self.auth.ensure_first_admin(login, password).await {
            Ok(Some(id)) => tracing::info!(id, "auth: first admin created"),
            Ok(None) => {}
            Err(e) => return Err(format!("first admin: {e:?}").into()),
        }
        Ok(())
    }
}
