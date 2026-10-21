//! Backend of the `board` plugin (layer 1): the client's ranking and KOL card.
//!
//! - `GET /api/board?date=`: rows, hero stats, source dates and ticker items ([`dto::Board`]).
//! - `GET /api/kol/{id}?date=`: one KOL's card with the Score breakdown and every published
//!   wallet with its Solscan link ([`dto::KolCard`]).
//!
//! Both answer only public DTO types. Ids are checked against the registry; dates must be
//! `YYYY-MM-DD` and a closed snapshot. Data fetched more than 48 hours ago is marked stale.
//! Declares the API slot `board/in_network` (default: not a member).

use api_board::InNetwork;
use api_registry::{KolProfile, RegistryApi, RegistryError};
use api_score::{ScoreApi, ScoreError};
use api_snapshots::{MemberReason, MetricSource, SnapshotsApi, SnapshotsError};
use arena_platform::{Api, Ctx, Person, Plugin, PluginEntry, Route, Slot, StartError, Tag};
use async_trait::async_trait;
use axum::{
    Json,
    extract::Query,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::migrate::Migrator;
use std::{collections::HashMap, sync::Arc};

pub mod dto;

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "board";

/// Migrations from `plugins/board/migrations`, run in schema `board`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// Data older than this is «data stale».
pub const STALE_AFTER_HOURS: i64 = 48;

/// Longest KOL id accepted in a path.
pub const MAX_ID_LEN: usize = 64;

/// Whether a row of `date`'s board fetched at `fetched_at` is stale: measured at the end
/// of that day, or at `now` while the day runs, so a past day keeps the marks it had.
pub fn is_stale(date: NaiveDate, fetched_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let day_end = (date + Duration::days(1))
        .and_time(NaiveTime::MIN)
        .and_utc();
    now.min(day_end) - fetched_at > Duration::hours(STALE_AFTER_HOURS)
}

/// Strict `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// Solscan account link for a published wallet.
pub fn solscan_url(address: &str) -> String {
    format!("https://solscan.io/account/{address}")
}

/// Base58, 32 to 44 characters: a Solana address.
fn is_address(s: &str) -> bool {
    (32..=44).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() && !matches!(b, b'0' | b'O' | b'I' | b'l'))
}

/// A handle safe to build a link from: 1 to 64 of `A-Z a-z 0-9 _ . -`.
fn handle(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(|h| h.trim_start_matches('@'))
        .filter(|h| {
            !h.is_empty()
                && h.len() <= 64
                && h.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        })
        .map(String::from)
}

struct Uses {
    registry: Api<dyn RegistryApi>,
    snapshots: Api<dyn SnapshotsApi>,
    score: Api<dyn ScoreApi>,
}

/// The board: reads registry, snapshots and score; asks the `in_network` slot.
pub struct Board {
    uses: Uses,
    in_network: Arc<Slot<dyn InNetwork>>,
}

pub struct BoardPlugin {
    board: Arc<Board>,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let uses = Uses {
        registry: ctx.apis.require::<dyn RegistryApi>(NAME)?,
        snapshots: ctx.apis.require::<dyn SnapshotsApi>(NAME)?,
        score: ctx.apis.require::<dyn ScoreApi>(NAME)?,
    };
    let in_network = ctx.slots.declare::<dyn InNetwork>(
        NAME,
        api_board::IN_NETWORK_SLOT,
        Arc::new(api_board::NotInNetwork),
    )?;
    Ok(Arc::new(BoardPlugin {
        board: Arc::new(Board { uses, in_network }),
    }))
}

/// A failed board request: status and a short public message.
#[derive(Debug)]
pub struct BoardError(pub StatusCode, pub String);

impl IntoResponse for BoardError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

fn unavailable() -> BoardError {
    BoardError(
        StatusCode::SERVICE_UNAVAILABLE,
        "board is unavailable".into(),
    )
}

