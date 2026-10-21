//! The wallets service: finding candidates, moving them through the state machine,
//! journaling every change and publishing confirmed wallets to the registry.

use crate::{
    error::Error,
    machine::{allowed, kol_status, source_str, status_str},
    store::{COLS, Raw, Row, rows},
};
use api_journal::JournalApi;
use api_kolexplorer::KolexplorerApi;
use api_registry::{RegistryApi, RegistryError, WalletSource};
use api_roster::{NetworkMember, RosterApi, is_solana_address};
use api_solanatracker::SolanatrackerApi;
use api_wallets::{
    Candidate, CandidateSource, CandidateStatus, CandidateStatus::*, KolWalletStatus, WalletStatus,
};
use arena_platform::{
    Api, Person,
    scheduler::{DATA_LOCK, LockSpec, RunError, run_locked},
};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{collections::BTreeMap, future::Future, sync::Arc};

const SYSTEM: Person = Person::SYSTEM;
/// A confirmed main wallet's last trade is looked up again after this long.
const REFRESH_AFTER: Duration = Duration::hours(12);

pub struct Wallets {
    pub(crate) pool: PgPool,
    pub(crate) roster: Api<dyn RosterApi>,
    pub(crate) kolexplorer: Api<dyn KolexplorerApi>,
    pub(crate) solanatracker: Api<dyn SolanatrackerApi>,
    pub(crate) registry: Api<dyn RegistryApi>,
    pub(crate) journal: Api<dyn JournalApi>,
}

/// Result of a trades check at Solana Tracker.
enum Trades {
    Yes(Option<DateTime<Utc>>),
    No,
    Failed(String),
}

/// What a new candidate should be, before the conflict check.
struct Found {
    status: CandidateStatus,
    reason: Option<String>,
    last_trade_at: Option<DateTime<Utc>>,
    checked: bool,
}

/// One candidate as the Wallets screen shows it. Staff only.
#[derive(Serialize)]
pub struct CandidateView {
    #[serde(flatten)]
    pub candidate: Candidate,
    pub last_trade_at: Option<DateTime<Utc>>,
    pub checked_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct KolView {
    pub kol: String,
    pub name: String,
    pub status: KolWalletStatus,
    pub main_address: Option<String>,
    pub last_trade_at: Option<DateTime<Utc>>,
    pub candidates: Vec<CandidateView>,
}

#[derive(Serialize)]
pub struct Overview {
    /// KOLs per status: `confirmed`, `hypothesis`, `not_found`, `conflict`.
    pub counts: BTreeMap<&'static str, u32>,
    pub kols: Vec<KolView>,
}

fn kol_status_str(s: KolWalletStatus) -> &'static str {
    match s {
        KolWalletStatus::Confirmed => "confirmed",
        KolWalletStatus::Hypothesis => "hypothesis",
        KolWalletStatus::NotFound => "not_found",
        KolWalletStatus::Conflict => "conflict",
    }
}

fn who(person: &Person) -> String {
    match person.id {
        Some(id) => format!("{}:{id}", person.role.as_str()),
        None => person.role.as_str().to_string(),
    }
}

fn same_handle(a: &str, b: &str) -> bool {
    let a = a.trim().trim_start_matches('@');
    !a.is_empty() && a.eq_ignore_ascii_case(b.trim().trim_start_matches('@'))
}

impl Wallets {
    // ----- reads ---------------------------------------------------------------------------

