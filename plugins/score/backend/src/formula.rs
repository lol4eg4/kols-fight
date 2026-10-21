//! Arena Score: a pure function of one KOL's metrics.
//!
//! ```text
//!  metrics ─┬─ d30 ──────────── Profit    log  $1,000 .. $500,000      × 0.40 ─┐
//!           ├─ wr30 | wr_all ── Accuracy  lin  30% .. 80%  (d30 > 0)  × 0.25 ─┤
//!           ├─ d7 ───────────── Form      log  $500 .. $100,000       × 0.20 ─┼─ Σ × 100 ─ round ─ Score 0..100
//!           └─ last_min ─────── Activity  ≤60 min = 1, log, 30 d = 0  × 0.15 ─┘     │
//!                                                                                   └─ Tier: GOAT ≥70, Diamond ≥50,
//!                                                                                            Sendoor ≥35, Degen
//! ```
//!
//! Each part's share of its scale is clamped to 0..=1. A part without data scores 0 and is
//! marked [`PartStatus::NoData`]. Accuracy uses `wr30`, or the all-time `wr_all` when `wr30`
//! is missing, and counts only when `d30 > 0` ([`PartStatus::NeedsProfit`] otherwise).
//! Changed only with the owner's consent.

use api_score::{Badge, Part, PartKind, PartStatus, ScoreCard, Tier};
use api_snapshots::Metrics;

pub const PROFIT_WEIGHT: f64 = 0.40;
pub const ACCURACY_WEIGHT: f64 = 0.25;
pub const FORM_WEIGHT: f64 = 0.20;
pub const ACTIVITY_WEIGHT: f64 = 0.15;

/// Profit scale: `d30` in USD, log.
pub const PROFIT_SCALE: (f64, f64) = (1_000.0, 500_000.0);
/// Accuracy scale: win rate, linear.
pub const ACCURACY_SCALE: (f64, f64) = (0.30, 0.80);
/// Form scale: `d7` in USD, log.
pub const FORM_SCALE: (f64, f64) = (500.0, 100_000.0);
/// Activity scale: minutes since the last trade; full up to an hour, zero at 30 days, log.
pub const ACTIVITY_SCALE: (f64, f64) = (60.0, 30.0 * 24.0 * 60.0);

fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

/// Share of a log scale `lo..hi`; 0 at or below `lo` (and for values not above zero).
fn log_share(v: f64, (lo, hi): (f64, f64)) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    clamp01((v / lo).ln() / (hi / lo).ln())
}

fn part(kind: PartKind, weight: f64, raw: Option<f64>, share: Option<f64>) -> Part {
    let (share, status) = match (raw, share) {
        (None, _) => (0.0, PartStatus::NoData),
        (Some(_), None) => (0.0, PartStatus::NeedsProfit),
        (Some(_), Some(s)) => (s, PartStatus::Counted),
    };
    Part {
        kind,
        raw,
        all_time: false,
        share,
        points: 100.0 * weight * share,
        status,
    }
}

/// Tier by Score.
pub fn tier(score: u8) -> Tier {
    match score {
        70.. => Tier::Goat,
        50.. => Tier::Diamond,
        35.. => Tier::Sendoor,
        _ => Tier::Degen,
    }
}

/// Badges, in [`Badge`] order.
pub fn badges(m: &Metrics) -> Vec<Badge> {
    let at_least = |v: Option<f64>, x: f64| v.is_some_and(|v| v >= x);
    let at_most = |v: Option<f64>, x: f64| v.is_some_and(|v| v <= x);
    [
        (Badge::Sniper, at_least(m.wr30, 0.65)),
        (Badge::OnFire, at_least(m.d7, 20_000.0)),
        (Badge::Whale, at_least(m.avg_pos, 1_000.0)),
        (Badge::Scalper, at_most(m.hold_s, 15.0)),
        (Badge::Hodl, at_least(m.hold_s, 3_600.0)),
        (Badge::Live, m.last_min_at_fetch.is_some_and(|l| l <= 60)),
        (Badge::InTopToday, m.in_top_today),
    ]
    .into_iter()
    .filter_map(|(b, on)| on.then_some(b))
    .collect()
}

