//! What every plugin gets at construction.

use crate::{
    apis::Apis,
    config::Config,
    events::Events,
    plugin::StartError,
    registry::Registry,
    slots::{NobodyPerson, PersonOfRequest, Slots},
};
use sqlx::PgPool;
use std::sync::Arc;

/// Shared context. Cheap to clone; clones share everything.
#[derive(Clone)]
pub struct Ctx {
    /// Shared pool with the default search_path: plugin SQL names its schema.
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub apis: Apis,
    pub events: Events,
    pub slots: Slots,
}

impl Ctx {
    /// Context for `app`; implementation rules come from `config`. Declares the platform
    /// slot `platform/person_of_request` (default: nobody).
    pub fn new(pool: PgPool, config: Config, app: &str) -> Result<Self, StartError> {
        let registry = Registry::new(app, config.rules());
        let ctx = Ctx {
            pool,
            config: Arc::new(config),
            apis: Apis::new(registry),
            events: Events::default(),
            slots: Slots::default(),
        };
        ctx.slots.declare::<dyn PersonOfRequest>(
            "platform",
            "person_of_request",
            Arc::new(NobodyPerson),
        )?;
        Ok(ctx)
    }
}
