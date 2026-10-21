//! Backend of the `score` plugin (layer 1): Score, tier, badges and rank of every KOL on a
//! closed snapshot.
//!
//! No routes. The formula is the pure [`formula::score`]. `rank(date)` reads the snapshot
//! through the snapshots API, names through the registry API (for the stable order of full
//! ties), and stores the result in `score.ranking` and `score.result`. Install ranks day 0
//! (2026-10-18) when it is not ranked yet.

use api_registry::RegistryApi;
use api_score::{Badge, Metrics, Part, RankedKol, Ranking, ScoreApi, ScoreCard, ScoreError, Tier};
use api_snapshots::{SnapshotStatus, SnapshotsApi, SnapshotsError};
use arena_platform::{Api, BoxError, Ctx, Person, Plugin, PluginEntry, StartError};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgPool, migrate::Migrator, types::Json};
use std::{cmp::Ordering, collections::HashMap, sync::Arc};

pub mod formula;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "score";

/// Migrations from `plugins/score/migrations`, run in schema `score`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// Day 0: the prototype snapshot, ranked at install.
pub const DAY0: NaiveDate = NaiveDate::from_ymd_opt(2026, 10, 18).expect("day 0");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

pub struct ScorePlugin {
    score: Arc<Score>,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let score = Arc::new(Score {
        pool: ctx.pool.clone(),
        snapshots: ctx.apis.require::<dyn SnapshotsApi>(NAME)?,
        registry: ctx.apis.require::<dyn RegistryApi>(NAME)?,
    });
    ctx.apis.provide::<dyn ScoreApi>(
        NAME,
        vec![("default", score.clone())],
        Arc::new(api_score::stub::Stub),
    )?;
    Ok(Arc::new(ScorePlugin { score }))
}

#[async_trait]
impl Plugin for ScorePlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    /// Ranks day 0 when it is not ranked yet, so layer 1 shows a ranking before any collection.
    async fn install(&self, _ctx: &Ctx) -> Result<(), BoxError> {
        if !self.score.is_ranked(DAY0).await? {
            self.score.rank(&Person::SYSTEM, DAY0).await?;
        }
        Ok(())
    }
}

/// Score over its Postgres tables and the snapshots and registry APIs.
pub struct Score {
    pool: PgPool,
    snapshots: Api<dyn SnapshotsApi>,
    registry: Api<dyn RegistryApi>,
}

fn internal(e: impl std::fmt::Display) -> ScoreError {
    ScoreError::Internal {
        message: e.to_string(),
    }
}

fn tier_str(t: Tier) -> &'static str {
    match t {
        Tier::Goat => "goat",
        Tier::Diamond => "diamond",
        Tier::Sendoor => "sendoor",
        Tier::Degen => "degen",
    }
}

fn tier_of(s: &str) -> Result<Tier, ScoreError> {
    match s {
        "goat" => Ok(Tier::Goat),
        "diamond" => Ok(Tier::Diamond),
        "sendoor" => Ok(Tier::Sendoor),
        "degen" => Ok(Tier::Degen),
        other => Err(internal(format!("unknown tier {other}"))),
    }
}

/// One KOL to rank.
pub struct Entry<'a> {
    pub kol: &'a str,
    pub name: &'a str,
    pub d30: Option<f64>,
    pub card: ScoreCard,
}

/// Rank order: Score desc, then `d30` desc (missing last), then name asc (case-insensitive,
/// then exact, then id), only for a stable order.
pub fn rank_order(a: &Entry, b: &Entry) -> Ordering {
    let d30 = |e: &Entry| e.d30.unwrap_or(f64::NEG_INFINITY);
    b.card
        .score
        .cmp(&a.card.score)
        .then_with(|| d30(b).total_cmp(&d30(a)))
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then_with(|| a.name.cmp(b.name))
        .then_with(|| a.kol.cmp(b.kol))
}