    pub(crate) async fn rows_of(&self, kol: &str) -> Result<Vec<Row>, Error> {
        let sql =
            format!("SELECT {COLS} FROM wallets.candidate WHERE kol = $1 ORDER BY created_at, id");
        rows(
            sqlx::query_as::<_, Raw>(&sql)
                .bind(kol)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub(crate) async fn all_rows(&self) -> Result<Vec<Row>, Error> {
        let sql = format!("SELECT {COLS} FROM wallets.candidate ORDER BY created_at, id");
        rows(sqlx::query_as::<_, Raw>(&sql).fetch_all(&self.pool).await?)
    }

    pub(crate) async fn row(&self, id: i64) -> Result<Row, Error> {
        let sql = format!("SELECT {COLS} FROM wallets.candidate WHERE id = $1");
        let raw = sqlx::query_as::<_, Raw>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        raw.map(Row::try_from)
            .transpose()?
            .ok_or(Error::NoCandidate(id))
    }

    async fn row_by_address(&self, kol: &str, address: &str) -> Result<Option<Row>, Error> {
        let sql = format!("SELECT {COLS} FROM wallets.candidate WHERE kol = $1 AND address = $2");
        let raw = sqlx::query_as::<_, Raw>(&sql)
            .bind(kol)
            .bind(address)
            .fetch_optional(&self.pool)
            .await?;
        raw.map(Row::try_from).transpose()
    }

    async fn members(&self) -> Result<Vec<NetworkMember>, Error> {
        Ok(self.roster.pick(&SYSTEM).members(&SYSTEM).await?)
    }

    /// The staff view: counts per status and every candidate of every network KOL.
    pub async fn overview(&self, person: &Person) -> Result<Overview, Error> {
        if !person.is_staff() {
            return Err(Error::Forbidden);
        }
        let members = self.roster.pick(person).members(person).await?;
        let mut by_kol: BTreeMap<String, Vec<Row>> = BTreeMap::new();
        for r in self.all_rows().await? {
            by_kol.entry(r.kol.clone()).or_default().push(r);
        }
        let mut counts: BTreeMap<&'static str, u32> =
            ["confirmed", "hypothesis", "not_found", "conflict"]
                .into_iter()
                .map(|k| (k, 0))
                .collect();
        let mut kols = Vec::new();
        for m in members {
            let rows = by_kol.remove(&m.kol).unwrap_or_default();
            let status = kol_status(rows.iter().map(|r| r.status));
            *counts.entry(kol_status_str(status)).or_default() += 1;
            let main = rows
                .iter()
                .filter(|r| r.status == Confirmed)
                .find(|r| r.main)
                .or_else(|| rows.iter().find(|r| r.status == Confirmed));
            kols.push(KolView {
                kol: m.kol,
                name: m.name,
                status,
                main_address: main.map(|r| r.address.clone()),
                last_trade_at: main.and_then(|r| r.last_trade_at),
                candidates: rows
                    .iter()
                    .map(|r| CandidateView {
                        candidate: r.candidate(),
                        last_trade_at: r.last_trade_at,
                        checked_at: r.checked_at,
                    })
                    .collect(),
            });
        }
        Ok(Overview { counts, kols })
    }

    /// Public-safe status of one network KOL: only a confirmed main address is named.
    pub async fn status(&self, person: &Person, kol: &str) -> Result<WalletStatus, Error> {
        if !self.roster.pick(person).is_member(person, kol).await? {
            return Err(Error::NotMember(kol.to_string()));
        }
        let rows = self.rows_of(kol).await?;
        let main = rows
            .iter()
            .filter(|r| r.status == Confirmed)
            .find(|r| r.main)
            .or_else(|| rows.iter().find(|r| r.status == Confirmed));
        Ok(WalletStatus {
            kol: kol.to_string(),
            status: kol_status(rows.iter().map(|r| r.status)),
            main_address: main.map(|r| r.address.clone()),
            last_trade_at: main.and_then(|r| r.last_trade_at),
        })
    }

    // ----- locks ---------------------------------------------------------------------------

    /// Runs `fut` under the shared `arena_data` lock, trying once. The sheet read holds the
    /// lock itself while it emits «table read», so the event handler must not use this.
    pub(crate) async fn locked<T, F>(&self, fut: F) -> Result<T, Error>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, Error>> + Send + 'static,
    {
        match run_locked(&self.pool, vec![LockSpec::try_once(DATA_LOCK)], fut).await {
            Ok(r) => r,
            Err(RunError::Busy { .. }) => Err(Error::Busy),
            Err(e) => Err(Error::Internal(e.to_string())),
        }
    }

    // ----- writes: one change at a time, each journaled -----------------------------------

    async fn record(
        &self,
        person: &Person,
        kol: &str,
        action: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) -> Result<(), Error> {
        self.journal
            .pick(person)
            .record(person, "wallets", kol, action, before, after)
            .await
            .map_err(|e| Error::Internal(format!("saved, but the journal failed: {e}")))?;
        Ok(())
    }

    fn state(r: &Row) -> Value {
        json!({
            "id": r.id,
            "address": r.address,
            "status": status_str(r.status),
            "source": source_str(r.source),
            "main": r.main,
            "reason": r.reason,
            "confirmation": r.confirmation,
        })
    }

    /// Moves a candidate along an allowed edge of the state machine.
    pub(crate) async fn move_to(
        &self,
        person: &Person,
        row: &Row,
        to: CandidateStatus,
        reason: Option<String>,
        confirmation: Option<String>,
        action: &str,
    ) -> Result<Row, Error> {
        if !allowed(row.status, to) {
            return Err(Error::Refused(format!(
                "a {} wallet cannot become {}",
                status_str(row.status),
                status_str(to)
            )));
        }
        let sql = format!(
            "UPDATE wallets.candidate SET status = $3, reason = $4, \
             confirmation = COALESCE($5, confirmation), main = (main AND $3 = 'confirmed'), updated_at = now() \
             WHERE id = $1 AND status = $2 RETURNING {COLS}"
        );
        let raw = sqlx::query_as::<_, Raw>(&sql)
            .bind(row.id)
            .bind(status_str(row.status))
            .bind(status_str(to))
            .bind(&reason)
            .bind(&confirmation)
            .fetch_optional(&self.pool)
            .await?;
        let Some(raw) = raw else {
            return Err(Error::Refused(
                "the wallet changed meanwhile, reload".into(),
            ));
        };
        let after = Row::try_from(raw)?;
        self.record(
            person,
            &row.kol,
            action,
            Some(Self::state(row)),
            Some(Self::state(&after)),
        )
        .await?;
        Ok(after)
    }

    /// Adds a candidate. A confirmed address another KOL holds is added as `conflict`, and the
    /// other KOL's confirmed copy becomes `conflict` too.
    #[allow(clippy::too_many_arguments)]
    async fn insert(
        &self,
        person: &Person,
        kol: &str,
        address: &str,
        source: CandidateSource,
        evidence: Option<String>,
        found: Found,
        action: &str,
    ) -> Result<Row, Error> {
        let mut status = found.status;
        let mut reason = found.reason;
        let mut other = None;
        if status == Confirmed
            && let Some(owner) = self.claim(kol, address).await?
        {
            status = Conflict;
            reason = Some(format!("also claimed by {owner}"));
            other = Some(owner);
        }
        let sql = format!(
            "INSERT INTO wallets.candidate (kol, address, status, source, evidence, sent_by, reason, last_trade_at, checked_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CASE WHEN $9 THEN now() END) RETURNING {COLS}"
        );
        let raw = sqlx::query_as::<_, Raw>(&sql)
            .bind(kol)
            .bind(address)
            .bind(status_str(status))
            .bind(source_str(source))
            .bind(&evidence)
            .bind(who(person))
            .bind(&reason)
            .bind(found.last_trade_at)
            .bind(found.checked)
            .fetch_one(&self.pool)
            .await?;
        let row = Row::try_from(raw)?;
        self.record(person, kol, action, None, Some(Self::state(&row)))
            .await?;
        if let Some(owner) = other {
            self.conflict_other(person, &owner, address, kol).await?;
        }
        Ok(row)
    }

    /// Who else holds `address`: a confirmed or conflicting candidate of another KOL, or the
    /// registry's published owner.
    async fn claim(&self, kol: &str, address: &str) -> Result<Option<String>, Error> {
        let local: Option<String> = sqlx::query_scalar(
            "SELECT kol FROM wallets.candidate WHERE address = $1 AND kol <> $2 \
             AND status IN ('confirmed', 'conflict') ORDER BY id LIMIT 1",
        )
        .bind(address)
        .bind(kol)
        .fetch_optional(&self.pool)
        .await?;
        if local.is_some() {
            return Ok(local);
        }
        let owner = self
            .registry
            .pick(&SYSTEM)
            .find_by_wallet(&SYSTEM, address)
            .await?;
        Ok(owner.filter(|o| o != kol))
    }

    /// The confirmed copy of `address` at `other` becomes `conflict` and is taken off the
    /// registry until staff resolves it.
    async fn conflict_other(
        &self,
        person: &Person,
        other: &str,
        address: &str,
        this: &str,
    ) -> Result<(), Error> {
        let theirs = self.row_by_address(other, address).await?;
        if let Some(row) = theirs.filter(|r| r.status == Confirmed) {
            self.move_to(
                person,
                &row,
                Conflict,
                Some(format!("also claimed by {this}")),
                None,
                "conflict",
            )
            .await?;
            Box::pin(self.publish_kol(person, other)).await?;
        }
        Ok(())
    }

    /// A candidate left `confirmed` or `conflict` for `address`: conflicting copies elsewhere
    /// that nobody else claims any more are confirmed again.
    async fn release(&self, person: &Person, address: &str) -> Result<(), Error> {
        let sql = format!(
            "SELECT {COLS} FROM wallets.candidate WHERE address = $1 AND status = 'conflict' ORDER BY id"
        );
        let held = rows(
            sqlx::query_as::<_, Raw>(&sql)
                .bind(address)
                .fetch_all(&self.pool)
                .await?,
        )?;
        for row in held {
            if self.claim(&row.kol, address).await?.is_none() {
                self.move_to(
                    person,
                    &row,
                    Confirmed,
                    Some("conflict resolved".into()),
                    None,
                    "confirm",
                )
                .await?;
                self.publish_kol(person, &row.kol).await?;
            }
        }
        Ok(())
    }

    /// Confirms `row`, or marks it `conflict` when another KOL holds the address.
    async fn confirm_or_conflict(
        &self,
        person: &Person,
        row: Row,
        confirmation: Option<String>,
        reason: Option<String>,
        action: &str,
    ) -> Result<Row, Error> {
        if let Some(owner) = self.claim(&row.kol, &row.address).await? {
            if row.status == Conflict {
                return Err(Error::Refused(format!("still claimed by {owner}")));
            }
            let moved = self
                .move_to(
                    person,
                    &row,
                    Conflict,
                    Some(format!("also claimed by {owner}")),
                    None,
                    "conflict",
                )
                .await?;
            self.conflict_other(person, &owner, &row.address, &row.kol)
                .await?;
            return Ok(moved);
        }
        self.move_to(person, &row, Confirmed, reason, confirmation, action)
            .await
    }

    // ----- publishing ----------------------------------------------------------------------

    /// Makes the registry's wallets of `kol` match the confirmed candidates. Wallets that
    /// KOL Explorer published stay; a confirmed main that differs from KOL Explorer's main
    /// becomes `conflict` and is not published.
    pub(crate) async fn publish_kol(&self, person: &Person, kol: &str) -> Result<(), Error> {
        for _ in 0..6 {
            let confirmed: Vec<Row> = self
                .rows_of(kol)
                .await?
                .into_iter()
                .filter(|r| r.status == Confirmed)
                .collect();
            let registry = self.registry.pick(&SYSTEM);
            let profile = registry
                .get_kol(&SYSTEM, kol)
                .await?
                .ok_or_else(|| Error::Internal(format!("registry does not know {kol}")))?;
            let ke = |w: &api_registry::PublishedWallet| w.source == WalletSource::Kolexplorer;
            let ke_main = profile.main_wallet.clone().filter(ke);
            let ke_others: Vec<String> = profile
                .other_wallets
                .iter()
                .filter(|w| ke(w))
                .map(|w| w.address.clone())
                .collect();

            if confirmed.is_empty() {
                let ours = profile
                    .main_wallet
                    .iter()
                    .chain(&profile.other_wallets)
                    .any(|w| !ke(w));
                if !ours {
                    return Ok(());
                }
                let main = ke_main.as_ref().map(|w| w.address.as_str());
                registry
                    .publish_wallets(&SYSTEM, kol, main, &ke_others, WalletSource::Kolexplorer)
                    .await?;
                self.record(
                    person,
                    kol,
                    "publish",
                    None,
                    Some(json!({ "main": main, "others": ke_others })),
                )
                .await?;
                return Ok(());
            }

            let main_row = confirmed
                .iter()
                .find(|r| r.main)
                .unwrap_or(&confirmed[0])
                .clone();
            if let Some(km) = &ke_main
                && km.address != main_row.address
            {
                self.move_to(
                    person,
                    &main_row,
                    Conflict,
                    Some(format!(
                        "KOL Explorer publishes another main wallet {}",
                        km.address
                    )),
                    None,
                    "conflict",
                )
                .await?;
                continue;
            }
            let main = main_row.address.clone();
            let mut others: Vec<String> = confirmed
                .iter()
                .filter(|r| r.address != main)
                .map(|r| r.address.clone())
                .collect();
            for a in ke_main.iter().map(|w| &w.address).chain(&ke_others) {
                if *a != main && !others.contains(a) {
                    others.push(a.clone());
                }
            }
            let mut published: Vec<&String> =
                profile.other_wallets.iter().map(|w| &w.address).collect();
            published.sort();
            let mut wanted: Vec<&String> = others.iter().collect();
            wanted.sort();
            let in_sync = profile.main_wallet.as_ref().map(|w| w.address.as_str())
                == Some(main.as_str())
                && published == wanted;
            if !in_sync {
                let source = if ke_main.as_ref().is_some_and(|w| w.address == main) {
                    WalletSource::Kolexplorer
                } else {
                    match main_row.source {
                        CandidateSource::Prototype => WalletSource::Prototype,
                        CandidateSource::Kolexplorer => WalletSource::Kolexplorer,
                        _ => WalletSource::Network,
                    }
                };
                match registry
                    .publish_wallets(&SYSTEM, kol, Some(&main), &others, source)
                    .await
                {
                    Ok(()) => {
                        self.record(
                            person,
                            kol,
                            "publish",
                            None,
                            Some(json!({ "main": main, "others": others })),
                        )
                        .await?;
                    }
                    Err(RegistryError::WalletTaken { address, owner }) => {
                        if let Some(row) = confirmed.iter().find(|r| r.address == address) {
                            self.move_to(
                                person,
                                row,
                                Conflict,
                                Some(format!("already published for {owner}")),
                                None,
                                "conflict",
                            )
                            .await?;
                            self.conflict_other(person, &owner, &address, kol).await?;
                        }
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            sqlx::query(
                "UPDATE wallets.candidate SET main = (address = $2) WHERE kol = $1 AND (main OR address = $2)",
            )
            .bind(kol)
            .bind(&main)
            .execute(&self.pool)
            .await?;
            return Ok(());
        }
        Err(Error::Internal(format!("wallets of {kol} did not settle")))
    }

    // ----- finding candidates --------------------------------------------------------------

    async fn trades(&self, address: &str) -> Trades {
        match self
            .solanatracker
            .pick(&SYSTEM)
            .wallet_summary(&SYSTEM, address)
            .await
        {
            Ok(s) if s.has_trades => Trades::Yes(s.last_trade_at),
            Ok(_) => Trades::No,
            Err(e) => Trades::Failed(e.to_string()),
        }
    }

    fn found(t: &Trades, notes: &mut Vec<String>, address: &str) -> Found {
        match t {
            Trades::Yes(last) => Found {
                status: Confirmed,
                reason: Some("trades found".into()),
                last_trade_at: *last,
                checked: true,
            },
            Trades::No => Found {
                status: NotFound,
                reason: Some("no trades found for this address".into()),
                last_trade_at: None,
                checked: true,
            },
            Trades::Failed(e) => {
                notes.push(format!("trades check of {address}: {e}"));
                Found {
                    status: NotFound,
                    reason: Some(format!("trades check failed: {e}")),
                    last_trade_at: None,
                    checked: false,
                }
            }
        }
    }

    async fn touch(
        &self,
        row: &Row,
        last: Option<DateTime<Utc>>,
        reason: Option<&str>,
    ) -> Result<(), Error> {
        sqlx::query(
            "UPDATE wallets.candidate SET last_trade_at = COALESCE($2, last_trade_at), \
             reason = COALESCE($3, reason), checked_at = now() WHERE id = $1",
        )
        .bind(row.id)
        .bind(last)
        .bind(reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The addresses of the sheet row decide the sheet candidates of the KOL.
    async fn sheet_pass(
        &self,
        person: &Person,
        m: &NetworkMember,
        notes: &mut Vec<String>,
    ) -> Result<(), Error> {
        let current: Vec<&String> = m
            .sheet_wallets
            .iter()
            .filter(|a| is_solana_address(a))
            .collect();
        for row in self.rows_of(&m.kol).await? {
            let live = matches!(row.status, Confirmed | NotFound | Conflict | Hypothesis);
            if row.source == CandidateSource::Sheet && live && !current.contains(&&row.address) {
                self.move_to(
                    person,
                    &row,
                    Retired,
                    Some("sheet shows another address".into()),
                    None,
                    "retire",
                )
                .await?;
                self.release(person, &row.address).await?;
            }
        }
        for addr in current {
            match self.row_by_address(&m.kol, addr).await? {
                None => {
                    let t = self.trades(addr).await;
                    let found = Self::found(&t, notes, addr);
                    self.insert(
                        person,
                        &m.kol,
                        addr,
                        CandidateSource::Sheet,
                        Some("network sheet".into()),
                        found,
                        "found",
                    )
                    .await?;
                }
                Some(row) if matches!(row.status, NotFound | Retired | Hypothesis) => {
                    match self.trades(addr).await {
                        Trades::Yes(last) => {
                            self.touch(&row, last, None).await?;
                            self.confirm_or_conflict(
                                person,
                                row,
                                None,
                                Some("sheet address, trades found".into()),
                                "confirm",
                            )
                            .await?;
                        }
                        Trades::No => {
                            let reason = "no trades found for this address";
                            if row.status == Retired {
                                self.move_to(
                                    person,
                                    &row,
                                    NotFound,
                                    Some(reason.into()),
                                    None,
                                    "recheck",
                                )
                                .await?;
                            } else {
                                self.touch(&row, None, Some(reason)).await?;
                            }
                        }
                        Trades::Failed(e) => notes.push(format!("trades check of {addr}: {e}")),
                    }
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    /// The confirmed main wallet's last trade, looked up again when it is old.
    async fn refresh(&self, kol: &str, notes: &mut Vec<String>) -> Result<(), Error> {
        let rows = self.rows_of(kol).await?;
        let main = rows
            .iter()
            .filter(|r| r.status == Confirmed)
            .find(|r| r.main)
            .or_else(|| rows.iter().find(|r| r.status == Confirmed));
        let Some(main) = main else { return Ok(()) };
        if main
            .checked_at
            .is_some_and(|at| Utc::now() - at < REFRESH_AFTER)
        {
            return Ok(());
        }
        match self.trades(&main.address).await {
            Trades::Yes(last) => self.touch(main, last, None).await?,
            Trades::No => self.touch(main, None, None).await?,
            Trades::Failed(e) => notes.push(format!("last trade of {}: {e}", main.address)),
        }
        Ok(())
    }

    /// Matches the KOL's handle against wallet labels at Solana Tracker and KOL Explorer. A
    /// verified X handle confirms; a name-only match (unverified handle, FOMO handle) is a
    /// hypothesis.
    async fn label_pass(
        &self,
        person: &Person,
        m: &NetworkMember,
        notes: &mut Vec<String>,
    ) -> Result<(), Error> {
        let Some(handle) = self
            .roster
            .pick(&SYSTEM)
            .verified_handle(&SYSTEM, &m.kol)
            .await?
        else {
            return Ok(());
        };
        let mut found: Vec<(String, CandidateSource, String)> = Vec::new();
        match self
            .kolexplorer
            .pick(&SYSTEM)
            .wallet_by_handle(&SYSTEM, &handle.handle)
            .await
        {
            Ok(Some(address)) => found.push((
                address,
                CandidateSource::Kolexplorer,
                "KOL Explorer label".into(),
            )),
            Ok(None) => {}
            Err(e) => notes.push(format!("KOL Explorer lookup: {e}")),
        }
        match self
            .solanatracker
            .pick(&SYSTEM)
            .wallet_by_handle(&SYSTEM, &handle.handle)
            .await
        {
            Ok(Some(w)) => {
                let by_fomo = m
                    .fomo_handle
                    .as_deref()
                    .zip(w.fomo_handle.as_deref())
                    .is_some_and(|(a, b)| same_handle(a, b));
                let source = if by_fomo {
                    CandidateSource::Fomo
                } else {
                    CandidateSource::Solanatracker
                };
                let evidence = if by_fomo {
                    "FOMO handle in Solana Tracker identity"
                } else {
                    "Solana Tracker identity"
                };
                if !found.iter().any(|(a, _, _)| *a == w.address) {
                    found.push((w.address, source, evidence.into()));
                }
            }
            Ok(None) => {}
            Err(e) => notes.push(format!("Solana Tracker lookup: {e}")),
        }
        for (address, source, evidence) in found {
            if !is_solana_address(&address) {
                notes.push(format!("{evidence} gave something that is not an address"));
                continue;
            }
            match self.row_by_address(&m.kol, &address).await? {
                None => {
                    let found = if handle.verified {
                        Found {
                            status: Confirmed,
                            reason: Some("verified X handle equals the wallet label".into()),
                            last_trade_at: None,
                            checked: false,
                        }
                    } else {
                        Found {
                            status: Hypothesis,
                            reason: Some(format!("matches @{} by name only", handle.handle)),
                            last_trade_at: None,
                            checked: false,
                        }
                    };
                    self.insert(
                        person,
                        &m.kol,
                        &address,
                        source,
                        Some(evidence),
                        found,
                        "found",
                    )
                    .await?;
                }
                Some(row) if handle.verified && row.status == Hypothesis => {
                    self.confirm_or_conflict(
                        person,
                        row,
                        None,
                        Some("verified X handle equals the wallet label".into()),
                        "confirm",
                    )
                    .await?;
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    /// Conflicting candidates nobody else claims any more are confirmed again.
    async fn recheck_conflicts(&self, person: &Person, kol: &str) -> Result<(), Error> {
        for row in self
            .rows_of(kol)
            .await?
            .into_iter()
            .filter(|r| r.status == Conflict)
        {
            if self.claim(kol, &row.address).await?.is_none() {
                self.move_to(
                    person,
                    &row,
                    Confirmed,
                    Some("conflict resolved".into()),
                    None,
                    "confirm",
                )
                .await?;
            }
        }
        Ok(())
    }

    /// One KOL's pass: sheet addresses, labels, publishing. Returns the problems that did not
    /// stop it (a lookup that failed).
    pub async fn match_member(
        &self,
        person: &Person,
        m: &NetworkMember,
    ) -> Result<Vec<String>, Error> {
        let mut notes = Vec::new();
        self.sheet_pass(person, m, &mut notes).await?;
        self.recheck_conflicts(person, &m.kol).await?;
        let rows = self.rows_of(&m.kol).await?;
        let has_confirmed = rows.iter().any(|r| r.status == Confirmed);
        let has_hypothesis = rows.iter().any(|r| r.status == Hypothesis);
        if !has_confirmed || has_hypothesis {
            self.label_pass(person, m, &mut notes).await?;
        }
        self.refresh(&m.kol, &mut notes).await?;
        self.publish_kol(person, &m.kol).await?;
        Ok(notes)
    }

    /// The pass over the whole network. A failing KOL does not stop the others; the failures
    /// come back together.
    pub async fn match_all(&self, person: &Person) -> Result<(), Error> {
        let mut problems = Vec::new();
        for m in self.members().await? {
            match self.match_member(person, &m).await {
                Ok(notes) => problems.extend(notes.into_iter().map(|n| format!("{}: {n}", m.kol))),
                Err(e) => problems.push(format!("{}: {e}", m.kol)),
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(Error::Internal(problems.join("; ")))
        }
    }

    /// One KOL's pass after staff verified the X handle.
    pub async fn match_kol(&self, person: &Person, kol: &str) -> Result<(), Error> {
        let Some(m) = self.members().await?.into_iter().find(|m| m.kol == kol) else {
            return Ok(());
        };
        let notes = self.match_member(person, &m).await?;
        if notes.is_empty() {
            Ok(())
        } else {
            Err(Error::Internal(notes.join("; ")))
        }
    }

    // ----- staff decisions -----------------------------------------------------------------

    fn staff(person: &Person) -> Result<(), Error> {
        if person.is_staff() {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }

    /// A staff member records that the KOL confirmed the address (hypothesis or conflict).
    /// Without «where and when» it is refused.
    pub async fn confirm(
        &self,
        person: &Person,
        id: i64,
        where_when: &str,
    ) -> Result<Candidate, Error> {
        Self::staff(person)?;
        let where_when = where_when.trim();
        if where_when.is_empty() {
            return Err(Error::Refused(
                "say where and when the KOL confirmed this wallet".into(),
            ));
        }
        let row = self.row(id).await?;
        if !matches!(row.status, Hypothesis | Conflict) {
            return Err(Error::Refused(format!(
                "a {} wallet cannot be confirmed by hand",
                status_str(row.status)
            )));
        }
        let kol = row.kol.clone();
        let done = self
            .confirm_or_conflict(
                person,
                row,
                Some(where_when.to_string()),
                Some("confirmed by the KOL".into()),
                "confirm",
            )
            .await?;
        self.publish_kol(person, &kol).await?;
        Ok(self.row(done.id).await?.candidate())
    }

    pub async fn reject(
        &self,
        person: &Person,
        id: i64,
        reason: Option<&str>,
    ) -> Result<Candidate, Error> {
        Self::staff(person)?;
        let row = self.row(id).await?;
        if !matches!(row.status, Hypothesis | Conflict) {
            return Err(Error::Refused(format!(
                "a {} wallet cannot be rejected",
                status_str(row.status)
            )));
        }
        let reason = reason
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .unwrap_or("rejected by staff");
        let done = self
            .move_to(
                person,
                &row,
                Rejected,
                Some(reason.to_string()),
                None,
                "reject",
            )
            .await?;
        self.release(person, &row.address).await?;
        self.publish_kol(person, &row.kol).await?;
        Ok(done.candidate())
    }

    pub async fn revoke(&self, person: &Person, id: i64, reason: &str) -> Result<Candidate, Error> {
        Self::staff(person)?;
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(Error::Refused("say why the wallet is revoked".into()));
        }
        let row = self.row(id).await?;
        if !matches!(row.status, Confirmed | Conflict) {
            return Err(Error::Refused(format!(
                "a {} wallet cannot be revoked",
                status_str(row.status)
            )));
        }
        let done = self
            .move_to(
                person,
                &row,
                Revoked,
                Some(reason.to_string()),
                None,
                "revoke",
            )
            .await?;
        self.release(person, &row.address).await?;
        self.publish_kol(person, &row.kol).await?;
        Ok(done.candidate())
    }

    pub async fn set_main(&self, person: &Person, id: i64) -> Result<Candidate, Error> {
        Self::staff(person)?;
        let row = self.row(id).await?;
        if row.status != Confirmed {
            return Err(Error::Refused(
                "only a confirmed wallet can be the main one".into(),
            ));
        }
        let before: Option<String> = self
            .rows_of(&row.kol)
            .await?
            .into_iter()
            .find(|r| r.status == Confirmed && r.main)
            .map(|r| r.address);
        sqlx::query("UPDATE wallets.candidate SET main = (id = $2), updated_at = now() WHERE kol = $1 AND (main OR id = $2)")
            .bind(&row.kol)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.record(
            person,
            &row.kol,
            "main",
            Some(json!({ "main": before })),
            Some(json!({ "main": row.address })),
        )
        .await?;
        self.publish_kol(person, &row.kol).await?;
        Ok(self.row(id).await?.candidate())
    }

    /// An address the KOL (or someone for the KOL) sent. A trades check at Solana Tracker
    /// decides: trades found confirms it, none leaves it «not found» with the reason.
    pub async fn propose(
        &self,
        person: &Person,
        kol: &str,
        address: &str,
        evidence: &str,
    ) -> Result<Candidate, Error> {
        Self::staff(person)?;
        let address = address.trim();
        if !is_solana_address(address) {
            return Err(Error::InvalidAddress(address.to_string()));
        }
        let evidence = evidence.trim();
        if evidence.is_empty() {
            return Err(Error::Refused("say where the address came from".into()));
        }
        if !self.roster.pick(person).is_member(person, kol).await? {
            return Err(Error::NotMember(kol.to_string()));
        }
        let mut notes = Vec::new();
        let t = self.trades(address).await;
        let id = match self.row_by_address(kol, address).await? {
            None => {
                let found = Self::found(&t, &mut notes, address);
                self.insert(
                    person,
                    kol,
                    address,
                    CandidateSource::FromKol,
                    Some(evidence.to_string()),
                    found,
                    "propose",
                )
                .await?
                .id
            }
            Some(row) if matches!(row.status, NotFound | Retired | Hypothesis) => {
                sqlx::query("UPDATE wallets.candidate SET source = 'from_kol', evidence = $2, sent_by = $3 WHERE id = $1")
                    .bind(row.id)
                    .bind(evidence)
                    .bind(who(person))
                    .execute(&self.pool)
                    .await?;
                let row = self.row(row.id).await?;
                let id = row.id;
                match t {
                    Trades::Yes(last) => {
                        self.touch(&row, last, None).await?;
                        self.confirm_or_conflict(
                            person,
                            row,
                            None,
                            Some("sent by the KOL, trades found".into()),
                            "confirm",
                        )
                        .await?;
                    }
                    Trades::No => {
                        let reason = "no trades found for this address";
                        if row.status == Retired {
                            self.move_to(
                                person,
                                &row,
                                NotFound,
                                Some(reason.into()),
                                None,
                                "recheck",
                            )
                            .await?;
                        } else {
                            self.touch(&row, None, Some(reason)).await?;
                        }
                    }
                    Trades::Failed(e) => {
                        return Err(Error::Internal(format!("trades check failed: {e}")));
                    }
                }
                id
            }
            Some(row) => row.id,
        };
        self.publish_kol(person, kol).await?;
        Ok(self.row(id).await?.candidate())
    }
}

/// Convenience for handlers: the service behind an `Arc`.
pub type Shared = Arc<Wallets>;
