//! Backend of the `fight` plugin (layer 1): KOL Fight on one snapshot.
//!
//! - [`resolve::resolve`]: the eight rounds, a pure function of the fighters' cards and rows.
//! - [`Fight`]: [`FightApi`] over the snapshots, score and registry APIs.
//! - Routes: `GET /api/fight?ids=&date=` (JSON [`api_fight::FightResult`]) and
//!   `GET /api/fight/image?ids=&date=` (1200x630 PNG, see [`image`]).
//! - Page meta for `/fight`: title, `og:` and `twitter:` tags with the image link.

use api_fight::{FightApi, FightError, FightResult, KolId, MAX_FIGHTERS, MIN_FIGHTERS};
use api_registry::{RegistryApi, RegistryError};
use api_score::{ScoreApi, ScoreError};
use api_snapshots::{SnapshotsApi, SnapshotsError};
use arena_platform::{
    Api, Ctx, MetaTag, PageMeta, Person, Plugin, PluginEntry, Route, StartError, Tag,
};
use async_trait::async_trait;
use axum::{
    Json,
    extract::Query,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;
use sqlx::migrate::Migrator;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub mod avatar;
pub mod image;
pub mod resolve;

pub use resolve::{FighterInput, resolve};

/// Plugin name: folder, schema and crate suffix.
pub const NAME: &str = "fight";

/// Migrations from `plugins/fight/migrations`, run in schema `fight`.
pub static MIGRATOR: Migrator = sqlx::migrate!("../migrations");

/// What the server lists: manifest and constructor.
pub const ENTRY: PluginEntry = PluginEntry {
    manifest: include_str!("../../plugin.toml"),
    build,
};

/// Longest KOL id accepted in a link.
pub const MAX_ID_LEN: usize = 64;

pub struct FightPlugin {
    /// Own API through the registry, so `off` rules apply to routes and page meta too.
    api: Api<dyn FightApi>,
    images: Arc<Mutex<image::Cache>>,
}

fn build(ctx: &Ctx) -> Result<Arc<dyn Plugin>, StartError> {
    let fight = Arc::new(Fight {
        snapshots: ctx.apis.require::<dyn SnapshotsApi>(NAME)?,
        score: ctx.apis.require::<dyn ScoreApi>(NAME)?,
        registry: ctx.apis.require::<dyn RegistryApi>(NAME)?,
    });
    ctx.apis.provide::<dyn FightApi>(
        NAME,
        vec![("default", fight)],
        Arc::new(api_fight::stub::Stub),
    )?;
    Ok(Arc::new(FightPlugin {
        api: ctx.apis.require::<dyn FightApi>(NAME)?,
        images: Arc::new(Mutex::new(image::Cache::new(image::CACHE_SIZE))),
    }))
}

/// KOL Fight over the snapshots, score and registry APIs.
pub struct Fight {
    snapshots: Api<dyn SnapshotsApi>,
    score: Api<dyn ScoreApi>,
    registry: Api<dyn RegistryApi>,
}

fn from_snapshots(e: SnapshotsError) -> FightError {
    match e {
        SnapshotsError::Off => FightError::Off,
        SnapshotsError::NotBuilt => FightError::NotBuilt,
        SnapshotsError::NoSnapshot { date } => FightError::NoSnapshot { date: Some(date) },
        e => FightError::Internal {
            message: e.to_string(),
        },
    }
}

fn from_score(e: ScoreError) -> FightError {
    match e {
        ScoreError::Off => FightError::Off,
        ScoreError::NotBuilt => FightError::NotBuilt,
        ScoreError::NoSnapshot { date } => FightError::NoSnapshot { date: Some(date) },
        e => FightError::Internal {
            message: e.to_string(),
        },
    }
}

fn from_registry(e: RegistryError) -> FightError {
    match e {
        RegistryError::Off => FightError::Off,
        RegistryError::NotBuilt => FightError::NotBuilt,
        e => FightError::Internal {
            message: e.to_string(),
        },
    }
}

#[async_trait]
impl FightApi for Fight {
    async fn fight(
        &self,
        person: &Person,
        ids: &[KolId],
        date: Option<NaiveDate>,
    ) -> Result<FightResult, FightError> {
        if ids.len() > MAX_FIGHTERS {
            return Err(FightError::TooManyFighters { count: ids.len() });
        }
        for (i, id) in ids.iter().enumerate() {
            if ids[..i].contains(id) {
                return Err(FightError::DuplicateFighter { kol: id.clone() });
            }
        }
        let snapshots = self.snapshots.pick(person);
        // A closed date can fight once the daily run has ranked it.
        let ranked = self
            .score
            .pick(person)
            .ranked_dates(person)
            .await
            .map_err(from_score)?;
        let mut dates = snapshots.dates(person).await.map_err(from_snapshots)?;
        dates.retain(|d| ranked.contains(d));
        let date = match date {
            None => *dates.last().ok_or(FightError::NoSnapshot { date: None })?,
            Some(d) if dates.contains(&d) => d,
            Some(d) => return Err(FightError::NoSnapshot { date: Some(d) }),
        };
        let view = snapshots
            .snapshot(person, date)
            .await
            .map_err(from_snapshots)?;
        let ranking = self
            .score
            .pick(person)
            .ranking(person, date)
            .await
            .map_err(from_score)?;
        let profiles = self
            .registry
            .pick(person)
            .list_kols(person, ids)
            .await
            .map_err(from_registry)?;

        let ranked: HashMap<&str, _> = ranking.rows.iter().map(|r| (r.kol.as_str(), r)).collect();
        let rows: HashMap<&str, _> = view.members.iter().map(|m| (m.kol.as_str(), m)).collect();
        let names: HashMap<&str, &str> = profiles
            .iter()
            .map(|p| (p.id.as_str(), p.name.as_str()))
            .collect();
        let mut fighters = Vec::new();
        let mut missing = Vec::new();
        for id in ids {
            match (ranked.get(id.as_str()), rows.get(id.as_str())) {
                (Some(r), Some(m)) => fighters.push(FighterInput {
                    kol: id.clone(),
                    name: names.get(id.as_str()).unwrap_or(&id.as_str()).to_string(),
                    rank: Some(r.rank),
                    card: r.card.clone(),
                    row: m.row.clone(),
                }),
                _ => missing.push(id.clone()),
            }
        }
        if fighters.len() < MIN_FIGHTERS {
            return Err(FightError::TooFewFighters { missing });
        }
        let mut result = resolve(&fighters, date);
        result.snapshot_closed_at = view.snapshot.closed_at;
        result.missing = missing;
        Ok(result)
    }
}

/// Query of `/api/fight`, `/api/fight/image` and `/fight`.
#[derive(Debug, Default, Deserialize)]
pub struct FightQuery {
    /// Comma-separated KOL ids.
    pub ids: Option<String>,
    /// `YYYY-MM-DD`; absent = latest closed snapshot.
    pub date: Option<String>,
}

/// Strict `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// Ids and date from the query; `Err` is a 400 message.
fn parse_query(q: &FightQuery) -> Result<(Vec<KolId>, Option<NaiveDate>), String> {
    let ids: Vec<KolId> = q
        .ids
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if ids.iter().any(|id| id.len() > MAX_ID_LEN) {
        return Err("bad id".into());
    }
    let date = match q.date.as_deref() {
        None | Some("") => None,
        Some(s) => Some(parse_date(s).ok_or("date must be YYYY-MM-DD")?),
    };
    Ok((ids, date))
}

fn error(status: StatusCode, body: serde_json::Value) -> Response {
    (status, Json(body)).into_response()
}

fn fight_error(e: FightError) -> Response {
    let msg = e.to_string();
    match e {
        FightError::TooFewFighters { missing } => error(
            StatusCode::BAD_REQUEST,
            json!({"error": msg, "missing": missing}),
        ),
        FightError::TooManyFighters { .. } | FightError::DuplicateFighter { .. } => {
            error(StatusCode::BAD_REQUEST, json!({"error": msg}))
        }
        FightError::NoSnapshot { .. } => error(StatusCode::NOT_FOUND, json!({"error": msg})),
        FightError::Off | FightError::NotBuilt => {
            error(StatusCode::SERVICE_UNAVAILABLE, json!({"error": msg}))
        }
        FightError::Internal { .. } => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": "internal error"}),
        ),
    }
}