#[derive(sqlx::FromRow)]
struct ResultRecord {
    kol: String,
    rank: i32,
    score: i16,
    tier: String,
    parts: Json<Vec<Part>>,
    badges: Json<Vec<Badge>>,
}

impl Score {
    async fn is_ranked(&self, date: NaiveDate) -> Result<bool, ScoreError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM score.ranking WHERE date = $1)")
            .bind(date)
            .fetch_one(&self.pool)
            .await
            .map_err(internal)
    }
}

#[async_trait]
impl ScoreApi for Score {
    async fn score(&self, _: &Person, metrics: &Metrics) -> Result<ScoreCard, ScoreError> {
        Ok(formula::score(metrics))
    }

    async fn rank(&self, person: &Person, date: NaiveDate) -> Result<(), ScoreError> {
        let view = match self.snapshots.pick(person).snapshot(person, date).await {
            Ok(view) => view,
            Err(SnapshotsError::NoSnapshot { date }) => {
                return Err(ScoreError::NoSnapshot { date });
            }
            Err(e) => return Err(internal(e)),
        };
        if view.snapshot.status != SnapshotStatus::Closed {
            return Err(ScoreError::NotClosed { date });
        }
        let ids: Vec<String> = view.members.iter().map(|m| m.kol.clone()).collect();
        let names: HashMap<String, String> = self
            .registry
            .pick(person)
            .list_kols(person, &ids)
            .await
            .map_err(internal)?
            .into_iter()
            .map(|k| (k.id, k.name))
            .collect();
        let empty = Metrics::default();
        let mut entries: Vec<Entry> = view
            .members
            .iter()
            .map(|m| {
                let metrics = m.row.as_ref().map_or(&empty, |r| &r.metrics);
                Entry {
                    kol: &m.kol,
                    name: names.get(&m.kol).unwrap_or(&m.kol),
                    d30: metrics.d30,
                    card: formula::score(metrics),
                }
            })
            .collect();
        entries.sort_by(rank_order);

        let mut tx = self.pool.begin().await.map_err(internal)?;
        sqlx::query("DELETE FROM score.ranking WHERE date = $1")
            .bind(date)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        sqlx::query("INSERT INTO score.ranking (date, ranked_at) VALUES ($1, now())")
            .bind(date)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        for (i, e) in entries.iter().enumerate() {
            sqlx::query(
                "INSERT INTO score.result (date, kol, rank, score, tier, parts, badges) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(date)
            .bind(e.kol)
            .bind(i as i32 + 1)
            .bind(i16::from(e.card.score))
            .bind(tier_str(e.card.tier))
            .bind(Json(&e.card.parts))
            .bind(Json(&e.card.badges))
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        }
        tx.commit().await.map_err(internal)
    }

    async fn ranking(&self, _: &Person, date: NaiveDate) -> Result<Ranking, ScoreError> {
        let ranked_at: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT ranked_at FROM score.ranking WHERE date = $1")
                .bind(date)
                .fetch_optional(&self.pool)
                .await
                .map_err(internal)?;
        let ranked_at = ranked_at.ok_or(ScoreError::NotRanked { date })?;
        let records: Vec<ResultRecord> = sqlx::query_as(
            "SELECT kol, rank, score, tier, parts, badges FROM score.result \
             WHERE date = $1 ORDER BY rank",
        )
        .bind(date)
        .fetch_all(&self.pool)
        .await
        .map_err(internal)?;
        let rows = records
            .into_iter()
            .map(|r| {
                Ok(RankedKol {
                    kol: r.kol,
                    rank: r.rank as u32,
                    card: ScoreCard {
                        score: r.score as u8,
                        tier: tier_of(&r.tier)?,
                        parts: r.parts.0,
                        badges: r.badges.0,
                    },
                })
            })
            .collect::<Result<_, ScoreError>>()?;
        Ok(Ranking {
            date,
            ranked_at,
            rows,
        })
    }
}
