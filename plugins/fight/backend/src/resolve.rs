//! The fight itself: a pure function of the fighters' cards and metric rows on one snapshot.
//!
//! ```text
//! round:   best value among fighters (lower for LastTrade)
//!   any value missing ........ void (missing_data)
//!   every fighter has best ... void (all_tied)
//!   one has best ............. win: 1 point
//!   k (< all) have best ...... tie: 1/k point each
//! counted round: every fighter without points loses 1 HP segment
//! winner: most points -> higher Arena Score -> draw
//! ```
//!
//! Points are kept in twelfths (1/k is exact for k = 1..=4), so ties on points are exact.

use api_fight::{
    Award, Decision, FightResult, Fighter, HP_SEGMENTS, KolId, Round, RoundKind, RoundOutcome,
    RoundValue, ScoreCard, VoidReason,
};
use api_snapshots::MetricRow;
use chrono::NaiveDate;

/// One fighter as found on the snapshot.
#[derive(Clone, Debug)]
pub struct FighterInput {
    pub kol: KolId,
    pub name: String,
    /// Place on the snapshot's ranking.
    pub rank: Option<u32>,
    pub card: ScoreCard,
    /// Metric row on the snapshot; `None` = no data in every metric round.
    pub row: Option<MetricRow>,
}

const WHOLE: u32 = 12;

/// Round name used in the summary lines and the image.
pub fn round_label(kind: RoundKind) -> &'static str {
    match kind {
        RoundKind::Score => "Arena Score",
        RoundKind::Pnl30d => "PnL 30d",
        RoundKind::Pnl7d => "PnL 7d",
        RoundKind::Pnl24h => "PnL 24h",
        RoundKind::WinRate => "Win rate",
        RoundKind::WinRate7d => "Win rate 7d",
        RoundKind::PositionSize => "Position size",
        RoundKind::LastTrade => "Last trade",
    }
}

fn value_of(kind: RoundKind, f: &FighterInput) -> RoundValue {
    if kind == RoundKind::Score {
        return RoundValue {
            kol: f.kol.clone(),
            value: Some(f64::from(f.card.score)),
            all_time: false,
            source: None,
            fetched_at: None,
        };
    }
    let (value, all_time) = match &f.row {
        None => (None, false),
        Some(row) => {
            let m = &row.metrics;
            match kind {
                RoundKind::Score => unreachable!("handled above"),
                RoundKind::Pnl30d => (m.d30, false),
                RoundKind::Pnl7d => (m.d7, false),
                RoundKind::Pnl24h => (m.d1, false),
                RoundKind::WinRate => match (m.wr30, m.wr_all) {
                    (Some(v), _) => (Some(v), false),
                    (None, Some(v)) => (Some(v), true),
                    (None, None) => (None, false),
                },
                RoundKind::WinRate7d => (m.wr7, false),
                RoundKind::PositionSize => (m.avg_pos, false),
                RoundKind::LastTrade => (m.last_min_at_fetch.map(|v| v as f64), false),
            }
        }
    };
    RoundValue {
        kol: f.kol.clone(),
        value: value.filter(|v| v.is_finite()),
        all_time,
        source: f.row.as_ref().map(|r| r.source),
        fetched_at: f.row.as_ref().map(|r| r.fetched_at),
    }
}

