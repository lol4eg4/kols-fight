//! Backend of the `tournament` plugin (layer 4): a bracket of 8 KOLs.
//!
//! `GET /api/tournament?ids=<8 ids>&date=YYYY-MM-DD` (public) runs the bracket with the fight
//! rules: the ids pair up in the order given (1 v 2, 3 v 4, 5 v 6, 7 v 8), each pair fights on
//! one snapshot, the winners meet in the semifinals and then the final. A draw advances the
//! KOL with the better place on the ranking (the first listed when the places are equal).
//! `GET /api/tournament/lineups?date=` (public) gives the ready lineups for the Top-8 and
//! Our network buttons, already in bracket order (seeded 1 v 8, 4 v 5, 2 v 7, 3 v 6).
//! Nothing is stored: the same ids and date always give the same bracket.

use api_fight::{Decision, FightApi, FightError, FightResult, Fighter, KolId};
use api_roster::{RosterApi, RosterError};
use api_score::{ScoreApi, ScoreError, Tier};
use arena_platform::{Api, Ctx, Person, Plugin, PluginEntry, Route, StartError, Tag};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::migrate::Migrator;
use std::collections::HashSet;
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "tournament";

/// Migrations from `plugins/tournament/migrations`, run in schema `tournament`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// Fighters in a bracket.
pub const BRACKET_SIZE: usize = 8;
/// Longest id accepted.
const MAX_ID_LEN: usize = 64;

/// Where the seeds sit in the bracket order: 1 v 8, 4 v 5, 2 v 7, 3 v 6.
const SEED_ORDER: [usize; BRACKET_SIZE] = [0, 7, 3, 4, 1, 6, 2, 5];

/// One fighter in the bracket.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Contestant {
    pub kol: KolId,
    pub name: String,
    /// Place on the day's ranking.
    pub rank: Option<u32>,
    pub score: u8,
    pub tier: Tier,
}

/// How a match was decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchDecision {
    /// Most round points.
    Rounds,
    /// Equal points, higher Arena Score.
    ScoreTiebreak,
    /// The fight was a draw: the better ranked KOL advances.
    RankTiebreak,
}

/// One fight of the bracket.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Match {
    pub a: Contestant,
    pub b: Contestant,
    pub points_a: f64,
    pub points_b: f64,
    pub counted_rounds: u8,
    /// The KOL that goes on.
    pub winner: KolId,
    pub decision: MatchDecision,
}

/// Quarterfinals, semifinals or final.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundName {
    Quarterfinals,
    Semifinals,
    Final,
}

/// One column of the bracket.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BracketRound {
    pub name: RoundName,
    pub matches: Vec<Match>,
}

/// Body of `GET /api/tournament`. Public: names, places, Scores and fight totals.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TournamentView {
    pub date: NaiveDate,
    /// Quarterfinals (4 matches), semifinals (2), final (1).
    pub rounds: Vec<BracketRound>,
    pub champion: Contestant,
}

/// Body of `GET /api/tournament/lineups`: ids in bracket order; an empty list = not available.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Lineups {
    pub date: NaiveDate,
    /// The 8 best ranked KOLs.
    pub top: Vec<KolId>,
    /// The 8 best ranked KOLs of the network; empty with fewer than 8 on the board.
    pub network: Vec<KolId>,
}

/// Why a tournament call has no answer.
#[derive(Debug, PartialEq, Eq)]
pub enum TournamentError {
    BadIds(String),
    BadDate,
    /// These ids are not on the snapshot's board.
    UnknownKols(Vec<KolId>),
    /// No closed snapshot for the date.
    NoSnapshot,
    /// Fight, score or roster is off or not built.
    Unavailable,
    Internal(String),
}

/// The ids of `raw` (comma separated): exactly [`BRACKET_SIZE`], each valid, none twice.
pub fn parse_ids(raw: &str) -> Result<Vec<KolId>, TournamentError> {
    let ids: Vec<KolId> = raw.split(',').map(|s| s.trim().to_string()).collect();
    if ids.len() != BRACKET_SIZE {
        return Err(TournamentError::BadIds(format!(
            "ids must list exactly {BRACKET_SIZE} KOLs, got {}",
            ids.len()
        )));
    }
    let mut seen = HashSet::new();
    for id in &ids {
        let ok = !id.is_empty()
            && id.len() <= MAX_ID_LEN
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
        if !ok {
            return Err(TournamentError::BadIds(
                "an id is empty, too long or has characters outside letters, digits, - _ .".into(),
            ));
        }
        if !seen.insert(id.as_str()) {
            return Err(TournamentError::BadIds(format!("{id} is listed twice")));
        }
    }
    Ok(ids)
}

