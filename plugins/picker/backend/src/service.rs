//! The picker: network KOLs with Score, tier and rates, filtered for a client brief, and
//! the two exports. Rates come only from the roster API, and only into the staff list and
//! the export with rates.

use crate::csv;
use api_registry::{KolProfile, RegistryApi};
use api_roster::{FORMATS, NetworkMember, Rates, RosterApi, RosterError};
use api_score::{RankedKol, ScoreApi, ScoreError, Tier};
use arena_platform::{Api, Person, Tag};
use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Without a date, the latest ranking at most this many days back.
pub const LOOKBACK_DAYS: u64 = 14;

/// Filters from the screen. Every one is optional; a set filter leaves out KOLs without
/// the value.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Filters {
    pub score_min: Option<String>,
    pub score_max: Option<String>,
    pub tier: Option<String>,
    pub rate_min: Option<String>,
    pub rate_max: Option<String>,
    pub format: Option<String>,
    pub date: Option<String>,
}

/// Public part of a row: what a client may see.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PublicRow {
    pub kol: String,
    pub name: String,
    pub x_handle: Option<String>,
    pub fomo: Option<String>,
    pub x_followers: Option<i64>,
    pub score: Option<u8>,
    pub tier: Option<Tier>,
    pub rank: Option<u32>,
    pub formats: Vec<String>,
}

/// A row of the staff list: the public part and the rates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    #[serde(flatten)]
    pub public: PublicRow,
    pub low_usd: Option<f64>,
    pub high_usd: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PickList {
    /// Date of the ranking used; `None` when there is none.
    pub date: Option<NaiveDate>,
    /// Why there is no Score, when there is none.
    pub no_ranking: Option<String>,
    /// Network members before the filters.
    pub total: usize,
    pub rows: Vec<Row>,
    /// Formats the filter knows.
    pub formats: Vec<String>,
}

/// A file to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    pub filename: String,
    pub csv: String,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unavailable(String),
}

pub struct Picker {
    pub(crate) roster: Api<dyn RosterApi>,
    pub(crate) score: Api<dyn ScoreApi>,
    pub(crate) registry: Api<dyn RegistryApi>,
}

fn number(name: &str, v: &Option<String>) -> Result<Option<f64>, Error> {
    match v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => s
            .replace([',', '$', ' '], "")
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .map(Some)
            .ok_or_else(|| Error::Invalid(format!("{name}: not a number"))),
    }
}

fn tier(v: &Option<String>) -> Result<Option<Tier>, Error> {
    match v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some("goat") => Ok(Some(Tier::Goat)),
        Some("diamond") => Ok(Some(Tier::Diamond)),
        Some("sendoor") => Ok(Some(Tier::Sendoor)),
        Some("degen") => Ok(Some(Tier::Degen)),
        Some(other) => Err(Error::Invalid(format!("unknown tier {other}"))),
    }
}

fn date(v: &Option<String>) -> Result<Option<NaiveDate>, Error> {
    match v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| Error::Invalid(format!("date: {s} is not YYYY-MM-DD"))),
    }
}

/// Parsed filters.
struct Want {
    score: (Option<f64>, Option<f64>),
    tier: Option<Tier>,
    rate: (Option<f64>, Option<f64>),
    format: Option<String>,
}

impl Want {
    fn parse(f: &Filters) -> Result<Want, Error> {
        let format = f
            .format
            .as_deref()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        if let Some(fm) = &format
            && !FORMATS.contains(&fm.as_str())
        {
            return Err(Error::Invalid(format!("unknown format {fm}")));
        }
        Ok(Want {
            score: (
                number("score from", &f.score_min)?,
                number("score to", &f.score_max)?,
            ),
            tier: tier(&f.tier)?,
            rate: (
                number("rate from", &f.rate_min)?,
                number("rate to", &f.rate_max)?,
            ),
            format,
        })
    }

    fn keeps(&self, r: &Row) -> bool {
        let p = &r.public;
        if self.score.0.is_some() || self.score.1.is_some() {
            let Some(s) = p.score.map(f64::from) else {
                return false;
            };
            if self.score.0.is_some_and(|min| s < min) || self.score.1.is_some_and(|max| s > max) {
                return false;
            }
        }
        if let Some(t) = self.tier
            && p.tier != Some(t)
        {
            return false;
        }
        if self.rate.0.is_some() || self.rate.1.is_some() {
            // the KOL's range [low, high] must overlap the wanted one
            let (low, high) = match (r.low_usd, r.high_usd) {
                (None, None) => return false,
                (Some(l), None) => (l, l),
                (None, Some(h)) => (h, h),
                (Some(l), Some(h)) => (l.min(h), l.max(h)),
            };
            if self.rate.0.is_some_and(|min| high < min) || self.rate.1.is_some_and(|max| low > max)
            {
                return false;
            }
        }
        if let Some(f) = &self.format
            && !p.formats.contains(f)
        {
            return false;
        }
        true
    }
}

