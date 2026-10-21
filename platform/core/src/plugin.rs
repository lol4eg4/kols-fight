//! The `Plugin` trait, tagged routes, and construction order.

use crate::{BoxError, ctx::Ctx, manifest::Manifest, page::PageMeta, person::Tag, scheduler::Job};
use async_trait::async_trait;
use axum::routing::MethodRouter;
use sqlx::migrate::Migrator;
use std::{collections::HashMap, sync::Arc};

/// One HTTP route with its audience tag. The server mounts only tags its apps serve and
/// checks the person's role against the tag before the handler runs.
pub struct Route {
    pub tag: Tag,
    /// axum path, e.g. `/api/kol/{id}`.
    pub path: String,
    pub handler: MethodRouter,
}

impl Route {
    pub fn new(tag: Tag, path: impl Into<String>, handler: MethodRouter) -> Self {
        Route {
            tag,
            path: path.into(),
            handler,
        }
    }
}

/// A business block. Built once at start by its [`PluginEntry::build`].
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Same as `name` in `plugin.toml`; also its database schema.
    fn name(&self) -> &'static str;

    /// Its migrations (`sqlx::migrate!("../migrations")`), run in its own schema.
    fn migrations(&self) -> &'static Migrator;

    /// Its routes, each with a tag.
    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        Vec::new()
    }

    /// Runs after every plugin's migrations, in construction order. Idempotent.
    async fn install(&self, _ctx: &Ctx) -> Result<(), BoxError> {
        Ok(())
    }

    /// Its scheduled jobs.
    fn jobs(&self, _ctx: &Ctx) -> Vec<Job> {
        Vec::new()
    }

    /// Meta tags for an SPA path (`/fight`), injected into `index.html`. First match wins.
    async fn page_meta(&self, _path: &str, _query: Option<&str>) -> Option<PageMeta> {
        None
    }
}

/// What the server lists per plugin: its `plugin.toml` text and its constructor.
/// The constructor takes the APIs it uses from `ctx` and registers its own.
#[derive(Clone, Copy)]
pub struct PluginEntry {
    pub manifest: &'static str,
    pub build: fn(&Ctx) -> Result<Arc<dyn Plugin>, StartError>,
}

/// A constructed plugin with its manifest.
#[derive(Clone)]
pub struct Built {
    pub manifest: Manifest,
    pub plugin: Arc<dyn Plugin>,
}

/// Start-up error. Every variant names the plugins involved.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("plugin {plugin}: plugin.toml: {message}")]
    Manifest { plugin: String, message: String },
    #[error("plugin {0} is listed twice")]
    DuplicatePlugin(String),
    #[error("plugin {plugin} uses {uses}, which is not in the plugin list")]
    UnknownUse { plugin: String, uses: String },
    #[error("plugin {plugin} (layer {layer}) uses {uses} from higher layer {uses_layer}")]
    HigherLayer {
        plugin: String,
        layer: u8,
        uses: String,
        uses_layer: u8,
    },
    #[error("plugin cycle: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
    #[error("plugin {plugin} needs API {api}, which no plugin provides")]
    MissingApi { plugin: String, api: String },
    #[error("plugin {plugin}: {message}")]
    Api { plugin: String, message: String },
    #[error("plugin {plugin}: {message}")]
    Slot { plugin: String, message: String },
    #[error("implementation rules: {0}")]
    Rule(String),
    #[error("plugin {plugin}: {message}")]
    Build { plugin: String, message: String },
}

/// Construction order: by layer; inside a layer, a plugin comes after the plugins it uses.
/// Ties keep list order. Returns indexes into `manifests`.
pub fn construction_order(manifests: &[Manifest]) -> Result<Vec<usize>, StartError> {
    let mut index = HashMap::new();
    for (i, m) in manifests.iter().enumerate() {
        if index.insert(m.name.as_str(), i).is_some() {
            return Err(StartError::DuplicatePlugin(m.name.clone()));
        }
    }
    for m in manifests {
        for u in &m.uses {
            let Some(&j) = index.get(u.as_str()) else {
                return Err(StartError::UnknownUse {
                    plugin: m.name.clone(),
                    uses: u.clone(),
                });
            };
            if manifests[j].layer > m.layer {
                return Err(StartError::HigherLayer {
                    plugin: m.name.clone(),
                    layer: m.layer,
                    uses: u.clone(),
                    uses_layer: manifests[j].layer,
                });
            }
        }
    }
    let mut layers: Vec<u8> = manifests.iter().map(|m| m.layer).collect();
    layers.sort_unstable();
    layers.dedup();
    let mut order = Vec::with_capacity(manifests.len());
    for layer in layers {
        let mut left: Vec<usize> = (0..manifests.len())
            .filter(|&i| manifests[i].layer == layer)
            .collect();
        while !left.is_empty() {
            let ready = left.iter().position(|&i| {
                manifests[i].uses.iter().all(|u| {
                    let j = index[u.as_str()];
                    manifests[j].layer < layer || order.contains(&j)
                })
            });
            match ready {
                Some(pos) => order.push(left.remove(pos)),
                None => return Err(StartError::Cycle(find_cycle(manifests, &index, &left))),
            }
        }
    }
    Ok(order)
}