/// `date` as YYYY-MM-DD, nothing else.
pub fn parse_date(raw: &str) -> Result<NaiveDate, TournamentError> {
    if raw.len() != 10 {
        return Err(TournamentError::BadDate);
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| TournamentError::BadDate)
}

fn contestant(f: &Fighter) -> Contestant {
    Contestant {
        kol: f.kol.clone(),
        name: f.name.clone(),
        rank: f.rank,
        score: f.card.score,
        tier: f.card.tier,
    }
}

/// Who goes on from `result`; a draw sends on the better ranked (the first when places are equal).
fn advance(result: &FightResult, a: &Fighter, b: &Fighter) -> (KolId, MatchDecision) {
    match (&result.winner, result.decision) {
        (Some(w), Decision::ScoreTiebreak) => (w.clone(), MatchDecision::ScoreTiebreak),
        (Some(w), _) => (w.clone(), MatchDecision::Rounds),
        (None, _) => {
            // `None` ranks last
            let place = |f: &Fighter| f.rank.unwrap_or(u32::MAX);
            let w = if place(b) < place(a) { b } else { a };
            (w.kol.clone(), MatchDecision::RankTiebreak)
        }
    }
}

fn fight_error(e: FightError) -> TournamentError {
    match e {
        FightError::Off | FightError::NotBuilt => TournamentError::Unavailable,
        FightError::TooFewFighters { missing } => TournamentError::UnknownKols(missing),
        FightError::NoSnapshot { .. } => TournamentError::NoSnapshot,
        other => TournamentError::Internal(other.to_string()),
    }
}

/// Runs the bracket of `ids` (as [`parse_ids`] returns them) on the snapshot of `date`
/// (`None` = the latest closed one; the first fight fixes the date for the others).
pub async fn run_bracket(
    fight: &dyn FightApi,
    person: &Person,
    ids: &[KolId],
    date: Option<NaiveDate>,
) -> Result<TournamentView, TournamentError> {
    let mut date = date;
    let mut field: Vec<KolId> = ids.to_vec();
    let mut contestants: Vec<Contestant> = Vec::new();
    let mut rounds = Vec::new();
    for name in [
        RoundName::Quarterfinals,
        RoundName::Semifinals,
        RoundName::Final,
    ] {
        let mut matches = Vec::new();
        let mut next = Vec::new();
        contestants.clear();
        for pair in field.chunks(2) {
            let result = fight.fight(person, pair, date).await.map_err(fight_error)?;
            date = Some(result.date);
            let (Some(a), Some(b)) = (
                result.fighters.iter().find(|f| f.kol == pair[0]),
                result.fighters.iter().find(|f| f.kol == pair[1]),
            ) else {
                return Err(TournamentError::UnknownKols(result.missing));
            };
            let (winner, decision) = advance(&result, a, b);
            matches.push(Match {
                a: contestant(a),
                b: contestant(b),
                points_a: a.points,
                points_b: b.points,
                counted_rounds: result.counted_rounds,
                winner: winner.clone(),
                decision,
            });
            contestants.push(contestant(if winner == a.kol { a } else { b }));
            next.push(winner);
        }
        rounds.push(BracketRound { name, matches });
        field = next;
    }
    let champion = contestants
        .pop()
        .ok_or_else(|| TournamentError::Internal("bracket has no champion".into()))?;
    Ok(TournamentView {
        date: date.ok_or_else(|| TournamentError::Internal("bracket has no date".into()))?,
        rounds,
        champion,
    })
}

/// The first [`BRACKET_SIZE`] of `by_rank` (best first) in bracket order; empty with fewer.
fn seeded(by_rank: &[KolId]) -> Vec<KolId> {
    if by_rank.len() < BRACKET_SIZE {
        return vec![];
    }
    SEED_ORDER.iter().map(|&i| by_rank[i].clone()).collect()
}

