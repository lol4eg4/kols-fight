//! `plugins/<name>/plugin.toml`: what a plugin is, uses, emits, listens, declares and fills.

use crate::person::Tag;
use serde::Deserialize;

/// Parsed `plugin.toml`. Unknown keys are an error.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Folder, schema and crate suffix: lowercase ASCII letters.
    pub name: String,
    /// 0 common, 1 arena, 2 daily data, 3 network, 4 games.
    pub layer: u8,
    pub version: String,
    /// Route tags the plugin serves.
    pub tags: Vec<Tag>,
    /// Plugins whose API it calls (same or lower layer).
    pub uses: Vec<String>,
    /// Events it emits; page events start with `page:`.
    pub emits: Vec<String>,
    /// Events it listens to; page events start with `page:`.
    pub listens: Vec<String>,
    /// Slots it declares: `screen:<name>` or `api:<name>`.
    pub slots: Vec<String>,
    /// Slots it fills: `<plugin>/<slot>` (`platform/<slot>` for platform slots).
    pub fills: Vec<String>,
    /// Widgets it offers.
    pub widgets: Vec<String>,
    /// Screens it owns.
    pub screens: Vec<Screen>,
}

/// One screen of a plugin.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub name: String,
    pub path: String,
    pub tag: Tag,
}

impl Manifest {
    /// Parses and checks the name.
    pub fn parse(text: &str) -> Result<Self, String> {
        let m: Manifest = toml::from_str(text).map_err(|e| e.to_string())?;
        if m.name.is_empty() || !m.name.bytes().all(|b| b.is_ascii_lowercase()) {
            return Err(format!(
                "plugin name {:?} must be lowercase ASCII letters",
                m.name
            ));
        }
        Ok(m)
    }

    /// Backend (non-page) events it listens to.
    pub fn backend_listens(&self) -> impl Iterator<Item = &str> {
        self.listens
            .iter()
            .map(String::as_str)
            .filter(|e| !e.starts_with("page:"))
    }

    /// Hosts of API slots it fills: `board` for `board/in_network` when board declares
    /// `api:in_network`. Needs every manifest to tell API slots from screen slots.
    pub fn api_slot_hosts<'a>(&'a self, all: &'a [Manifest]) -> Vec<&'a str> {
        self.fills
            .iter()
            .filter_map(|f| f.split_once('/'))
            .filter(|(host, slot)| {
                all.iter()
                    .any(|m| m.name == *host && m.slots.iter().any(|s| *s == format!("api:{slot}")))
            })
            .map(|(host, _)| host)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOTS: &str = r#"
name = "snapshots"
layer = 1
version = "0.1.0"
tags = []
uses = []
emits = []
listens = []
slots = []
fills = []
widgets = []
screens = []
"#;

    #[test]
    fn parses_and_rejects_unknown_keys() {
        let m = Manifest::parse(SNAPSHOTS).unwrap();
        assert_eq!(m.name, "snapshots");
        assert_eq!(m.layer, 1);
        let bad = format!("{SNAPSHOTS}extra = 1\n");
        assert!(Manifest::parse(&bad).is_err());
        let bad_name = SNAPSHOTS.replace("\"snapshots\"", "\"Snap-shots\"");
        assert!(Manifest::parse(&bad_name).is_err());
    }
}
