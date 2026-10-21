//! Server settings read once from env at start.
//!
//! A missing or empty value is "not configured". Code never replaces it with a default:
//! the feature that needs it reports [`NotConfigured`]. `Debug` never prints a value.

use crate::registry::Rule;
use std::{collections::BTreeMap, fmt};

/// Optional settings plugins read through [`Config::get`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Setting {
    /// Solana Tracker Data API key (`x-api-key`).
    SolanaTrackerApiKey,
    /// CSV export URL of the network sheet tab `Influencers`.
    NetworkSheetCsvUrl,
    /// Login of the first admin, created at install when no admin exists.
    FirstAdminLogin,
    /// Password of the first admin.
    FirstAdminPassword,
    /// Solana mainnet public RPC URL for the FOMO match.
    PublicRpcUrl,
}

impl Setting {
    pub const ALL: [Setting; 5] = [
        Setting::SolanaTrackerApiKey,
        Setting::NetworkSheetCsvUrl,
        Setting::FirstAdminLogin,
        Setting::FirstAdminPassword,
        Setting::PublicRpcUrl,
    ];

    /// Env variable name.
    pub fn env(self) -> &'static str {
        match self {
            Setting::SolanaTrackerApiKey => "SOLANA_TRACKER_API_KEY",
            Setting::NetworkSheetCsvUrl => "NETWORK_SHEET_CSV_URL",
            Setting::FirstAdminLogin => "FIRST_ADMIN_LOGIN",
            Setting::FirstAdminPassword => "FIRST_ADMIN_PASSWORD",
            Setting::PublicRpcUrl => "PUBLIC_RPC_URL",
        }
    }
}

/// A setting has no value. Displays as `SOLANA_TRACKER_API_KEY not configured`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{} not configured", .0.env())]
pub struct NotConfigured(pub Setting);

/// Env variable holding implementation rules as JSON (see [`Rule`]). Missing: no rules.
pub const IMPL_RULES_ENV: &str = "ARENA_IMPL_RULES";

/// Settings and implementation rules.
#[derive(Clone, Default)]
pub struct Config {
    values: BTreeMap<Setting, String>,
    rules: Vec<Rule>,
}

/// `ARENA_IMPL_RULES` is not valid JSON for a list of rules.
#[derive(Debug, thiserror::Error)]
#[error("{IMPL_RULES_ENV}: {0}")]
pub struct ConfigError(String);

impl Config {
    /// Reads every [`Setting`] and `ARENA_IMPL_RULES` from the process env.
    pub fn from_env() -> Result<Self, ConfigError> {
        let rules = std::env::var(IMPL_RULES_ENV).ok();
        Self::from_lookup(|name| std::env::var(name).ok(), rules.as_deref())
    }

    /// Builds a config from a lookup function (tests).
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
        rules_json: Option<&str>,
    ) -> Result<Self, ConfigError> {
        let values = Setting::ALL
            .into_iter()
            .filter_map(|s| {
                lookup(s.env())
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| (s, v))
            })
            .collect();
        let rules = match rules_json.map(str::trim).filter(|r| !r.is_empty()) {
            None => Vec::new(),
            Some(json) => serde_json::from_str(json).map_err(|e| ConfigError(e.to_string()))?,
        };
        Ok(Config { values, rules })
    }

    /// The value, or [`NotConfigured`].
    pub fn get(&self, setting: Setting) -> Result<&str, NotConfigured> {
        self.values
            .get(&setting)
            .map(String::as_str)
            .ok_or(NotConfigured(setting))
    }

    pub fn is_configured(&self, setting: Setting) -> bool {
        self.values.contains_key(&setting)
    }

    /// Implementation rules for the [`crate::Registry`].
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Config");
        for s in Setting::ALL {
            let shown = if self.is_configured(s) {
                "<set>"
            } else {
                "<not configured>"
            };
            d.field(s.env(), &shown);
        }
        d.field("impl_rules", &self.rules.len()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_is_not_configured_and_debug_redacts() {
        let cfg = Config::from_lookup(
            |name| match name {
                "SOLANA_TRACKER_API_KEY" => Some("st-secret-123".into()),
                "PUBLIC_RPC_URL" => Some("   ".into()),
                _ => None,
            },
            None,
        )
        .unwrap();
        assert_eq!(cfg.get(Setting::SolanaTrackerApiKey), Ok("st-secret-123"));
        let err = cfg.get(Setting::PublicRpcUrl).unwrap_err();
        assert_eq!(err.to_string(), "PUBLIC_RPC_URL not configured");
        assert!(!cfg.is_configured(Setting::NetworkSheetCsvUrl));
        let debug = format!("{cfg:?}");
        assert!(!debug.contains("st-secret-123"), "{debug}");
        assert!(
            debug.contains("SOLANA_TRACKER_API_KEY: \"<set>\""),
            "{debug}"
        );
        assert!(
            debug.contains("FIRST_ADMIN_PASSWORD: \"<not configured>\""),
            "{debug}"
        );
    }

    #[test]
    fn rules_parse_from_json() {
        let cfg = Config::from_lookup(
            |_| None,
            Some(r#"[{"plugin":"duel","role":"nobody","use":"off"}]"#),
        )
        .unwrap();
        assert_eq!(cfg.rules().len(), 1);
        assert!(Config::from_lookup(|_| None, Some("[{")).is_err());
    }
}