async fn run(
    api: &Api<dyn FightApi>,
    person: &Person,
    q: &FightQuery,
) -> Result<(Vec<KolId>, FightResult), Box<Response>> {
    let (ids, date) = parse_query(q)
        .map_err(|m| Box::new(error(StatusCode::BAD_REQUEST, json!({"error": m}))))?;
    let result = api
        .pick(person)
        .fight(person, &ids, date)
        .await
        .map_err(|e| Box::new(fight_error(e)))?;
    Ok((ids, result))
}

async fn fight_json(api: Api<dyn FightApi>, person: Person, q: FightQuery) -> Response {
    match run(&api, &person, &q).await {
        Ok((_, result)) => Json(result).into_response(),
        Err(r) => *r,
    }
}

async fn fight_image(
    api: Api<dyn FightApi>,
    images: Arc<Mutex<image::Cache>>,
    person: Person,
    q: FightQuery,
) -> Response {
    let (ids, result) = match run(&api, &person, &q).await {
        Ok(x) => x,
        Err(r) => return *r,
    };
    let key: image::Key = (ids, result.date, result.snapshot_closed_at);
    let cached = images.lock().expect("image cache").get(&key);
    let png = match cached {
        Some(png) => png,
        None => {
            let rendered = tokio::task::spawn_blocking(move || image::render(&result)).await;
            match rendered {
                Ok(Ok(png)) => {
                    let png = Arc::new(png);
                    images.lock().expect("image cache").put(key, png.clone());
                    png
                }
                _ => {
                    return error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        json!({"error": "internal error"}),
                    );
                }
            }
        }
    };
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        png.as_ref().clone(),
    )
        .into_response()
}