/// A cycle among `left` (all stuck), as names with the first repeated at the end.
fn find_cycle(manifests: &[Manifest], index: &HashMap<&str, usize>, left: &[usize]) -> Vec<String> {
    let mut path = vec![left[0]];
    loop {
        let cur = *path.last().expect("path");
        let next = manifests[cur]
            .uses
            .iter()
            .map(|u| index[u.as_str()])
            .find(|j| left.contains(j))
            .expect("stuck plugin uses another stuck plugin");
        if let Some(start) = path.iter().position(|&p| p == next) {
            let mut names: Vec<String> = path[start..]
                .iter()
                .map(|&i| manifests[i].name.clone())
                .collect();
            names.push(manifests[next].name.clone());
            return names;
        }
        path.push(next);
    }
}

/// Parses every manifest, orders them, and builds each plugin in order.
pub fn assemble(ctx: &Ctx, entries: &[PluginEntry]) -> Result<Vec<Built>, StartError> {
    let manifests = entries
        .iter()
        .map(|e| {
            Manifest::parse(e.manifest).map_err(|message| StartError::Manifest {
                plugin: e
                    .manifest
                    .lines()
                    .find_map(|l| l.strip_prefix("name = "))
                    .unwrap_or("?")
                    .trim_matches('"')
                    .to_string(),
                message,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let order = construction_order(&manifests)?;
    let mut built = Vec::with_capacity(order.len());
    for i in order {
        let manifest = manifests[i].clone();
        let plugin = (entries[i].build)(ctx)?;
        if plugin.name() != manifest.name {
            return Err(StartError::Build {
                plugin: manifest.name,
                message: format!("built plugin calls itself {}", plugin.name()),
            });
        }
        built.push(Built { manifest, plugin });
    }
    let names: Vec<&str> = built.iter().map(|b| b.manifest.name.as_str()).collect();
    ctx.apis.check_rules(&names)?;
    Ok(built)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, layer: u8, uses: &[&str]) -> Manifest {
        Manifest {
            name: name.into(),
            layer,
            version: "0.1.0".into(),
            tags: vec![],
            uses: uses.iter().map(|s| s.to_string()).collect(),
            emits: vec![],
            listens: vec![],
            slots: vec![],
            fills: vec![],
            widgets: vec![],
            screens: vec![],
        }
    }

    fn names(ms: &[Manifest], order: &[usize]) -> Vec<String> {
        order.iter().map(|&i| ms[i].name.clone()).collect()
    }

    #[test]
    fn orders_by_layer_then_uses() {
        let ms = vec![
            m("wallets", 3, &["roster", "registry"]),
            m("auth", 0, &["journal"]),
            m("roster", 3, &["registry"]),
            m("registry", 1, &[]),
            m("journal", 0, &[]),
        ];
        let order = construction_order(&ms).unwrap();
        assert_eq!(
            names(&ms, &order),
            ["journal", "auth", "registry", "roster", "wallets"]
        );
    }

    #[test]
    fn cycle_unknown_and_higher_layer_are_errors() {
        let ms = vec![m("a", 1, &["b"]), m("b", 1, &["c"]), m("c", 1, &["a"])];
        let err = construction_order(&ms).unwrap_err();
        assert_eq!(err.to_string(), "plugin cycle: a -> b -> c -> a");

        let ms = vec![m("a", 1, &["ghost"])];
        let err = construction_order(&ms).unwrap_err();
        assert_eq!(
            err.to_string(),
            "plugin a uses ghost, which is not in the plugin list"
        );

        let ms = vec![m("low", 1, &["high"]), m("high", 2, &[])];
        let err = construction_order(&ms).unwrap_err();
        assert!(
            err.to_string().contains("plugin low (layer 1) uses high"),
            "{err}"
        );

        let ms = vec![m("a", 1, &[]), m("a", 1, &[])];
        assert!(matches!(
            construction_order(&ms),
            Err(StartError::DuplicatePlugin(_))
        ));
    }
}
