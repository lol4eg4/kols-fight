//! Implementation registry: picks a plugin's implementation, or `off` (its stub), per call.
//!
//! Rules come from the app config (`ARENA_IMPL_RULES`, JSON list). A rule matches by plugin
//! and optionally by app, role and person id. The most specific match wins
//! (person id, then role, then app); among equals the later rule wins. No match: `default`.

use crate::person::{Person, Role};
use serde::Deserialize;

/// Name of the implementation every plugin registers first.
pub const DEFAULT_IMPL: &str = "default";
/// Rule value that selects the stub.
pub const OFF: &str = "off";

/// One rule, e.g. `{"plugin":"duel","role":"nobody","use":"off"}`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Plugin name.
    pub plugin: String,
    /// App the rule is for; `None` = every app.
    #[serde(default)]
    pub app: Option<String>,
    /// Role the rule is for; `None` = every role.
    #[serde(default)]
    pub role: Option<Role>,
    /// Person id the rule is for; `None` = every person.
    #[serde(default)]
    pub person: Option<i64>,
    /// Implementation name, or `off`.
    #[serde(rename = "use")]
    pub choice: String,
}

/// What [`Registry::choose`] picked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Impl {
    /// A registered implementation by name (`default` when no rule matches).
    Named(String),
    /// The stub: the plugin answers «off».
    Off,
}

/// Rules for one app.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    app: String,
    rules: Vec<Rule>,
}

impl Registry {
    /// Keeps the rules for `app` and the rules with no app.
    pub fn new(app: &str, rules: &[Rule]) -> Self {
        let rules = rules
            .iter()
            .filter(|r| r.app.as_deref().is_none_or(|a| a == app))
            .cloned()
            .collect();
        Registry { app: app.to_string(), rules }
    }

    pub fn app(&self) -> &str {
        &self.app
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Implementation of `plugin` for a call by `person`.
    pub fn choose(&self, plugin: &str, person: &Person) -> Impl {
        let mut best: Option<(u8, &Rule)> = None;
        for rule in &self.rules {
            if rule.plugin != plugin
                || rule.role.is_some_and(|r| r != person.role)
                || rule.person.is_some_and(|id| Some(id) != person.id)
            {
                continue;
            }
            let rank = 4 * u8::from(rule.person.is_some())
                + 2 * u8::from(rule.role.is_some())
                + u8::from(rule.app.is_some());
            if best.is_none_or(|(b, _)| rank >= b) {
                best = Some((rank, rule));
            }
        }
        match best {
            Some((_, r)) if r.choice == OFF => Impl::Off,
            Some((_, r)) => Impl::Named(r.choice.clone()),
            None => Impl::Named(DEFAULT_IMPL.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(plugin: &str, app: Option<&str>, role: Option<Role>, person: Option<i64>, choice: &str) -> Rule {
        Rule {
            plugin: plugin.into(),
            app: app.map(Into::into),
            role,
            person,
            choice: choice.into(),
        }
    }

    #[test]
    fn no_rule_is_default() {
        let reg = Registry::new("kol-arena", &[]);
        assert_eq!(reg.choose("fight", &Person::NOBODY), Impl::Named("default".into()));
    }

    #[test]
    fn specific_rules_win_and_off_selects_stub() {
        let rules = [
            rule("duel", None, None, None, "off"),
            rule("duel", None, Some(Role::Staff), None, "default"),
            rule("duel", None, None, Some(7), "v2"),
            rule("duel", Some("other-app"), None, None, "v3"),
            rule("fight", Some("kol-arena"), Some(Role::Nobody), None, "off"),
        ];
        let reg = Registry::new("kol-arena", &rules);
        assert_eq!(reg.rules().len(), 4);
        assert_eq!(reg.choose("duel", &Person::NOBODY), Impl::Off);
        assert_eq!(reg.choose("duel", &Person::staff(3)), Impl::Named("default".into()));
        assert_eq!(reg.choose("duel", &Person::staff(7)), Impl::Named("v2".into()));
        assert_eq!(reg.choose("fight", &Person::NOBODY), Impl::Off);
        assert_eq!(reg.choose("fight", &Person::admin(1)), Impl::Named("default".into()));
    }

    #[test]
    fn later_rule_wins_among_equals() {
        let rules = [
            rule("radar", None, None, None, "off"),
            rule("radar", None, None, None, "default"),
        ];
        let reg = Registry::new("kol-arena", &rules);
        assert_eq!(reg.choose("radar", &Person::NOBODY), Impl::Named("default".into()));
    }
}
