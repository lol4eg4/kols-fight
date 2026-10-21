//! Backend of the `duel` plugin (layer 4): the duel of the day.
//!
//! `GET /api/duel?date=YYYY-MM-DD` (public) returns the pair of the day and the result of
//! their fight. The pair is the same for every visitor: it comes from a hash of the date over
//! the top [`TOP`] of that day's ranking, and is stored the first time it is asked for, so a
//! later re-rank of the day does not change it.

use api_fight::{FightApi, FightError};
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
use sqlx::{PgPool, migrate::Migrator};
use std::sync::Arc;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "duel";

/// Migrations from `plugins/duel/migrations`, run in schema `duel`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// The pair comes from this many top places.
pub const TOP: usize = 20;

/// KOL id (registry slug).
pub type KolId = String;

/// FNV-1a over the bytes: the same on every machine and release, unlike `DefaultHasher`.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Final mixing step of splitmix64, to get a second independent number from the hash.
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// The pair for `date` from `top` (best rank first; only the first [`TOP`] count): two
/// different KOLs, the better ranked first. The same `date` and `top` always give the same pair.
/// `None` with fewer than two KOLs.
pub fn pick_pair(date: NaiveDate, top: &[KolId]) -> Option<(KolId, KolId)> {
    let top = &top[..top.len().min(TOP)];
    let n = top.len() as u64;
    if n < 2 {
        return None;
    }
    let h = fnv1a(format!("duel:{date}").as_bytes());
    let i = h % n;
    let mut j = mix(h) % (n - 1);
    if j >= i {
        j += 1;
    }
    let (a, b) = (i.min(j) as usize, i.max(j) as usize);
    Some((top[a].clone(), top[b].clone()))
}

/// One fighter of the duel.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DuelFighter {
    pub kol: KolId,
    pub name: String,
    /// Place on the day's ranking.
    pub rank: Option<u32>,
    pub score: u8,
    pub tier: Tier,
    /// Round points in the fight.
    pub points: f64,
    /// HP segments left after the fight.
    pub hp: u8,
}

/// Body of `GET /api/duel`. Public: the pair and the fight's totals.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DuelView {
    pub date: NaiveDate,
    /// The two fighters, the better ranked first.
    pub fighters: Vec<DuelFighter>,
    /// `None` for a draw.
    pub winner: Option<KolId>,
    pub counted_rounds: u8,
}

/// Why `GET /api/duel` has no answer.
#[derive(Debug, PartialEq, Eq)]
pub enum DuelError {
    /// The date has no ranking, or fewer than two KOLs, or the pair cannot fight.
    NoDuel(NaiveDate),
    /// Score or fight is off or not built.
    Unavailable,
    Internal(String),
}

/// The pairs of the days, kept in the plugin's schema.
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        Store { pool }
    }

    /// The stored pair of `date`.
    pub async fn pair(&self, date: NaiveDate) -> Result<Option<(KolId, KolId)>, sqlx::Error> {
        sqlx::query_as("SELECT kol_a, kol_b FROM duel.pick WHERE on_date = $1")
            .bind(date)
            .fetch_optional(&self.pool)
            .await
    }

    /// Stores the pair of `date` unless one is stored already; returns the stored one.
    pub async fn keep(
        &self,
        date: NaiveDate,
        a: &str,
        b: &str,
    ) -> Result<(KolId, KolId), sqlx::Error> {
        sqlx::query(
            "INSERT INTO duel.pick (on_date, kol_a, kol_b) VALUES ($1, $2, $3) \
             ON CONFLICT (on_date) DO NOTHING",
        )
        .bind(date)
        .bind(a)
        .bind(b)
        .execute(&self.pool)
        .await?;
        sqlx::query_as("SELECT kol_a, kol_b FROM duel.pick WHERE on_date = $1")
            .bind(date)
            .fetch_one(&self.pool)
            .await
    }
}

fn db(e: sqlx::Error) -> DuelError {
    DuelError::Internal(e.to_string())
}