fn roster_err(e: RosterError) -> Error {
    match e {
        RosterError::Forbidden => Error::Forbidden,
        other => Error::Unavailable(other.to_string()),
    }
}

/// Formats of the member and the public profile, in the roster's order.
fn formats(m: &NetworkMember, p: Option<&KolProfile>) -> Vec<String> {
    FORMATS
        .iter()
        .filter(|f| {
            m.formats.iter().any(|x| x == *f)
                || p.is_some_and(|p| p.formats.iter().any(|x| x == *f))
        })
        .map(|f| f.to_string())
        .collect()
}

fn public_row(m: &NetworkMember, p: Option<&KolProfile>, ranked: Option<&RankedKol>) -> PublicRow {
    PublicRow {
        kol: m.kol.clone(),
        name: p.map(|p| p.name.clone()).unwrap_or_else(|| m.name.clone()),
        x_handle: p
            .and_then(|p| p.x_handle.clone())
            .or_else(|| m.x_handle.clone()),
        fomo: p
            .and_then(|p| p.links.fomo.clone())
            .or_else(|| m.fomo_handle.clone()),
        x_followers: p
            .and_then(|p| p.reach.as_ref())
            .and_then(|r| r.reach.x_followers),
        score: ranked.map(|r| r.card.score),
        tier: ranked.map(|r| r.card.tier),
        rank: ranked.map(|r| r.rank),
        formats: formats(m, p),
    }
}

fn tier_name(t: Tier) -> &'static str {
    match t {
        Tier::Goat => "GOAT",
        Tier::Diamond => "Diamond",
        Tier::Sendoor => "Sendoor",
        Tier::Degen => "Degen",
    }
}

const NONE: &str = "—";

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_else(|| NONE.to_string())
}

/// Client columns: public fields only.
const CLIENT_HEADER: [&str; 9] = [
    "Name",
    "X",
    "FOMO",
    "X followers",
    "Score",
    "Tier",
    "Rank",
    "Formats",
    "Snapshot",
];

fn client_cells(p: &PublicRow, date: Option<NaiveDate>) -> Vec<String> {
    vec![
        p.name.clone(),
        opt(p.x_handle.as_ref().map(|h| format!("https://x.com/{h}"))),
        opt(p.fomo.clone()),
        opt(p.x_followers),
        opt(p.score),
        opt(p.tier.map(tier_name)),
        opt(p.rank),
        if p.formats.is_empty() {
            NONE.to_string()
        } else {
            p.formats.join(" ")
        },
        opt(date),
    ]
}

impl Picker {
    /// The ranking for `date`, or the latest one in the last [`LOOKBACK_DAYS`] days.
    async fn ranking(
        &self,
        person: &Person,
        date: Option<NaiveDate>,
        today: NaiveDate,
    ) -> Result<
        (
            Option<NaiveDate>,
            BTreeMap<String, RankedKol>,
            Option<String>,
        ),
        Error,
    > {
        let score = self.score.pick(person);
        let days: Vec<NaiveDate> = match date {
            Some(d) => vec![d],
            None => (0..=LOOKBACK_DAYS)
                .filter_map(|n| today.checked_sub_days(Days::new(n)))
                .collect(),
        };
        let mut why = None;
        for d in days {
            match score.ranking(person, d).await {
                Ok(r) => {
                    let rows = r.rows.into_iter().map(|k| (k.kol.clone(), k)).collect();
                    return Ok((Some(r.date), rows, None));
                }
                Err(ScoreError::Internal { message }) => {
                    return Err(Error::Unavailable(format!("score: {message}")));
                }
                Err(e @ (ScoreError::Off | ScoreError::NotBuilt)) => {
                    return Ok((None, BTreeMap::new(), Some(e.to_string())));
                }
                Err(e) => why = Some(e.to_string()),
            }
        }
        let why = match date {
            Some(_) => why,
            None => Some(format!("no ranking in the last {LOOKBACK_DAYS} days")),
        };
        Ok((None, BTreeMap::new(), why))
    }

