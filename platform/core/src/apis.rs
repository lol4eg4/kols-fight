//! Type map of plugin APIs: `dyn Trait` -> implementations plus stub.
//!
//! A plugin registers its API with [`Apis::provide`] while it is built; plugins that use it
//! take an [`Api`] handle with [`Apis::require`]. [`Api::pick`] asks the [`Registry`] which
//! implementation serves this person, so a plugin can be `off` for an app, role or person.

use crate::{
    plugin::StartError,
    person::Person,
    registry::{DEFAULT_IMPL, Impl, OFF, Registry},
};
use std::{
    any::{Any, TypeId, type_name},
    collections::HashMap,
    sync::{Arc, RwLock},
};

struct Entry<T: ?Sized> {
    plugin: &'static str,
    impls: Vec<(&'static str, Arc<T>)>,
    stub: Arc<T>,
}

type AnyEntry = Box<dyn Any + Send + Sync>;

/// All registered APIs. Cheap to clone; clones share the map.
#[derive(Clone)]
pub struct Apis {
    map: Arc<RwLock<HashMap<TypeId, AnyEntry>>>,
    /// plugin name -> implementation names, in registration order.
    impls: Arc<RwLock<HashMap<&'static str, Vec<&'static str>>>>,
    registry: Arc<Registry>,
}

/// Handle to one API. [`Api::pick`] returns the implementation for a call.
pub struct Api<T: ?Sized> {
    entry: Arc<Entry<T>>,
    registry: Arc<Registry>,
}

impl<T: ?Sized> Clone for Api<T> {
    fn clone(&self) -> Self {
        Api {
            entry: self.entry.clone(),
            registry: self.registry.clone(),
        }
    }
}

impl<T: ?Sized + Send + Sync + 'static> Api<T> {
    /// Plugin that provides this API.
    pub fn plugin(&self) -> &'static str {
        self.entry.plugin
    }

    /// Implementation for a call by `person`: the registry's choice, or the stub for `off`.
    pub fn pick(&self, person: &Person) -> Arc<T> {
        match self.registry.choose(self.entry.plugin, person) {
            Impl::Off => self.entry.stub.clone(),
            Impl::Named(name) => self
                .entry
                .impls
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or(&self.entry.impls[0])
                .1
                .clone(),
        }
    }
}

impl Apis {
    pub fn new(registry: Registry) -> Self {
        Apis {
            map: Arc::default(),
            impls: Arc::default(),
            registry: Arc::new(registry),
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Registers `plugin`'s API `T` (a `dyn Trait`). `impls` is non-empty; the first is
    /// the default and should be named `default`. `stub` answers «off».
    pub fn provide<T: ?Sized + Send + Sync + 'static>(
        &self,
        plugin: &'static str,
        impls: Vec<(&'static str, Arc<T>)>,
        stub: Arc<T>,
    ) -> Result<(), StartError> {
        let api = type_name::<T>();
        if impls.is_empty() {
            return Err(StartError::Api {
                plugin: plugin.into(),
                message: format!("provides {api} with no implementation"),
            });
        }
        let mut map = self.map.write().expect("apis lock");
        if let Some(existing) = map.get(&TypeId::of::<T>()) {
            let first = existing
                .downcast_ref::<Arc<Entry<T>>>()
                .map(|e| e.plugin)
                .unwrap_or("?");
            return Err(StartError::Api {
                plugin: plugin.into(),
                message: format!("provides {api}, already provided by {first}"),
            });
        }
        let names: Vec<&'static str> = impls.iter().map(|(n, _)| *n).collect();
        let entry = Arc::new(Entry { plugin, impls, stub });
        map.insert(TypeId::of::<T>(), Box::new(entry));
        self.impls
            .write()
            .expect("apis lock")
            .entry(plugin)
            .or_default()
            .extend(names);
        Ok(())
    }

    /// Handle to API `T`, if some plugin provides it.
    pub fn get<T: ?Sized + Send + Sync + 'static>(&self) -> Option<Api<T>> {
        let map = self.map.read().expect("apis lock");
        let entry = map
            .get(&TypeId::of::<T>())?
            .downcast_ref::<Arc<Entry<T>>>()?
            .clone();
        Some(Api {
            entry,
            registry: self.registry.clone(),
        })
    }

    /// Handle to API `T` for plugin `by`; a start-up error naming `by` when nobody provides it.
    pub fn require<T: ?Sized + Send + Sync + 'static>(
        &self,
        by: &str,
    ) -> Result<Api<T>, StartError> {
        self.get::<T>().ok_or_else(|| StartError::MissingApi {
            plugin: by.into(),
            api: type_name::<T>().into(),
        })
    }