/// Parts, Score, tier and badges of `m`.
pub fn score(m: &Metrics) -> ScoreCard {
    let profit = part(
        PartKind::Profit,
        PROFIT_WEIGHT,
        m.d30,
        m.d30.map(|v| log_share(v, PROFIT_SCALE)),
    );

    let (wr, all_time) = match (m.wr30, m.wr_all) {
        (Some(w), _) => (Some(w), false),
        (None, Some(w)) => (Some(w), true),
        (None, None) => (None, false),
    };
    let (lo, hi) = ACCURACY_SCALE;
    let accuracy_share = wr
        .filter(|_| m.d30.is_some_and(|d| d > 0.0))
        .map(|w| clamp01((w - lo) / (hi - lo)));
    let mut accuracy = part(PartKind::Accuracy, ACCURACY_WEIGHT, wr, accuracy_share);
    accuracy.all_time = all_time;

    let form = part(
        PartKind::Form,
        FORM_WEIGHT,
        m.d7,
        m.d7.map(|v| log_share(v, FORM_SCALE)),
    );

    let last = m.last_min_at_fetch.map(|l| l as f64);
    let activity_share = last.map(|l| {
        let (full, zero) = ACTIVITY_SCALE;
        if l <= full {
            1.0
        } else if l >= zero {
            0.0
        } else {
            1.0 - (l / full).ln() / (zero / full).ln()
        }
    });
    let activity = part(PartKind::Activity, ACTIVITY_WEIGHT, last, activity_share);

    let total = 100.0
        * (PROFIT_WEIGHT * profit.share
            + ACCURACY_WEIGHT * accuracy.share
            + FORM_WEIGHT * form.share
            + ACTIVITY_WEIGHT * activity.share);
    let score = total.round().clamp(0.0, 100.0) as u8;
    ScoreCard {
        score,
        tier: tier(score),
        parts: vec![profit, accuracy, form, activity],
        badges: badges(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tekkerrss() -> Metrics {
        Metrics {
            d7: Some(-1_600.0),
            d30: Some(207_060.0),
            wr_all: Some(0.442),
            avg_pos: Some(1_530.0),
            last_min_at_fetch: Some(120),
            ..Metrics::default()
        }
    }

    #[test]
    fn control_example_tekkerrss() {
        let card = score(&tekkerrss());
        assert_eq!(card.score, 55);
        assert_eq!(card.tier, Tier::Diamond);
        let kinds: Vec<PartKind> = card.parts.iter().map(|p| p.kind).collect();
        assert_eq!(kinds, PartKind::ALL);
        let [profit, accuracy, form, activity] = &card.parts[..] else {
            panic!("four parts");
        };
        assert!((profit.share - 0.8582).abs() < 1e-3, "{}", profit.share);
        assert!((profit.points - 34.33).abs() < 1e-2);
        assert_eq!(accuracy.raw, Some(0.442));
        assert!(accuracy.all_time);
        assert!((accuracy.share - 0.284).abs() < 1e-9);
        assert_eq!(form.status, PartStatus::Counted);
        assert_eq!(form.share, 0.0);
        assert!((activity.share - 0.8946).abs() < 1e-3);
        // Whale only; never Sniper from the all-time win rate.
        assert_eq!(card.badges, [Badge::Whale]);
    }

    #[test]
    fn missing_parts_score_zero_with_no_data() {
        let card = score(&Metrics::default());
        assert_eq!(card.score, 0);
        assert_eq!(card.tier, Tier::Degen);
        for p in &card.parts {
            assert_eq!(
                (p.status, p.share, p.points, p.raw),
                (PartStatus::NoData, 0.0, 0.0, None)
            );
        }
        assert!(card.badges.is_empty());

        // Only activity known: a trade within the hour gives its full 15 points.
        let card = score(&Metrics {
            last_min_at_fetch: Some(5),
            ..Metrics::default()
        });
        assert_eq!(card.score, 15);
        assert_eq!(card.parts[3].status, PartStatus::Counted);
        assert_eq!(card.parts[0].status, PartStatus::NoData);
        assert_eq!(card.badges, [Badge::Live]);
    }

    #[test]
    fn accuracy_needs_profit_above_zero() {
        for d30 in [Some(0.0), Some(-5_000.0), None] {
            let card = score(&Metrics {
                d30,
                wr30: Some(0.80),
                ..Metrics::default()
            });
            let acc = &card.parts[1];
            assert_eq!(acc.status, PartStatus::NeedsProfit, "{d30:?}");
            assert_eq!((acc.share, acc.points, acc.raw), (0.0, 0.0, Some(0.80)));
        }
        let card = score(&Metrics {
            d30: Some(1.0),
            wr30: Some(0.80),
            ..Metrics::default()
        });
        assert_eq!(card.parts[1].status, PartStatus::Counted);
        assert_eq!(card.parts[1].points, 25.0);
        assert_eq!(card.score, 25);
        assert_eq!(card.badges, [Badge::Sniper]);
    }

    #[test]
    fn scales_clamp_and_tiers() {
        let top = Metrics {
            d30: Some(10_000_000.0),
            d7: Some(10_000_000.0),
            wr30: Some(0.99),
            last_min_at_fetch: Some(0),
            ..Metrics::default()
        };
        assert_eq!(score(&top).score, 100);
        let dead = Metrics {
            d30: Some(999.0),
            d7: Some(499.0),
            wr30: Some(0.10),
            last_min_at_fetch: Some(50_000),
            ..Metrics::default()
        };
        let card = score(&dead);
        assert_eq!(card.score, 0);
        assert!(card.parts.iter().all(|p| p.status == PartStatus::Counted));
        assert_eq!(
            [
                tier(100),
                tier(70),
                tier(69),
                tier(50),
                tier(49),
                tier(35),
                tier(34),
                tier(0)
            ],
            [
                Tier::Goat,
                Tier::Goat,
                Tier::Diamond,
                Tier::Diamond,
                Tier::Sendoor,
                Tier::Sendoor,
                Tier::Degen,
                Tier::Degen
            ]
        );
    }

    #[test]
    fn badge_thresholds() {
        let m = Metrics {
            wr30: Some(0.65),
            d7: Some(20_000.0),
            avg_pos: Some(1_000.0),
            hold_s: Some(15.0),
            last_min_at_fetch: Some(60),
            in_top_today: true,
            ..Metrics::default()
        };
        assert_eq!(
            badges(&m),
            [
                Badge::Sniper,
                Badge::OnFire,
                Badge::Whale,
                Badge::Scalper,
                Badge::Live,
                Badge::InTopToday
            ]
        );
        let m = Metrics {
            wr30: Some(0.649),
            d7: Some(19_999.0),
            avg_pos: Some(999.0),
            hold_s: Some(3_600.0),
            last_min_at_fetch: Some(61),
            ..Metrics::default()
        };
        assert_eq!(badges(&m), [Badge::Hodl]);
    }
}