/// The Top-8 and Our network lineups for `date`.
pub async fn lineups(
    score: &dyn ScoreApi,
    roster: &dyn RosterApi,
    person: &Person,
    date: NaiveDate,
) -> Result<Lineups, TournamentError> {
    let ranking = score.ranking(person, date).await.map_err(|e| match e {
        ScoreError::NoSnapshot { .. }
        | ScoreError::NotClosed { .. }
        | ScoreError::NotRanked { .. } => TournamentError::NoSnapshot,
        ScoreError::Off | ScoreError::NotBuilt => TournamentError::Unavailable,
        other => TournamentError::Internal(other.to_string()),
    })?;
    let members: HashSet<KolId> = roster
        .members(person)
        .await
        .map_err(|e| match e {
            RosterError::Off | RosterError::NotBuilt => TournamentError::Unavailable,
            other => TournamentError::Internal(other.to_string()),
        })?
        .into_iter()
        .map(|m| m.kol)
        .collect();
    let mut rows = ranking.rows;
    rows.sort_by(|x, y| x.rank.cmp(&y.rank).then_with(|| x.kol.cmp(&y.kol)));
    let all: Vec<KolId> = rows.iter().map(|r| r.kol.clone()).collect();
    let net: Vec<KolId> = all
        .iter()
        .filter(|k| members.contains(*k))
        .cloned()
        .collect();
    Ok(Lineups {
        date,
        top: seeded(&all),
        network: seeded(&net),
    })
}

/// What this plugin calls.
#[derive(Clone)]
struct Uses {
    fight: Api<dyn FightApi>,
    score: Api<dyn ScoreApi>,
    roster: Api<dyn RosterApi>,
}

pub struct TournamentPlugin {
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        fight: ctx.apis.require::<dyn FightApi>(NAME)?,
        score: ctx.apis.require::<dyn ScoreApi>(NAME)?,
        roster: ctx.apis.require::<dyn RosterApi>(NAME)?,
    };
    Ok(Arc::new(TournamentPlugin { uses }))
}

impl Plugin for TournamentPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        vec![
            Route::new(
                Tag::Public,
                "/api/tournament",
                get(get_tournament).with_state(self.uses.clone()),
            ),
            Route::new(
                Tag::Public,
                "/api/tournament/lineups",
                get(get_lineups).with_state(self.uses.clone()),
            ),
        ]
    }
}

#[derive(Deserialize)]
struct BracketQuery {
    ids: Option<String>,
    date: Option<String>,
}

#[derive(Deserialize)]
struct DateQuery {
    date: Option<String>,
}

fn problem(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

fn error_response(e: TournamentError) -> Response {
    match e {
        TournamentError::BadIds(m) => problem(StatusCode::BAD_REQUEST, "bad_ids", &m),
        TournamentError::BadDate => problem(
            StatusCode::BAD_REQUEST,
            "bad_date",
            "date must be YYYY-MM-DD",
        ),
        TournamentError::UnknownKols(missing) => problem(
            StatusCode::NOT_FOUND,
            "unknown_kol",
            &format!("not on the board: {}", missing.join(", ")),
        ),
        TournamentError::NoSnapshot => problem(
            StatusCode::NOT_FOUND,
            "no_snapshot",
            "no closed snapshot for that date",
        ),
        TournamentError::Unavailable => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the tournament is not available now",
        ),
        TournamentError::Internal(message) => {
            tracing::error!(%message, "tournament failed");
            problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "tournament failed",
            )
        }
    }
}

async fn get_tournament(
    State(uses): State<Uses>,
    person: Person,
    Query(q): Query<BracketQuery>,
) -> Response {
    let ids = match parse_ids(q.ids.as_deref().unwrap_or("")) {
        Ok(ids) => ids,
        Err(e) => return error_response(e),
    };
    let date = match q.date.as_deref().map(parse_date).transpose() {
        Ok(d) => d,
        Err(e) => return error_response(e),
    };
    let fight = uses.fight.pick(&person);
    match run_bracket(&*fight, &person, &ids, date).await {
        Ok(view) => Json(view).into_response(),
        Err(e) => error_response(e),
    }
}

async fn get_lineups(
    State(uses): State<Uses>,
    person: Person,
    Query(q): Query<DateQuery>,
) -> Response {
    let date = match parse_date(q.date.as_deref().unwrap_or("")) {
        Ok(d) => d,
        Err(e) => return error_response(e),
    };
    let score = uses.score.pick(&person);
    let roster = uses.roster.pick(&person);
    match lineups(&*score, &*roster, &person, date).await {
        Ok(l) => Json(l).into_response(),
        Err(e) => error_response(e),
    }
}