    /// Checks every registry rule against the built plugins: the plugin exists, and the
    /// value is `off` or an implementation that plugin registered (`default` for plugins
    /// without an API).
    pub fn check_rules(&self, plugins: &[&str]) -> Result<(), StartError> {
        let impls = self.impls.read().expect("apis lock");
        for rule in self.registry.rules() {
            if !plugins.contains(&rule.plugin.as_str()) {
                return Err(StartError::Rule(format!(
                    "rule for unknown plugin {}",
                    rule.plugin
                )));
            }
            let known = impls.get(rule.plugin.as_str());
            let ok = rule.choice == OFF
                || match known {
                    Some(names) => names.contains(&rule.choice.as_str()),
                    None => rule.choice == DEFAULT_IMPL,
                };
            if !ok {
                return Err(StartError::Rule(format!(
                    "rule for {} names unknown implementation {}",
                    rule.plugin, rule.choice
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{person::Role, registry::Rule};

    trait Greet: Send + Sync {
        fn hi(&self) -> &'static str;
    }
    struct Real;
    struct Other;
    struct Stub;
    impl Greet for Real {
        fn hi(&self) -> &'static str {
            "real"
        }
    }
    impl Greet for Other {
        fn hi(&self) -> &'static str {
            "other"
        }
    }
    impl Greet for Stub {
        fn hi(&self) -> &'static str {
            "off"
        }
    }

    fn rule(role: Option<Role>, choice: &str) -> Rule {
        Rule {
            plugin: "greeter".into(),
            app: None,
            role,
            person: None,
            choice: choice.into(),
        }
    }

    #[test]
    fn pick_follows_registry() {
        let rules = [rule(Some(Role::Nobody), "off"), rule(Some(Role::Admin), "other")];
        let apis = Apis::new(Registry::new("app", &rules));
        apis.provide::<dyn Greet>(
            "greeter",
            vec![("default", Arc::new(Real)), ("other", Arc::new(Other))],
            Arc::new(Stub),
        )
        .unwrap();
        let api = apis.require::<dyn Greet>("caller").unwrap();
        assert_eq!(api.plugin(), "greeter");
        assert_eq!(api.pick(&Person::NOBODY).hi(), "off");
        assert_eq!(api.pick(&Person::staff(1)).hi(), "real");
        assert_eq!(api.pick(&Person::admin(1)).hi(), "other");
        apis.check_rules(&["greeter"]).unwrap();
        assert!(apis.check_rules(&["nobody"]).is_err());
    }

    #[test]
    fn missing_and_duplicate_apis_are_errors() {
        let apis = Apis::new(Registry::default());
        let err = apis.require::<dyn Greet>("wallets").err().unwrap();
        assert!(err.to_string().contains("wallets"), "{err}");
        apis.provide::<dyn Greet>("a", vec![("default", Arc::new(Real))], Arc::new(Stub))
            .unwrap();
        let err = apis
            .provide::<dyn Greet>("b", vec![("default", Arc::new(Real))], Arc::new(Stub))
            .unwrap_err();
        assert!(err.to_string().contains("already provided by a"), "{err}");
    }

    #[test]
    fn unknown_impl_in_rule_is_error() {
        let apis = Apis::new(Registry::new("app", &[rule(None, "v9")]));
        apis.provide::<dyn Greet>("greeter", vec![("default", Arc::new(Real))], Arc::new(Stub))
            .unwrap();
        assert!(apis.check_rules(&["greeter"]).is_err());
    }
}
