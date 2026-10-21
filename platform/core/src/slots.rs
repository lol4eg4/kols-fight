//! API slots: a plugin declares a slot of type `dyn Trait` with a default; a plugin of the
//! same or a higher layer fills it. Callers read the slot at call time, so a filler built
//! after the host still counts.

use crate::{person::Person, plugin::StartError};
use async_trait::async_trait;
use axum::http::HeaderMap;
use std::{
    any::{Any, TypeId, type_name},
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// One slot: its default and at most one filler.
pub struct Slot<T: ?Sized> {
    owner: &'static str,
    name: &'static str,
    default: Arc<T>,
    filler: RwLock<Option<(&'static str, Arc<T>)>>,
}

impl<T: ?Sized> Slot<T> {
    /// The filler's implementation, or the default.
    pub fn get(&self) -> Arc<T> {
        match &*self.filler.read().expect("slot lock") {
            Some((_, imp)) => imp.clone(),
            None => self.default.clone(),
        }
    }

    /// Plugin that filled the slot.
    pub fn filler(&self) -> Option<&'static str> {
        self.filler
            .read()
            .expect("slot lock")
            .as_ref()
            .map(|(p, _)| *p)
    }

    /// `<owner>/<name>`, as in `plugin.toml` `fills`.
    pub fn id(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// All API slots. Cheap to clone; clones share the map.
#[derive(Clone, Default)]
pub struct Slots {
    map: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl Slots {
    /// Declares slot `owner/name` of type `T` (a `dyn Trait`) with its default.
    pub fn declare<T: ?Sized + Send + Sync + 'static>(
        &self,
        owner: &'static str,
        name: &'static str,
        default: Arc<T>,
    ) -> Result<Arc<Slot<T>>, StartError> {
        let mut map = self.map.write().expect("slots lock");
        if map.contains_key(&TypeId::of::<T>()) {
            return Err(StartError::Slot {
                plugin: owner.into(),
                message: format!("slot {} is already declared", type_name::<T>()),
            });
        }
        let slot = Arc::new(Slot {
            owner,
            name,
            default,
            filler: RwLock::new(None),
        });
        map.insert(TypeId::of::<T>(), Box::new(slot.clone()));
        Ok(slot)
    }

    /// The slot of type `T`, if declared.
    pub fn get<T: ?Sized + Send + Sync + 'static>(&self) -> Option<Arc<Slot<T>>> {
        self.map
            .read()
            .expect("slots lock")
            .get(&TypeId::of::<T>())?
            .downcast_ref::<Arc<Slot<T>>>()
            .cloned()
    }

    /// Fills slot `T` with `filler`'s implementation. The slot must be declared and empty.
    pub fn fill<T: ?Sized + Send + Sync + 'static>(
        &self,
        filler: &'static str,
        imp: Arc<T>,
    ) -> Result<(), StartError> {
        let slot = self.get::<T>().ok_or_else(|| StartError::Slot {
            plugin: filler.into(),
            message: format!("fills undeclared slot {}", type_name::<T>()),
        })?;
        let mut current = slot.filler.write().expect("slot lock");
        if let Some((other, _)) = &*current {
            return Err(StartError::Slot {
                plugin: filler.into(),
                message: format!("slot {} is already filled by {other}", slot.id()),
            });
        }
        *current = Some((filler, imp));
        Ok(())
    }
}

/// Platform slot «person of request» (`platform/person_of_request`): who sent a request.
/// Filled by the auth plugin; the default answers nobody, so only public routes work.
#[async_trait]
pub trait PersonOfRequest: Send + Sync {
    async fn person(&self, headers: &HeaderMap) -> Person;
}

/// Default of [`PersonOfRequest`]: nobody.
pub struct NobodyPerson;

#[async_trait]
impl PersonOfRequest for NobodyPerson {
    async fn person(&self, _headers: &HeaderMap) -> Person {
        Person::NOBODY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Member: Send + Sync {
        fn is_member(&self, kol: &str) -> bool;
    }
    struct No;
    impl Member for No {
        fn is_member(&self, _: &str) -> bool {
            false
        }
    }
    struct Yes;
    impl Member for Yes {
        fn is_member(&self, kol: &str) -> bool {
            kol == "decu"
        }
    }

    #[test]
    fn default_until_filled() {
        let slots = Slots::default();
        let slot = slots
            .declare::<dyn Member>("board", "in_network", Arc::new(No))
            .unwrap();
        assert_eq!(slot.id(), "board/in_network");
        assert!(!slot.get().is_member("decu"));
        assert_eq!(slot.filler(), None);
        slots.fill::<dyn Member>("roster", Arc::new(Yes)).unwrap();
        assert!(slot.get().is_member("decu"));
        assert_eq!(slot.filler(), Some("roster"));
        let err = slots
            .fill::<dyn Member>("other", Arc::new(Yes))
            .unwrap_err();
        assert!(
            err.to_string().contains("already filled by roster"),
            "{err}"
        );
    }

    #[test]
    fn undeclared_and_double_declare_fail() {
        let slots = Slots::default();
        assert!(slots.fill::<dyn Member>("roster", Arc::new(Yes)).is_err());
        slots
            .declare::<dyn Member>("board", "in_network", Arc::new(No))
            .unwrap();
        assert!(
            slots
                .declare::<dyn Member>("board", "in_network", Arc::new(No))
                .is_err()
        );
    }

    #[tokio::test]
    async fn person_default_is_nobody() {
        assert_eq!(NobodyPerson.person(&HeaderMap::new()).await, Person::NOBODY);
    }
}