/// Points as text: `5`, `4.5`, `2.33`.
pub fn points_text(points: f64) -> String {
    let s = format!("{points:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Runs the eight rounds on `fighters` (2 to 4, already checked) for snapshot `date`.
/// `snapshot_closed_at` and `missing` are left empty for the caller.
pub fn resolve(fighters: &[FighterInput], date: NaiveDate) -> FightResult {
    let n = fighters.len();
    let mut twelfths = vec![0u32; n];
    let mut hp = vec![HP_SEGMENTS; n];
    let mut won: Vec<Vec<RoundKind>> = vec![Vec::new(); n];
    let mut rounds = Vec::with_capacity(RoundKind::ALL.len());
    let mut counted = 0u8;

    for (i, kind) in RoundKind::ALL.into_iter().enumerate() {
        let values: Vec<RoundValue> = fighters.iter().map(|f| value_of(kind, f)).collect();
        let nums: Option<Vec<f64>> = values.iter().map(|v| v.value).collect();
        let (outcome, awards, damaged) = match nums {
            None => (
                RoundOutcome::Void {
                    reason: VoidReason::MissingData,
                },
                Vec::new(),
                Vec::new(),
            ),
            Some(nums) => {
                let best = nums.iter().copied().fold(None, |acc: Option<f64>, v| {
                    Some(match acc {
                        None => v,
                        Some(a) if kind.higher_wins() => a.max(v),
                        Some(a) => a.min(v),
                    })
                });
                let best = best.expect("at least one fighter");
                let top: Vec<usize> = (0..n).filter(|&j| nums[j] == best).collect();
                if top.len() == n {
                    (
                        RoundOutcome::Void {
                            reason: VoidReason::AllTied,
                        },
                        Vec::new(),
                        Vec::new(),
                    )
                } else {
                    counted += 1;
                    let share = WHOLE / top.len() as u32;
                    let mut awards = Vec::new();
                    for &j in &top {
                        twelfths[j] += share;
                        won[j].push(kind);
                        awards.push(Award {
                            kol: fighters[j].kol.clone(),
                            points: f64::from(share) / f64::from(WHOLE),
                        });
                    }
                    let mut damaged = Vec::new();
                    for j in (0..n).filter(|j| !top.contains(j)) {
                        hp[j] = hp[j].saturating_sub(1);
                        damaged.push(fighters[j].kol.clone());
                    }
                    let outcome = if top.len() == 1 {
                        RoundOutcome::Win {
                            kol: fighters[top[0]].kol.clone(),
                        }
                    } else {
                        RoundOutcome::Tie {
                            kols: top.iter().map(|&j| fighters[j].kol.clone()).collect(),
                        }
                    };
                    (outcome, awards, damaged)
                }
            }
        };
        rounds.push(Round {
            number: i as u8 + 1,
            kind,
            values,
            outcome,
            awards,
            damaged,
        });
    }

    let most = twelfths.iter().copied().max().unwrap_or(0);
    let leaders: Vec<usize> = (0..n).filter(|&j| twelfths[j] == most).collect();
    let (winner, decision) = if leaders.len() == 1 {
        (Some(leaders[0]), Decision::Rounds)
    } else {
        let top_score = leaders.iter().map(|&j| fighters[j].card.score).max();
        let by_score: Vec<usize> = leaders
            .iter()
            .copied()
            .filter(|&j| Some(fighters[j].card.score) == top_score)
            .collect();
        if by_score.len() == 1 {
            (Some(by_score[0]), Decision::ScoreTiebreak)
        } else {
            (None, Decision::Draw)
        }
    };

    let out: Vec<Fighter> = fighters
        .iter()
        .enumerate()
        .map(|(j, f)| Fighter {
            kol: f.kol.clone(),
            name: f.name.clone(),
            rank: f.rank,
            card: f.card.clone(),
            points: f64::from(twelfths[j]) / f64::from(WHOLE),
            hp: hp[j],
            rounds_won: won[j].clone(),
        })
        .collect();

    let summary = summary(&out, winner, decision);
    FightResult {
        date,
        snapshot_closed_at: None,
        winner: winner.map(|j| fighters[j].kol.clone()),
        decision,
        counted_rounds: counted,
        counted_line: format!("{counted} of {} rounds counted", RoundKind::ALL.len()),
        summary,
        fighters: out,
        rounds,
        missing: Vec::new(),
    }
}

fn took(f: &Fighter) -> String {
    if f.rounds_won.is_empty() {
        format!("{} took no rounds", f.name)
    } else {
        let names: Vec<&str> = f.rounds_won.iter().map(|k| round_label(*k)).collect();
        format!("{} took {}", f.name, names.join(", "))
    }
}

/// Three lines: outcome; the winner's (or first fighter's) rounds; everyone else's rounds.
fn summary(fighters: &[Fighter], winner: Option<usize>, decision: Decision) -> Vec<String> {
    // Highest first: «2–1–0».
    let mut points: Vec<f64> = fighters.iter().map(|f| f.points).collect();
    points.sort_by(|a, b| b.total_cmp(a));
    let score: Vec<String> = points.into_iter().map(points_text).collect();
    let score = score.join("–");
    let first = match decision {
        Decision::Rounds => format!(
            "{} wins {score} on rounds.",
            fighters[winner.unwrap_or(0)].name
        ),
        Decision::ScoreTiebreak => format!(
            "Tie on rounds — {} wins on higher Arena Score.",
            fighters[winner.unwrap_or(0)].name
        ),
        Decision::Draw => format!("Draw: {score} on rounds and equal Arena Score."),
    };
    let lead = winner.unwrap_or(0);
    let rest: Vec<String> = (0..fighters.len())
        .filter(|&j| j != lead)
        .map(|j| took(&fighters[j]))
        .collect();
    vec![
        first,
        format!("{}.", took(&fighters[lead])),
        format!("{}.", rest.join("; ")),
    ]
}