    /// Members with profile and Score, ranked first, then by name.
    async fn rows(
        &self,
        person: &Person,
        date: Option<NaiveDate>,
        today: NaiveDate,
        with_rates: bool,
    ) -> Result<(Option<NaiveDate>, Option<String>, Vec<Row>), Error> {
        let members = self
            .roster
            .pick(person)
            .members(person)
            .await
            .map_err(roster_err)?;
        let ids: Vec<String> = members.iter().map(|m| m.kol.clone()).collect();
        let profiles: BTreeMap<String, KolProfile> = self
            .registry
            .pick(person)
            .list_kols(person, &ids)
            .await
            .map_err(|e| Error::Unavailable(e.to_string()))?
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();
        let (date, ranked, why) = self.ranking(person, date, today).await?;
        let rates: BTreeMap<String, Rates> = if with_rates && !ids.is_empty() {
            self.roster
                .pick(person)
                .rates(person, &ids)
                .await
                .map_err(roster_err)?
                .into_iter()
                .map(|r| (r.kol.clone(), r))
                .collect()
        } else {
            BTreeMap::new()
        };
        let mut rows: Vec<Row> = members
            .iter()
            .map(|m| {
                let rate = rates.get(&m.kol);
                Row {
                    public: public_row(m, profiles.get(&m.kol), ranked.get(&m.kol)),
                    low_usd: rate.and_then(|r| r.low_usd),
                    high_usd: rate.and_then(|r| r.high_usd),
                }
            })
            .collect();
        rows.sort_by(|a, b| {
            let key = |r: &Row| {
                (
                    r.public.rank.unwrap_or(u32::MAX),
                    r.public.name.to_lowercase(),
                )
            };
            key(a).cmp(&key(b))
        });
        Ok((date, why, rows))
    }

    /// The staff list after the filters.
    pub async fn list(
        &self,
        person: &Person,
        filters: &Filters,
        today: NaiveDate,
    ) -> Result<PickList, Error> {
        if !Tag::Staff.admits(person) {
            return Err(Error::Forbidden);
        }
        let want = Want::parse(filters)?;
        let (date, why, rows) = self.rows(person, date(&filters.date)?, today, true).await?;
        let total = rows.len();
        Ok(PickList {
            date,
            no_ranking: why,
            total,
            rows: rows.into_iter().filter(|r| want.keeps(r)).collect(),
            formats: FORMATS.iter().map(|f| f.to_string()).collect(),
        })
    }

    /// The checked KOLs as CSV, in list order. Without `with_rates` the file holds public
    /// fields only, and the roster's rates are not even read.
    pub async fn export(
        &self,
        person: &Person,
        kols: &[String],
        with_rates: bool,
        date_text: &Option<String>,
        today: NaiveDate,
    ) -> Result<Export, Error> {
        if !Tag::Staff.admits(person) {
            return Err(Error::Forbidden);
        }
        let wanted: Vec<&str> = kols
            .iter()
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
            .collect();
        if wanted.is_empty() {
            return Err(Error::Invalid("check at least one KOL".into()));
        }
        let (date, _, rows) = self
            .rows(person, date(date_text)?, today, with_rates)
            .await?;
        let rows: Vec<&Row> = rows
            .iter()
            .filter(|r| wanted.contains(&r.public.kol.as_str()))
            .collect();
        if rows.is_empty() {
            return Err(Error::Invalid(
                "none of these KOLs is in the network".into(),
            ));
        }
        let stamp = date.unwrap_or(today);
        if with_rates {
            let mut header: Vec<&str> = CLIENT_HEADER.to_vec();
            header.extend(["Low $", "High $"]);
            let cells: Vec<Vec<String>> = rows
                .iter()
                .map(|r| {
                    let mut c = client_cells(&r.public, date);
                    c.push(opt(r.low_usd));
                    c.push(opt(r.high_usd));
                    c
                })
                .collect();
            Ok(Export {
                filename: format!("picker-with-rates-{stamp}.csv"),
                csv: csv::table(&header, &cells),
            })
        } else {
            let cells: Vec<Vec<String>> =
                rows.iter().map(|r| client_cells(&r.public, date)).collect();
            Ok(Export {
                filename: format!("picker-{stamp}.csv"),
                csv: csv::table(&CLIENT_HEADER, &cells),
            })
        }
    }
}
