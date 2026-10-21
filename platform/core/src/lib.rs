//! `arena-platform`: what every KOL Arena plugin builds on.
//!
//! - [`Person`], [`Role`] and route [`Tag`]s.
//! - [`Plugin`] trait, [`Route`], [`PageMeta`], [`Manifest`] and [`assemble`] (construction order).
//! - [`Ctx`] with [`Apis`] (type map of plugin APIs), [`Events`], [`Slots`] and [`Config`].
//! - Implementation [`Registry`]: picks an implementation or `off` (the stub) per call.
//! - Per-plugin schema migrations ([`migrate`]), [`scheduler`] with advisory locks.
//! - [`testing::TestDb`]: a throwaway test database.
//!
//! The platform names no business block.

pub mod apis;
pub mod config;
pub mod ctx;
pub mod events;
pub mod manifest;
pub mod migrate;
pub mod page;
pub mod person;
pub mod plugin;
pub mod registry;
pub mod scheduler;
pub mod slots;
pub mod testing;

pub use apis::{Api, Apis};
pub use config::{Config, NotConfigured, Setting};
pub use ctx::Ctx;
pub use events::{EmitError, Event, Events};
pub use manifest::{Manifest, Screen};
pub use page::{MetaAttr, MetaTag, PageMeta};
pub use person::{Person, Role, Tag};
pub use plugin::{Built, Plugin, PluginEntry, Route, StartError, assemble, construction_order};
pub use registry::{Impl, Registry, Rule};
pub use scheduler::{DataLock, Job, LockSpec, RunError, Scheduler};
pub use slots::{NobodyPerson, PersonOfRequest, Slot, Slots};

/// Boxed error for job, install and event handler results.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Boxed `Send` future.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
