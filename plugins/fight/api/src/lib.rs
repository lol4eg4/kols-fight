//! KOL Fight API (layer 1): a fight of 2 to 4 KOLs on one snapshot.
//!
//! Rounds in order ([`RoundKind::ALL`]); each round's best value wins 1 point; k fighters
//! tied on the best value split it (1/k each); when every fighter ties the round is void; a
//! round where any fighter has no value is void («not counted: not everyone has data»).
//! Every fighter starts with [`HP_SEGMENTS`] segments and loses one for each counted round it
//! does not win or split. Winner: most points; equal points -> higher Arena Score; equal
//! Score too -> draw. The same `(ids, date)` always gives the same result.

use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

pub use api_score::{ScoreCard, Tier};
pub use api_snapshots::MetricSource;

#[path = "../../stub/mod.rs"]
pub mod stub;

/// KOL id (registry slug).
pub type KolId = String;

/// HP segments each fighter starts with.
pub const HP_SEGMENTS: u8 = 8;
/// Fewest fighters.
pub const MIN_FIGHTERS: usize = 2;
/// Most fighters.
pub const MAX_FIGHTERS: usize = 4;

/// The eight rounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundKind {
    /// 1. Arena Score: higher wins.
    Score,
    /// 2. PnL 30 days (`d30`): higher wins.
    Pnl30d,
    /// 3. PnL 7 days (`d7`): higher wins.
    Pnl7d,
    /// 4. PnL 24 hours (`d1`): higher wins.
    Pnl24h,
    /// 5. Win rate: `wr30`, or `wr_all` (marked `*`) when `wr30` is `None`: higher wins.
    WinRate,
    /// 6. Win rate 7 days (`wr7`): higher wins.
    WinRate7d,
    /// 7. Position size (`avg_pos`): higher wins.
    PositionSize,
    /// 8. Freshness: minutes since the last trade at fetch (`last_min_at_fetch`): lower wins.
    LastTrade,
}

impl RoundKind {
    /// Rounds in fight order.
    pub const ALL: [RoundKind; 8] = [
        RoundKind::Score,
        RoundKind::Pnl30d,
        RoundKind::Pnl7d,
        RoundKind::Pnl24h,
        RoundKind::WinRate,
        RoundKind::WinRate7d,
        RoundKind::PositionSize,
        RoundKind::LastTrade,
    ];

    /// Whether the higher value wins (false only for [`RoundKind::LastTrade`]).
    pub fn higher_wins(self) -> bool {
        self != RoundKind::LastTrade
    }
}

/// One fighter's value in a round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoundValue {
    pub kol: KolId,
    /// `None` = no data.
    pub value: Option<f64>,
    /// Win rate round only: the value is the all-time win rate (`*`).
    pub all_time: bool,
    /// Source of the metric row; `None` for the Score round.
    pub source: Option<MetricSource>,
    /// When that row was fetched; `None` for the Score round.
    pub fetched_at: Option<DateTime<Utc>>,
}

/// Why a round is void.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoidReason {
    /// A fighter has no value.
    MissingData,
    /// Every fighter has the best value.
    AllTied,
}

/// Result of one round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoundOutcome {
    /// One fighter has the best value: 1 point.
    Win { kol: KolId },
    /// Several (not all) share the best value: 1/k point each.
    Tie { kols: Vec<KolId> },
    /// Not counted.
    Void { reason: VoidReason },
}

/// Points a fighter got in a round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Award {
    pub kol: KolId,
    /// 1, or 1/k for a tie.
    pub points: f64,
}

/// One round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Round {
    /// 1..=8.
    pub number: u8,
    pub kind: RoundKind,
    /// One value per fighter, in fighter order.
    pub values: Vec<RoundValue>,
    pub outcome: RoundOutcome,
    /// Empty for a void round.
    pub awards: Vec<Award>,
    /// Fighters that lose one HP segment (counted round, no points).
    pub damaged: Vec<KolId>,
}

/// One fighter and their totals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fighter {
    pub kol: KolId,
    pub name: String,
    /// Place on the snapshot's ranking.
    pub rank: Option<u32>,
    /// Score, tier, parts and badges («Where Score comes from»).
    pub card: ScoreCard,
    /// Sum of round points.
    pub points: f64,
    /// HP segments left, 0..=[`HP_SEGMENTS`].
    pub hp: u8,
    /// Rounds this fighter won or split, for the summary lines.
    pub rounds_won: Vec<RoundKind>,
}

/// How the winner was decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Most points.
    Rounds,
    /// Equal points, higher Arena Score («Tie on rounds — X wins on higher Arena Score»).
    ScoreTiebreak,
    /// Equal points and equal Score: no winner.
    Draw,
}

/// Fight result: the same as `GET /api/fight` returns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FightResult {
    /// Snapshot date the fight ran on.
    pub date: NaiveDate,
    /// That snapshot's last close time (part of the image cache key).
    pub snapshot_closed_at: Option<DateTime<Utc>>,
    /// Fighters in the requested order, without missing ids.
    pub fighters: Vec<Fighter>,
    /// The eight rounds in order.
    pub rounds: Vec<Round>,
    /// `None` for a draw.
    pub winner: Option<KolId>,
    pub decision: Decision,
    /// Rounds counted («6 of 8 rounds counted»).
    pub counted_rounds: u8,
    /// Requested ids not on that snapshot's board (unknown KOL, typo in a link).
    pub missing: Vec<KolId>,
}

/// Fight call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FightError {
    /// Fight is off for this app or person (stub).
    #[error("fight is off")]
    Off,
    /// The fight plugin is not built yet.
    #[error("fight is not built yet")]
    NotBuilt,
    /// Fewer than two fighters found on the snapshot: open the fighter picker.
    #[error("pick 2-4 KOLs")]
    TooFewFighters { missing: Vec<KolId> },
    #[error("at most 4 fighters, got {count}")]
    TooManyFighters { count: usize },
    #[error("{kol} is picked twice")]
    DuplicateFighter { kol: KolId },
    /// No closed snapshot for the date (or none at all when `date` is `None`).
    #[error("no snapshot")]
    NoSnapshot { date: Option<NaiveDate> },
    #[error("fight: {message}")]
    Internal { message: String },
}

/// KOL Fight operations.
#[async_trait]
pub trait FightApi: Send + Sync {
    /// Runs a fight of `ids` on the snapshot for `date` (`None` = latest closed snapshot).
    async fn fight(
        &self,
        person: &Person,
        ids: &[KolId],
        date: Option<NaiveDate>,
    ) -> Result<FightResult, FightError>;
}
