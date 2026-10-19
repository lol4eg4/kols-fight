//! Backend events: a plugin emits a typed event; listeners run one after another.
//!
//! The event type lives in the emitter's `api` crate; its [`Event::NAME`] equals the name in
//! `plugin.toml` (`emits` / `listens`). [`Events::emit`] runs handlers in registration order.
//! A failing handler is logged and does not stop the rest; the failures go back to the emitter.

use crate::{BoxError, BoxFuture};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    future::Future,
    sync::{Arc, RwLock},
};

/// A backend event payload.
pub trait Event: Clone + Send + Sync + 'static {
    /// Name as written in `plugin.toml`, e.g. `snapshot_opened`.
    const NAME: &'static str;
}

type Handler<E> = Arc<dyn Fn(E) -> BoxFuture<'static, Result<(), BoxError>> + Send + Sync>;
type Handlers<E> = Arc<Vec<(&'static str, Handler<E>)>>;

/// Event bus. Cheap to clone; clones share handlers.
#[derive(Clone, Default)]
pub struct Events {
    map: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

/// One or more handlers failed.
#[derive(Debug, thiserror::Error)]
#[error("event {event}: {}", failures.iter().map(|(p, m)| format!("{p}: {m}")).collect::<Vec<_>>().join("; "))]
pub struct EmitError {
    pub event: &'static str,
    /// (listening plugin, error text) per failed handler, in run order.
    pub failures: Vec<(&'static str, String)>,
}

impl Events {
    /// Registers `listener`'s handler for `E`.
    pub fn on<E, F, Fut>(&self, listener: &'static str, handler: F)
    where
        E: Event,
        F: Fn(E) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), BoxError>> + Send + 'static,
    {
        let handler: Handler<E> = Arc::new(move |e| Box::pin(handler(e)));
        let mut map = self.map.write().expect("events lock");
        let slot = map
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::new(Handlers::<E>::default()));
        let list = slot
            .downcast_mut::<Handlers<E>>()
            .expect("event handler type");
        let mut next = list.as_ref().clone();
        next.push((listener, handler));
        *list = Arc::new(next);
    }

    /// Plugins listening to `E`, in run order.
    pub fn listeners<E: Event>(&self) -> Vec<&'static str> {
        self.handlers::<E>().iter().map(|(p, _)| *p).collect()
    }

    fn handlers<E: Event>(&self) -> Handlers<E> {
        self.map
            .read()
            .expect("events lock")
            .get(&TypeId::of::<E>())
            .and_then(|h| h.downcast_ref::<Handlers<E>>())
            .cloned()
            .unwrap_or_default()
    }

    /// Runs every handler for `event`, one after another, in registration order.
    pub async fn emit<E: Event>(&self, event: &E) -> Result<(), EmitError> {
        let mut failures = Vec::new();
        for (listener, handler) in self.handlers::<E>().iter() {
            if let Err(e) = handler(event.clone()).await {
                tracing::error!(event = E::NAME, listener = *listener, error = %e, "event handler failed");
                failures.push((*listener, e.to_string()));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(EmitError {
                event: E::NAME,
                failures,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct Opened(u32);
    impl Event for Opened {
        const NAME: &'static str = "opened";
    }

    #[tokio::test]
    async fn handlers_run_in_order_and_errors_return() {
        let events = Events::default();
        let log = Arc::new(Mutex::new(Vec::new()));
        for name in ["first", "second", "third"] {
            let log = log.clone();
            events.on(name, move |e: Opened| {
                let log = log.clone();
                async move {
                    // Yield so a concurrent runner would interleave.
                    tokio::task::yield_now().await;
                    log.lock().unwrap().push(format!("{name}:{}", e.0));
                    if name == "second" {
                        Err::<(), BoxError>("boom".into())
                    } else {
                        Ok(())
                    }
                }
            });
        }
        assert_eq!(events.listeners::<Opened>(), ["first", "second", "third"]);
        let err = events.emit(&Opened(7)).await.unwrap_err();
        assert_eq!(err.event, "opened");
        assert_eq!(err.failures, vec![("second", "boom".to_string())]);
        assert_eq!(*log.lock().unwrap(), ["first:7", "second:7", "third:7"]);
    }

    #[tokio::test]
    async fn emit_without_listeners_is_ok() {
        Events::default().emit(&Opened(1)).await.unwrap();
    }
}