/// The duel of `date`: the stored pair, or a new one from the top of the ranking.
pub async fn duel(
    store: &Store,
    score: &dyn ScoreApi,
    fight: &dyn FightApi,
    person: &Person,
    date: NaiveDate,
) -> Result<DuelView, DuelError> {
    let (a, b) = match store.pair(date).await.map_err(db)? {
        Some(pair) => pair,
        None => {
            let ranking = match score.ranking(person, date).await {
                Ok(r) => r,
                Err(
                    ScoreError::NoSnapshot { .. }
                    | ScoreError::NotClosed { .. }
                    | ScoreError::NotRanked { .. },
                ) => return Err(DuelError::NoDuel(date)),
                Err(ScoreError::Off | ScoreError::NotBuilt) => return Err(DuelError::Unavailable),
                Err(e) => return Err(DuelError::Internal(e.to_string())),
            };
            let mut rows = ranking.rows;
            rows.sort_by(|x, y| x.rank.cmp(&y.rank).then_with(|| x.kol.cmp(&y.kol)));
            let top: Vec<KolId> = rows.into_iter().take(TOP).map(|r| r.kol).collect();
            let (a, b) = pick_pair(date, &top).ok_or(DuelError::NoDuel(date))?;
            store.keep(date, &a, &b).await.map_err(db)?
        }
    };
    let result = match fight.fight(person, &[a, b], Some(date)).await {
        Ok(r) => r,
        Err(FightError::Off | FightError::NotBuilt) => return Err(DuelError::Unavailable),
        Err(FightError::TooFewFighters { .. } | FightError::NoSnapshot { .. }) => {
            return Err(DuelError::NoDuel(date));
        }
        Err(e) => return Err(DuelError::Internal(e.to_string())),
    };
    Ok(DuelView {
        date,
        fighters: result
            .fighters
            .into_iter()
            .map(|f| DuelFighter {
                kol: f.kol,
                name: f.name,
                rank: f.rank,
                score: f.card.score,
                tier: f.card.tier,
                points: f.points,
                hp: f.hp,
            })
            .collect(),
        winner: result.winner,
        counted_rounds: result.counted_rounds,
    })
}

/// What this plugin holds and calls.
#[derive(Clone)]
struct Uses {
    store: Arc<Store>,
    score: Api<dyn ScoreApi>,
    fight: Api<dyn FightApi>,
}

pub struct DuelPlugin {
    uses: Uses,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        store: Arc::new(Store::new(ctx.pool.clone())),
        score: ctx.apis.require::<dyn ScoreApi>(NAME)?,
        fight: ctx.apis.require::<dyn FightApi>(NAME)?,
    };
    Ok(Arc::new(DuelPlugin { uses }))
}

impl Plugin for DuelPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        vec![Route::new(
            Tag::Public,
            "/api/duel",
            get(get_duel).with_state(self.uses.clone()),
        )]
    }
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

async fn get_duel(
    State(uses): State<Uses>,
    person: Person,
    Query(q): Query<DateQuery>,
) -> Response {
    let Some(date) = q
        .date
        .as_deref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    else {
        return problem(
            StatusCode::BAD_REQUEST,
            "bad_date",
            "date must be YYYY-MM-DD",
        );
    };
    let score = uses.score.pick(&person);
    let fight = uses.fight.pick(&person);
    match duel(&uses.store, &*score, &*fight, &person, date).await {
        Ok(view) => Json(view).into_response(),
        Err(DuelError::NoDuel(d)) => problem(
            StatusCode::NOT_FOUND,
            "no_duel",
            &format!("no duel for {d}"),
        ),
        Err(DuelError::Unavailable) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the duel is not available now",
        ),
        Err(DuelError::Internal(message)) => {
            tracing::error!(%message, "duel failed");
            problem(StatusCode::INTERNAL_SERVER_ERROR, "internal", "duel failed")
        }
    }
}
