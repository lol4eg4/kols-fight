//! Rows of `wallets.candidate`.

use crate::{
    error::Error,
    machine::{parse_source, parse_status},
};
use api_wallets::{Candidate, CandidateSource, CandidateStatus};
use chrono::{DateTime, Utc};

pub(crate) const COLS: &str = "id, kol, address, status, source, evidence, sent_by, confirmation, reason, \
     main, last_trade_at, checked_at, created_at, updated_at";

#[derive(sqlx::FromRow)]
pub(crate) struct Raw {
    id: i64,
    kol: String,
    address: String,
    status: String,
    source: String,
    evidence: Option<String>,
    sent_by: Option<String>,
    confirmation: Option<String>,
    reason: Option<String>,
    main: bool,
    last_trade_at: Option<DateTime<Utc>>,
    checked_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub id: i64,
    pub kol: String,
    pub address: String,
    pub status: CandidateStatus,
    pub source: CandidateSource,
    pub evidence: Option<String>,
    pub sent_by: Option<String>,
    pub confirmation: Option<String>,
    pub reason: Option<String>,
    pub main: bool,
    pub last_trade_at: Option<DateTime<Utc>>,
    pub checked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<Raw> for Row {
    type Error = Error;

    fn try_from(r: Raw) -> Result<Self, Error> {
        Ok(Row {
            status: parse_status(&r.status).ok_or_else(|| {
                Error::Internal(format!("candidate {}: unknown status {}", r.id, r.status))
            })?,
            source: parse_source(&r.source).ok_or_else(|| {
                Error::Internal(format!("candidate {}: unknown source {}", r.id, r.source))
            })?,
            id: r.id,
            kol: r.kol,
            address: r.address,
            evidence: r.evidence,
            sent_by: r.sent_by,
            confirmation: r.confirmation,
            reason: r.reason,
            main: r.main,
            last_trade_at: r.last_trade_at,
            checked_at: r.checked_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

impl Row {
    pub fn candidate(&self) -> Candidate {
        Candidate {
            id: self.id,
            kol: self.kol.clone(),
            address: self.address.clone(),
            status: self.status,
            source: self.source,
            evidence: self.evidence.clone(),
            sent_by: self.sent_by.clone(),
            confirmation: self.confirmation.clone(),
            reason: self.reason.clone(),
            main: self.main,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub(crate) fn rows(raw: Vec<Raw>) -> Result<Vec<Row>, Error> {
    raw.into_iter().map(Row::try_from).collect()
}
