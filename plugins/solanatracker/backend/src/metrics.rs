//! Solana Tracker answers -> snapshot metrics. Pure.
//!
//! ```text
//! summary  ──> unreal, total, avg_pos (average buy), last_min_at_fetch, wr_all, wins, losses
//! history  ──> d7  = Σ daily realized over the last 7 days   (today included)
//!              d30 = Σ daily realized over the last 30 days
//!              vol30, days30 (days with a buy or a sell), traded_24h (today's entry)
//! list     ──> pnl_rank (place in the list), in_top_today
//! ```
//!
//! KE-only fields (`d1`, `wr7`, `wr30`, `hold_s`) stay `None`. A history that is not
//! `complete` gives no history-based values.

use crate::models::{HistoryResp, WalletResp};
use api_snapshots::Metrics;
use api_solanatracker::StFailure;
use chrono::{DateTime, Duration, NaiveDate, Utc};

pub fn build(
    wallet: &WalletResp,
    history: &HistoryResp,
    now: DateTime<Utc>,
    pnl_rank: Option<i32>,
    in_top_today: bool,
) -> Result<Metrics, StFailure> {
    let summary = wallet
        .summary
        .as_ref()
        .ok_or_else(|| StFailure::BadResponse {
            message: "wallet answer has no summary".into(),
        })?;
    let mut m = Metrics {
        unreal: summary.pnl.as_ref().and_then(|p| p.unrealized),
        total: summary.pnl.as_ref().and_then(|p| p.total),
        avg_pos: summary.averages.as_ref().and_then(|a| a.buy),
        last_min_at_fetch: summary
            .timing
            .as_ref()
            .and_then(|t| t.last_trade)
            .map(|ms| ((now.timestamp_millis() - ms) / 60_000).max(0)),
        pnl_rank,
        in_top_today,
        ..Default::default()
    };
    if let Some(a) = &wallet.analysis
        && let Some(t) = &a.tokens
    {
        m.wins = t.winning;
        m.losses = t.losing;
        let closed = t.winning.unwrap_or(0) + t.losing.unwrap_or(0);
        if closed > 0 {
            m.wr_all = a.win_rate.map(|p| p / 100.0);
        }
    }

    if history.complete == Some(false) {
        return Ok(m);
    }
    let today = now.date_naive();
    let (mut d7, mut d30, mut vol30) = (0.0, 0.0, 0.0);
    let mut days30 = 0;
    let mut traded_today = false;
    for day in &history.days {
        let date = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d").map_err(|_| {
            StFailure::BadResponse {
                message: format!("history date {:?}", day.date),
            }
        })?;
        if date > today || date <= today - Duration::days(30) {
            continue;
        }
        let a = day.activity.as_ref();
        let realized = a
            .and_then(|a| a.pnl.as_ref())
            .and_then(|p| p.realized)
            .unwrap_or(0.0);
        let trades = a
            .and_then(|a| a.counts.as_ref())
            .map(|c| c.buys.unwrap_or(0.0) + c.sells.unwrap_or(0.0))
            .unwrap_or(0.0);
        d30 += realized;
        vol30 += a
            .and_then(|a| a.volume.as_ref())
            .and_then(|v| v.total)
            .unwrap_or(0.0);
        if trades > 0.0 {
            days30 += 1;
        }
        if date > today - Duration::days(7) {
            d7 += realized;
        }
        if date == today {
            traded_today = trades > 0.0;
        }
    }
    m.d7 = Some(d7);
    m.d30 = Some(d30);
    m.vol30 = Some(vol30);
    m.days30 = Some(days30);
    m.traded_24h = Some(traded_today);
    Ok(m)
}
