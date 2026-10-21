//! The parts of Solana Tracker's JSON we read. Everything is optional: a field the API
//! leaves out is "no data", never zero.

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub name: Option<String>,
    pub twitter: Option<String>,
    pub platform_profiles: Option<PlatformProfiles>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PlatformProfiles {
    pub fomo: Option<FomoProfile>,
}

#[derive(Debug, Default, Deserialize)]
pub struct FomoProfile {
    pub handle: Option<String>,
}

impl Identity {
    /// `identity.twitter` without `@`; empty -> `None`.
    pub fn x_handle(&self) -> Option<String> {
        let h = self.twitter.as_deref()?.trim().trim_start_matches('@');
        (!h.is_empty()).then(|| h.to_string())
    }

    pub fn fomo_handle(&self) -> Option<String> {
        let h = self
            .platform_profiles
            .as_ref()?
            .fomo
            .as_ref()?
            .handle
            .as_deref()?
            .trim();
        (!h.is_empty()).then(|| h.to_string())
    }
}

/// `GET /v2/pnl/leaderboard/kols/period`
#[derive(Debug, Deserialize)]
pub struct KolListResp {
    pub traders: Vec<Trader>,
}

#[derive(Debug, Deserialize)]
pub struct Trader {
    pub wallet: String,
    pub identity: Option<Identity>,
}

/// `GET /v2/pnl/wallets/{wallet}`. A wallet Solana Tracker has not indexed answers
/// `{"indexed":false,"queued":true,...}` with HTTP 200 and no `summary`.
#[derive(Debug, Deserialize)]
pub struct WalletResp {
    pub indexed: Option<bool>,
    pub identity: Option<Identity>,
    pub summary: Option<Summary>,
    pub analysis: Option<Analysis>,
}

#[derive(Debug, Deserialize)]
pub struct Summary {
    pub pnl: Option<Pnl>,
    pub counts: Option<Counts>,
    pub averages: Option<Averages>,
    pub timing: Option<Timing>,
}

#[derive(Debug, Deserialize)]
pub struct Pnl {
    pub realized: Option<f64>,
    pub unrealized: Option<f64>,
    pub total: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Counts {
    pub buys: Option<f64>,
    pub sells: Option<f64>,
    pub trades: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Averages {
    pub buy: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    /// Unix milliseconds.
    pub last_trade: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    /// Percent, all time.
    pub win_rate: Option<f64>,
    pub tokens: Option<Tokens>,
}

#[derive(Debug, Deserialize)]
pub struct Tokens {
    pub winning: Option<i32>,
    pub losing: Option<i32>,
}

/// `GET /v2/pnl/wallets/{wallet}/history?period=30d`
#[derive(Debug, Deserialize)]
pub struct HistoryResp {
    /// `false`: the history is still being built and cannot be summed.
    pub complete: Option<bool>,
    #[serde(default)]
    pub days: Vec<Day>,
}

#[derive(Debug, Deserialize)]
pub struct Day {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub activity: Option<Activity>,
}

#[derive(Debug, Deserialize)]
pub struct Activity {
    pub pnl: Option<ActivityPnl>,
    pub counts: Option<ActivityCounts>,
    pub volume: Option<Volume>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityPnl {
    pub realized: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ActivityCounts {
    pub buys: Option<f64>,
    pub sells: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Volume {
    pub total: Option<f64>,
}