fn internal() -> BoardError {
    BoardError(StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
}

fn no_snapshot() -> BoardError {
    BoardError(StatusCode::NOT_FOUND, "no snapshot for this date".into())
}

impl From<SnapshotsError> for BoardError {
    fn from(e: SnapshotsError) -> Self {
        match e {
            SnapshotsError::Off | SnapshotsError::NotBuilt => unavailable(),
            SnapshotsError::NoSnapshot { .. } => no_snapshot(),
            _ => internal(),
        }
    }
}

impl From<ScoreError> for BoardError {
    fn from(e: ScoreError) -> Self {
        match e {
            ScoreError::Off | ScoreError::NotBuilt => unavailable(),
            ScoreError::NoSnapshot { .. } | ScoreError::NotRanked { .. } => no_snapshot(),
            _ => internal(),
        }
    }
}

impl From<RegistryError> for BoardError {
    fn from(e: RegistryError) -> Self {
        match e {
            RegistryError::Off | RegistryError::NotBuilt => unavailable(),
            _ => internal(),
        }
    }
}

/// Query of both routes.
#[derive(Debug, Default, Deserialize)]
pub struct DateQuery {
    /// `YYYY-MM-DD`; absent = latest closed snapshot.
    pub date: Option<String>,
}

impl Board {
    /// The closed and ranked snapshot date for `raw` (latest when absent), and every such date.
    async fn date(
        &self,
        person: &Person,
        raw: Option<&str>,
    ) -> Result<(NaiveDate, Vec<NaiveDate>), BoardError> {
        // Close and rank are two steps of the daily run: a closed date shows once it is ranked.
        let ranked = self.uses.score.pick(person).ranked_dates(person).await?;
        let mut dates = self.uses.snapshots.pick(person).dates(person).await?;
        dates.retain(|d| ranked.contains(d));
        let date = match raw {
            None | Some("") => *dates.last().ok_or_else(no_snapshot)?,
            Some(s) => {
                let d = parse_date(s).ok_or_else(|| {
                    BoardError(StatusCode::BAD_REQUEST, "date must be YYYY-MM-DD".into())
                })?;
                if !dates.contains(&d) {
                    return Err(no_snapshot());
                }
                d
            }
        };
        Ok((date, dates))
    }

    /// `GET /api/board`.
    pub async fn board(
        &self,
        person: &Person,
        raw_date: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<dto::Board, BoardError> {
        let (date, dates) = self.date(person, raw_date).await?;
        let view = self
            .uses
            .snapshots
            .pick(person)
            .snapshot(person, date)
            .await?;
        let ranking = self.uses.score.pick(person).ranking(person, date).await?;
        let ids: Vec<String> = ranking.rows.iter().map(|r| r.kol.clone()).collect();
        let profiles = self
            .uses
            .registry
            .pick(person)
            .list_kols(person, &ids)
            .await?;
        let profiles: HashMap<&str, &KolProfile> =
            profiles.iter().map(|p| (p.id.as_str(), p)).collect();
        let members: HashMap<&str, _> = view.members.iter().map(|m| (m.kol.as_str(), m)).collect();
        let slot = self.in_network.get();

        let mut rows = Vec::with_capacity(ranking.rows.len());
        for r in &ranking.rows {
            let member = members.get(r.kol.as_str());
            let row = member.and_then(|m| m.row.as_ref());
            let profile = profiles.get(r.kol.as_str());
            rows.push(dto::BoardRow {
                kol: r.kol.clone(),
                name: profile.map_or_else(|| r.kol.clone(), |p| p.name.clone()),
                x_handle: profile.and_then(|p| handle(&p.x_handle)),
                rank: r.rank,
                score: r.card.score,
                tier: r.card.tier,
                badges: r.card.badges.clone(),
                top100: member.is_some_and(|m| m.reasons.contains(&MemberReason::Top100)),
                in_network: slot.is_member(person, &r.kol).await,
                source: row.map(|x| x.source),
                fetched_at: row.map(|x| x.fetched_at),
                stale: row.is_some_and(|x| is_stale(date, x.fetched_at, now)),
                metrics: row.map(|x| (&x.metrics).into()),
            });
        }

        let d30 = |r: &dto::BoardRow| r.metrics.as_ref().and_then(|m| m.d30);
        let stats = dto::Stats {
            tracked: rows.len() as u32,
            up_30d: rows
                .iter()
                .filter(|r| d30(r).is_some_and(|v| v > 0.0))
                .count() as u32,
            traded_last_hour: rows
                .iter()
                .filter(|r| {
                    r.metrics
                        .as_ref()
                        .and_then(|m| m.last_min_at_fetch)
                        .is_some_and(|v| v <= 60)
                })
                .count() as u32,
            total_d30: rows.iter().filter_map(d30).sum(),
        };

        let mut sources: Vec<dto::SourceDate> = Vec::new();
        for r in &rows {
            let (Some(source), Some(at)) = (r.source, r.fetched_at) else {
                continue;
            };
            match sources.iter_mut().find(|s| s.source == source) {
                Some(s) => {
                    s.kols += 1;
                    s.fetched_at = s.fetched_at.max(at);
                }
                None => sources.push(dto::SourceDate {
                    source,
                    fetched_at: at,
                    kols: 1,
                }),
            }
        }
        sources.sort_by_key(|s| match s.source {
            MetricSource::Kolexplorer => 0,
            MetricSource::Solanatracker => 1,
        });

        let mut ticker = Vec::new();
        if let Some(top) = rows.first() {
            ticker.push(dto::TickerItem {
                kind: dto::TickerKind::TopScore,
                kol: Some(top.kol.clone()),
                name: Some(top.name.clone()),
                value: f64::from(top.score),
            });
        }
        let best_week = rows
            .iter()
            .filter_map(|r| r.metrics.as_ref().and_then(|m| m.d7).map(|v| (r, v)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((r, v)) = best_week {
            ticker.push(dto::TickerItem {
                kind: dto::TickerKind::BestWeek,
                kol: Some(r.kol.clone()),
                name: Some(r.name.clone()),
                value: v,
            });
        }
        ticker.push(dto::TickerItem {
            kind: dto::TickerKind::TradedLastHour,
            kol: None,
            name: None,
            value: f64::from(stats.traded_last_hour),
        });
        ticker.push(dto::TickerItem {
            kind: dto::TickerKind::Up30d,
            kol: None,
            name: None,
            value: f64::from(stats.up_30d),
        });

        Ok(dto::Board {
            date,
            closed_at: view.snapshot.closed_at,
            dates,
            stats,
            sources,
            ticker,
            rows,
        })
    }

    /// `GET /api/kol/{id}`.
    pub async fn card(
        &self,
        person: &Person,
        id: &str,
        raw_date: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<dto::KolCard, BoardError> {
        let unknown = || BoardError(StatusCode::NOT_FOUND, "unknown KOL".into());
        if id.is_empty() || id.len() > MAX_ID_LEN {
            return Err(unknown());
        }
        let profile = self
            .uses
            .registry
            .pick(person)
            .get_kol(person, id)
            .await?
            .ok_or_else(unknown)?;
        let (date, _) = self.date(person, raw_date).await?;
        let ranking = self.uses.score.pick(person).ranking(person, date).await?;
        let ranked = ranking
            .rows
            .iter()
            .find(|r| r.kol == profile.id)
            .ok_or_else(|| {
                BoardError(
                    StatusCode::NOT_FOUND,
                    "not on the board on this date".into(),
                )
            })?;
        let view = self
            .uses
            .snapshots
            .pick(person)
            .snapshot(person, date)
            .await?;
        let member = view.members.iter().find(|m| m.kol == profile.id);
        let row = member.and_then(|m| m.row.as_ref());

        let mut wallets = Vec::new();
        let main = profile.main_wallet.iter().map(|w| (w, true));
        let others = profile.other_wallets.iter().map(|w| (w, false));
        for (w, is_main) in main.chain(others) {
            if is_address(&w.address) {
                wallets.push(dto::Wallet {
                    address: w.address.clone(),
                    main: is_main,
                    solscan_url: solscan_url(&w.address),
                });
            }
        }

        Ok(dto::KolCard {
            kol: profile.id.clone(),
            name: profile.name.clone(),
            x_handle: handle(&profile.x_handle),
            date,
            rank: ranked.rank,
            ranked: ranking.rows.len() as u32,
            top100: member.is_some_and(|m| m.reasons.contains(&MemberReason::Top100)),
            in_network: self.in_network.get().is_member(person, &profile.id).await,
            score: ranked.card.score,
            tier: ranked.card.tier,
            parts: ranked
                .card
                .parts
                .iter()
                .map(|p| dto::PartDto {
                    kind: p.kind,
                    raw: p.raw,
                    all_time: p.all_time,
                    share: p.share,
                    points: p.points,
                    status: p.status,
                })
                .collect(),
            badges: ranked.card.badges.clone(),
            source: row.map(|x| x.source),
            fetched_at: row.map(|x| x.fetched_at),
            stale: row.is_some_and(|x| is_stale(date, x.fetched_at, now)),
            metrics: row.map(|x| (&x.metrics).into()),
            formats: profile.formats.clone(),
            links: dto::Links {
                kolexplorer: handle(&profile.links.kolexplorer),
                solanatracker: profile.links.solanatracker,
                kolscan: profile.links.kolscan,
                fomo: handle(&profile.links.fomo),
                pumpfun: handle(&profile.links.pumpfun),
                telegram: handle(&profile.links.telegram),
            },
            wallets,
        })
    }
}

fn respond<T: serde::Serialize>(r: Result<T, BoardError>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => e.into_response(),
    }
}

#[async_trait]
impl Plugin for BoardPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        let board = self.board.clone();
        let card = self.board.clone();
        vec![
            Route::new(
                Tag::Public,
                "/api/board",
                get(
                    move |person: Person, Query(q): Query<DateQuery>| async move {
                        respond(board.board(&person, q.date.as_deref(), Utc::now()).await)
                    },
                ),
            ),
            Route::new(
                Tag::Public,
                "/api/kol/{id}",
                // The id is read from the URI so the handler also runs without a router.
                get(
                    move |person: Person, uri: Uri, Query(q): Query<DateQuery>| async move {
                        let id = uri.path().rsplit('/').next().unwrap_or("");
                        respond(card.card(&person, id, q.date.as_deref(), Utc::now()).await)
                    },
                ),
            ),
        ]
    }
}