const SITE: &str = "KOL Arena";
const DEFAULT_TITLE: &str = "KOL Fight — KOL Arena";
const DEFAULT_DESCRIPTION: &str =
    "Pick 2–4 Solana KOLs and fight them round by round on real wallet results.";

/// Page meta for `/fight?ids=&date=`: the fight's title, outcome and image link pinned to the
/// snapshot date. A fight that cannot run gets the generic KOL Fight meta.
pub async fn fight_meta(api: &Api<dyn FightApi>, query: Option<&str>) -> PageMeta {
    let q: FightQuery = serde_urlencoded::from_str(query.unwrap_or("")).unwrap_or_default();
    let person = Person::NOBODY;
    let Ok((ids, result)) = run(api, &person, &q).await else {
        return PageMeta {
            title: Some(DEFAULT_TITLE.into()),
            tags: vec![
                MetaTag::property("og:site_name", SITE),
                MetaTag::property("og:type", "website"),
                MetaTag::property("og:title", DEFAULT_TITLE),
                MetaTag::property("og:description", DEFAULT_DESCRIPTION),
                MetaTag::name("twitter:card", "summary"),
                MetaTag::name("twitter:title", DEFAULT_TITLE),
                MetaTag::name("twitter:description", DEFAULT_DESCRIPTION),
            ],
        };
    };
    let names: Vec<&str> = result.fighters.iter().map(|f| f.name.as_str()).collect();
    let title = format!("{} — KOL Fight", names.join(" vs "));
    let description = format!(
        "{} {}. Snapshot {}.",
        result.summary.first().map(String::as_str).unwrap_or(""),
        result.counted_line,
        result.date
    );
    let image = format!(
        "/api/fight/image?ids={}&date={}",
        ids.join(","),
        result.date
    );
    PageMeta {
        title: Some(title.clone()),
        tags: vec![
            MetaTag::property("og:site_name", SITE),
            MetaTag::property("og:type", "website"),
            MetaTag::property("og:title", title.clone()),
            MetaTag::property("og:description", description.clone()),
            MetaTag::property("og:image", image.clone()),
            MetaTag::property("og:image:width", image::WIDTH.to_string()),
            MetaTag::property("og:image:height", image::HEIGHT.to_string()),
            MetaTag::name("twitter:card", "summary_large_image"),
            MetaTag::name("twitter:title", title),
            MetaTag::name("twitter:description", description),
            MetaTag::name("twitter:image", image),
        ],
    }
}

#[async_trait]
impl Plugin for FightPlugin {
    fn name(&self) -> &'static str {
        NAME
    }

    fn migrations(&self) -> &'static Migrator {
        &MIGRATOR
    }

    fn routes(&self, _ctx: &Ctx) -> Vec<Route> {
        let api = self.api.clone();
        let json_api = api.clone();
        let images = self.images.clone();
        vec![
            Route::new(
                Tag::Public,
                "/api/fight",
                get(move |person: Person, Query(q): Query<FightQuery>| {
                    fight_json(json_api.clone(), person, q)
                }),
            ),
            Route::new(
                Tag::Public,
                "/api/fight/image",
                get(move |person: Person, Query(q): Query<FightQuery>| {
                    fight_image(api.clone(), images.clone(), person, q)
                }),
            ),
        ]
    }

    async fn page_meta(&self, path: &str, query: Option<&str>) -> Option<PageMeta> {
        if path != "/fight" && path != "/fight/" {
            return None;
        }
        Some(fight_meta(&self.api, query).await)
    }
}
