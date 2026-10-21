//! The roster: sheet read, membership, X handle, API.

use crate::{
    sheet::{self, SheetRow, Skipped},
    store::{self, MemberRow, db},
};
use api_registry::{KolLinks, KolUpsert, Reach, RegistryApi};
use api_roster::{
    FORMATS, NetworkMember, Rates, RosterApi, RosterError, TableRead, XHandle, XHandleVerified,
};
use api_snapshots::{MemberReason, SnapshotsApi};
use arena_platform::{
    Api, Config, DataLock, Events, Job, Person, Setting,
    scheduler::{LockSpec, RunError, run_locked},
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;

pub(crate) type Journal = Api<dyn api_journal::JournalApi>;

/// Where the sheet's CSV text comes from.
#[async_trait]
pub trait SheetFetch: Send + Sync {
    /// The CSV export at `url`. The error text must not contain `url`: it is a secret.
    async fn fetch(&self, url: &str) -> Result<String, String>;
}

/// Used until the server gets an HTTP client: every read fails, visibly.
pub struct NoHttpClient;

#[async_trait]
impl SheetFetch for NoHttpClient {
    async fn fetch(&self, _url: &str) -> Result<String, String> {
        Err("this build has no HTTP client for the sheet".into())
    }
}

/// Outcome of one sheet read.
#[derive(Clone, Debug, Serialize)]
pub struct SyncReport {
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    /// `ok` or `failed`; a failed read changes nothing.
    pub status: &'static str,
    pub error: Option<String>,
    pub rows: u32,
    pub added: u32,
    pub updated: u32,
    pub removed: u32,
    pub skipped: Vec<Skipped>,
    /// What went wrong after a good read, e.g. the wallet pass.
    pub note: Option<String>,
}

/// Why a roster operation did not happen.
#[derive(Debug, thiserror::Error)]
pub enum OpError {
    /// The shared `arena_data` lock is held.
    #[error("collection running, try again later")]
    Busy,
    #[error("{0} is not in the network")]
    NotMember(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Failed(String),
}

impl From<RosterError> for OpError {
    fn from(e: RosterError) -> Self {
        OpError::Failed(e.to_string())
    }
}

impl From<RunError> for OpError {
    fn from(e: RunError) -> Self {
        match e {
            RunError::Busy { .. } => OpError::Busy,
            other => OpError::Failed(other.to_string()),
        }
    }
}

pub(crate) const JOB: &str = "roster_sync";

pub struct Roster {
    pub(crate) pool: PgPool,
    pub(crate) config: Arc<Config>,
    pub(crate) events: Events,
    pub(crate) registry: Api<dyn RegistryApi>,
    pub(crate) snapshots: Api<dyn SnapshotsApi>,
    pub(crate) journal: Journal,
    pub(crate) fetcher: Arc<dyn SheetFetch>,
}

/// Fields of a member the journal and the change check look at.
fn fingerprint(r: &MemberRow) -> Vec<(&'static str, String)> {
    vec![
        ("name", r.name.clone()),
        ("fomo", r.fomo_handle.clone().unwrap_or_default()),
        ("pumpfun", r.pumpfun.clone().unwrap_or_default()),
        ("formats", r.formats.join(",")),
        ("group", r.grp.clone().unwrap_or_default()),
        ("rates", format!("{:?}/{:?}", r.low_usd, r.high_usd)),
        ("note", r.note.clone().unwrap_or_default()),
        ("wallets", r.sheet_wallets.join(",")),
        ("wallet_note", r.wallet_note.clone().unwrap_or_default()),
    ]
}

/// What a sheet row does to the member table.
struct Planned {
    row: SheetRow,
    kol: String,
    /// The member row before this read; `None` for a new member.
    before: Option<MemberRow>,
    formats: Vec<String>,
}

impl Roster {
    /// The 05:45 UTC job. It runs under the job lock and the `arena_data` lock.
    pub fn jobs(self: &Arc<Self>) -> Vec<Job> {
        let me = self.clone();
        vec![Job::daily(
            JOB,
            NaiveTime::from_hms_opt(5, 45, 0).expect("time"),
            DataLock::Try,
            move || {
                let me = me.clone();
                async move {
                    let report = me.sync_unlocked(&Person::SYSTEM).await;
                    match report.error {
                        None => Ok(()),
                        Some(e) => Err(e.into()),
                    }
                }
            },
        )]
    }

    /// «Read the sheet now»: the same run as the job, under the same locks. A held lock
    /// answers `Busy` («collection running, try again later»).
    pub async fn sync_now(self: &Arc<Self>, person: &Person) -> Result<SyncReport, OpError> {
        let me = self.clone();
        let person = *person;
        let locks = vec![
            LockSpec::job(JOB),
            LockSpec::try_once(arena_platform::scheduler::DATA_LOCK),
        ];
        Ok(run_locked(
            &self.pool,
            locks,
            async move { me.sync_unlocked(&person).await },
        )
        .await?)
    }

    /// One sheet read; the caller holds the `arena_data` lock. Never errors: a failure is a
    /// `failed` report, stored for the collection screen, and the roster stays as it was.
    pub async fn sync_unlocked(&self, person: &Person) -> SyncReport {
        let started_at = Utc::now();
        let mut report = SyncReport {
            started_at,
            finished_at: started_at,
            status: "failed",
            error: None,
            rows: 0,
            added: 0,
            updated: 0,
            removed: 0,
            skipped: Vec::new(),
            note: None,
        };
        match self.read_and_apply(person, &mut report).await {
            Ok(()) => {
                report.status = "ok";
                let date = Utc::now().date_naive();
                if let Err(e) = self.events.emit(&TableRead { date }).await {
                    report.note = Some(format!("after the read: {e}"));
                }
            }
            Err(e) => report.error = Some(e),
        }
        report.finished_at = Utc::now();
        if let Err(e) = self.store_run(&report).await {
            tracing::error!(error = %e, "roster: could not store the sheet run");
        }
        report
    }

    async fn store_run(&self, r: &SyncReport) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO roster.sync_run (started_at, finished_at, status, error, rows, added, updated, removed, skipped, note) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(r.started_at)
        .bind(r.finished_at)
        .bind(r.status)
        .bind(&r.error)
        .bind(r.rows as i32)
        .bind(r.added as i32)
        .bind(r.updated as i32)
        .bind(r.removed as i32)
        .bind(serde_json::to_value(&r.skipped).unwrap_or(Value::Null))
        .bind(&r.note)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn read_and_apply(&self, person: &Person, report: &mut SyncReport) -> Result<(), String> {
        let url = self
            .config
            .get(Setting::NetworkSheetCsvUrl)
            .map_err(|e| e.to_string())?;
        let text =
            self.fetcher.fetch(url).await.map_err(|e| {
                format!("sheet could not be read: {}", e.replace(url, "<sheet url>"))
            })?;
        let parsed = sheet::parse(&text).map_err(|e| e.to_string())?;
        report.rows = parsed.rows.len() as u32;
        report.skipped = parsed.skipped;
        if parsed.rows.is_empty() {
            return Err("sheet has no usable rows; roster unchanged".into());
        }

        let members = store::members(&self.pool, false)
            .await
            .map_err(|e| e.to_string())?;
        let mut planned: Vec<Planned> = Vec::new();
        for row in parsed.rows {
            let mut before = members
                .iter()
                .find(|m| m.fomo_key.as_deref() == Some(row.fomo_key.as_str()))
                .cloned();
            let mut known_kol = before.as_ref().map(|m| m.kol.clone());
            if known_kol.is_none() {
                // A KOL the registry already knows by this handle (a board KOL, a prototype
                // network KOL): reuse it instead of creating a second one.
                let found = self
                    .registry
                    .pick(person)
                    .find_by_handle(person, &row.fomo_key)
                    .await
                    .map_err(|e| e.to_string())?;
                if let Some(kol) = found {
                    before = members.iter().find(|m| m.kol == kol).cloned();
                    known_kol = Some(kol);
                }
            }
            if let Some(kol) = &known_kol
                && planned.iter().any(|p| &p.kol == kol)
            {
                report.skipped.push(Skipped {
                    line: row.line,
                    reason: "same KOL as an earlier row".into(),
                });
                continue;
            }
            let kol = self.registry_kol(person, &row, known_kol).await?;
            let formats = if row.formats.is_empty() {
                before
                    .as_ref()
                    .map(|b| b.formats.clone())
                    .unwrap_or_default()
            } else {
                row.formats.clone()
            };
            planned.push(Planned {
                row,
                kol,
                before,
                formats,
            });
        }
        for p in &planned {
            let old = p
                .before
                .as_ref()
                .map(|b| b.formats.clone())
                .unwrap_or_default();
            if p.formats != old || p.before.is_none() {
                self.registry
                    .pick(person)
                    .set_formats(person, &p.kol, &p.formats)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        self.apply(person, planned, report).await
    }

    /// The registry KOL for a sheet row: creates it, or updates its FOMO and pump.fun links.
    /// An existing KOL keeps its public name.
    async fn registry_kol(
        &self,
        person: &Person,
        row: &SheetRow,
        known: Option<String>,
    ) -> Result<String, String> {
        let registry = self.registry.pick(person);
        let profile = match &known {
            Some(kol) => registry
                .get_kol(person, kol)
                .await
                .map_err(|e| e.to_string())?,
            None => None,
        };
        let Some(profile) = profile else {
            // New KOL; its X handle is the FOMO handle until staff verifies one.
            return registry
                .upsert_kol(
                    person,
                    &KolUpsert {
                        id: known,
                        name: row.name.clone(),
                        x_handle: Some(row.fomo.clone()),
                        links: KolLinks {
                            fomo: Some(row.fomo.clone()),
                            pumpfun: row.pumpfun.clone(),
                            ..KolLinks::default()
                        },
                    },
                )
                .await
                .map_err(|e| e.to_string());
        };
        let mut links = profile.links.clone();
        links.fomo = Some(row.fomo.clone());
        if row.pumpfun.is_some() {
            links.pumpfun = row.pumpfun.clone();
        }
        if links != profile.links {
            registry
                .upsert_kol(
                    person,
                    &KolUpsert {
                        id: Some(profile.id.clone()),
                        name: profile.name.clone(),
                        x_handle: profile.x_handle.clone(),
                        links,
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(profile.id)
    }

    async fn apply(
        &self,
        person: &Person,
        planned: Vec<Planned>,
        report: &mut SyncReport,
    ) -> Result<(), String> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        let mut journal: Vec<(String, &'static str, Option<Value>, Option<Value>)> = Vec::new();
        for p in &planned {
            let r = &p.row;
            let state = |name: &str, fomo: &str, pumpfun: Option<&str>, formats: &[String]| json!({ "name": name, "fomo": fomo, "pumpfun": pumpfun, "formats": formats });
            sqlx::query(
                "INSERT INTO roster.member (kol, name, fomo_handle, fomo_key, pumpfun, formats, active, last_seen_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, true, $7) \
                 ON CONFLICT (kol) DO UPDATE SET name = EXCLUDED.name, fomo_handle = EXCLUDED.fomo_handle, \
                   fomo_key = EXCLUDED.fomo_key, pumpfun = COALESCE(EXCLUDED.pumpfun, roster.member.pumpfun), \
                   formats = EXCLUDED.formats, active = true, left_at = NULL, last_seen_at = EXCLUDED.last_seen_at",
            )
            .bind(&p.kol)
            .bind(&r.name)
            .bind(&r.fomo)
            .bind(&r.fomo_key)
            .bind(&r.pumpfun)
            .bind(&p.formats)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO roster.terms (kol, grp, low_usd, high_usd, note, wallet_note, sheet_wallets, sheet_line, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
                 ON CONFLICT (kol) DO UPDATE SET grp = EXCLUDED.grp, low_usd = EXCLUDED.low_usd, \
                   high_usd = EXCLUDED.high_usd, note = EXCLUDED.note, wallet_note = EXCLUDED.wallet_note, \
                   sheet_wallets = EXCLUDED.sheet_wallets, sheet_line = EXCLUDED.sheet_line, updated_at = EXCLUDED.updated_at",
            )
            .bind(&p.kol)
            .bind(&r.group)
            .bind(r.low_usd)
            .bind(r.high_usd)
            .bind(&r.note)
            .bind(&r.wallet_note)
            .bind(&r.wallets)
            .bind(r.line as i32)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

            let after = state(&r.name, &r.fomo, r.pumpfun.as_deref(), &p.formats);
            match &p.before {
                None => {
                    report.added += 1;
                    self.event(&mut tx, &p.kol, "joined", "added by the sheet")
                        .await?;
                    journal.push((p.kol.clone(), "added", None, Some(after)));
                }
                Some(b) if !b.active => {
                    report.added += 1;
                    self.event(&mut tx, &p.kol, "rejoined", "back in the sheet")
                        .await?;
                    journal.push((p.kol.clone(), "rejoined", None, Some(after)));
                }
                Some(b) => {
                    let mut now_row = b.clone();
                    now_row.name = r.name.clone();
                    now_row.fomo_handle = Some(r.fomo.clone());
                    now_row.pumpfun = r.pumpfun.clone().or(b.pumpfun.clone());
                    now_row.formats = p.formats.clone();
                    now_row.grp = r.group.clone();
                    now_row.low_usd = r.low_usd;
                    now_row.high_usd = r.high_usd;
                    now_row.note = r.note.clone();
                    now_row.wallet_note = r.wallet_note.clone();
                    now_row.sheet_wallets = r.wallets.clone();
                    let changed: Vec<&str> = fingerprint(b)
                        .into_iter()
                        .zip(fingerprint(&now_row))
                        .filter(|(x, y)| x.1 != y.1)
                        .map(|(x, _)| x.0)
                        .collect();
                    if !changed.is_empty() {
                        report.updated += 1;
                        // Values of rates, group and notes never go to the journal: only
                        // the names of the fields that changed.
                        let mut after = after;
                        after["changed"] = json!(changed);
                        let before = state(
                            &b.name,
                            b.fomo_handle.as_deref().unwrap_or(""),
                            b.pumpfun.as_deref(),
                            &b.formats,
                        );
                        journal.push((p.kol.clone(), "updated", Some(before), Some(after)));
                    }
                }
            }
        }
        let matched: Vec<String> = planned.iter().map(|p| p.kol.clone()).collect();
        let left: Vec<String> = sqlx::query_scalar(
            "UPDATE roster.member SET active = false, left_at = $2 \
             WHERE active AND NOT keep AND kol <> ALL($1) RETURNING kol",
        )
        .bind(&matched)
        .bind(now)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        for kol in &left {
            self.event(&mut tx, kol, "left", "row removed from the sheet")
                .await?;
            journal.push((
                kol.clone(),
                "removed",
                Some(json!({ "active": true })),
                Some(json!({ "active": false })),
            ));
        }
        report.removed = left.len() as u32;
        tx.commit().await.map_err(|e| e.to_string())?;

        let j = self.journal.pick(person);
        for (kol, action, before, after) in journal {
            j.record(person, "roster", &kol, action, before, after)
                .await
                .map_err(|e| format!("journal: {e}"))?;
        }
        j.record(
            person,
            "roster",
            "sheet",
            "sync",
            None,
            Some(json!({ "rows": report.rows, "added": report.added, "updated": report.updated, "removed": report.removed })),
        )
        .await
        .map_err(|e| format!("journal: {e}"))?;
        Ok(())
    }

    async fn event(
        &self,
        tx: &mut sqlx::PgConnection,
        kol: &str,
        action: &str,
        reason: &str,
    ) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO roster.membership_event (kol, action, reason) VALUES ($1, $2, $3)",
        )
        .bind(kol)
        .bind(action)
        .bind(reason)
        .execute(tx)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Adds the network KOLs with a published wallet to the snapshot of `date`.
    pub async fn add_to_snapshot(&self, date: NaiveDate) -> Result<(), String> {
        let person = Person::SYSTEM;
        let ids: Vec<String> = store::members(&self.pool, true)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|m| m.kol)
            .collect();
        let profiles = self
            .registry
            .pick(&person)
            .list_kols(&person, &ids)
            .await
            .map_err(|e| e.to_string())?;
        let with_wallet: Vec<String> = profiles
            .into_iter()
            .filter(|p| p.main_wallet.is_some())
            .map(|p| p.id)
            .collect();
        self.snapshots
            .pick(&person)
            .set_members(&person, date, MemberReason::Network, &with_wallet)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn is_member(&self, kol: &str) -> Result<bool, RosterError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM roster.member WHERE kol = $1 AND active)")
            .bind(kol)
            .fetch_one(&self.pool)
            .await
            .map_err(db)
    }

    /// Staff records a verified X handle, formats and/or reach for a member. A new or
    /// changed handle emits «X handle verified»; its error text comes back as `warning`.
    pub(crate) async fn patch(
        &self,
        person: &Person,
        kol: &str,
        patch: &MemberPatch,
    ) -> Result<(MemberRow, Option<String>), OpError> {
        let before = store::member(&self.pool, kol)
            .await?
            .filter(|m| m.active)
            .ok_or_else(|| OpError::NotMember(kol.to_string()))?;
        let handle = match &patch.x_handle {
            Some(h) => {
                let h = h.trim().trim_start_matches('@');
                if h.is_empty()
                    || h.len() > 15
                    || !h.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    return Err(OpError::Invalid("not an X handle".into()));
                }
                Some(h.to_string())
            }
            None => None,
        };
        let formats = match &patch.formats {
            Some(list) => {
                let mut out = Vec::new();
                for f in list {
                    let f = f.trim().to_lowercase();
                    if !FORMATS.contains(&f.as_str()) {
                        return Err(OpError::Invalid(format!("unknown format {f}")));
                    }
                    if !out.contains(&f) {
                        out.push(f);
                    }
                }
                Some(out)
            }
            None => None,
        };
        if let Some(r) = &patch.reach {
            if r.source.trim().is_empty() {
                return Err(OpError::Invalid("reach needs a source".into()));
            }
            if [r.x_followers, r.telegram_subscribers, r.pumpfun_followers]
                .iter()
                .flatten()
                .any(|n| *n < 0)
            {
                return Err(OpError::Invalid("reach cannot be negative".into()));
            }
        }

        let registry = self.registry.pick(person);
        if let Some(h) = &handle {
            let profile = registry
                .get_kol(person, kol)
                .await
                .map_err(|e| OpError::Failed(e.to_string()))?
                .ok_or_else(|| OpError::Failed(format!("registry does not know {kol}")))?;
            registry
                .upsert_kol(
                    person,
                    &KolUpsert {
                        id: Some(kol.to_string()),
                        name: profile.name,
                        x_handle: Some(h.clone()),
                        links: profile.links,
                    },
                )
                .await
                .map_err(|e| OpError::Failed(e.to_string()))?;
        }
        if let Some(f) = &formats {
            registry
                .set_formats(person, kol, f)
                .await
                .map_err(|e| OpError::Failed(e.to_string()))?;
        }
        if let Some(r) = &patch.reach {
            registry
                .set_reach(
                    person,
                    kol,
                    &Reach {
                        x_followers: r.x_followers,
                        telegram_subscribers: r.telegram_subscribers,
                        pumpfun_followers: r.pumpfun_followers,
                    },
                    r.source.trim(),
                    r.date,
                )
                .await
                .map_err(|e| OpError::Failed(e.to_string()))?;
        }
        sqlx::query(
            "UPDATE roster.member SET x_handle = COALESCE($2, x_handle), x_verified = x_verified OR $3, \
             formats = COALESCE($4, formats) WHERE kol = $1",
        )
        .bind(kol)
        .bind(&handle)
        .bind(handle.is_some())
        .bind(&formats)
        .execute(&self.pool)
        .await
        .map_err(|e| OpError::Failed(e.to_string()))?;
        let after = store::member(&self.pool, kol)
            .await?
            .ok_or_else(|| OpError::NotMember(kol.to_string()))?;

        let state = |m: &MemberRow| json!({ "x_handle": m.handle().map(|h| h.handle), "verified": m.x_verified, "formats": m.formats });
        let mut after_json = state(&after);
        if let Some(r) = &patch.reach {
            after_json["reach"] = json!({ "source": r.source.trim(), "date": r.date });
        }
        self.journal
            .pick(person)
            .record(
                person,
                "roster",
                kol,
                "patch",
                Some(state(&before)),
                Some(after_json),
            )
            .await
            .map_err(|e| OpError::Failed(format!("saved, but the journal failed: {e}")))?;

        let mut warning = None;
        let verified_now = handle.is_some()
            && (!before.x_verified
                || before.x_handle.as_deref().map(str::to_lowercase)
                    != handle.as_deref().map(str::to_lowercase));
        if verified_now
            && let Err(e) = self
                .events
                .emit(&XHandleVerified {
                    kol: kol.to_string(),
                })
                .await
        {
            warning = Some(format!("X handle saved; wallet match: {e}"));
        }
        Ok((after, warning))
    }

    /// The team export: everything the sheet gave, including rates and notes.
    pub async fn export_csv(&self) -> Result<String, RosterError> {
        let mut out =
            String::from("Name,Group,Low $,High $,Deliverable / note,FOMO,Pump.fun,Formats\r\n");
        for m in store::members(&self.pool, true).await? {
            let cells = [
                text_cell(&m.name),
                text_cell(m.grp.as_deref().unwrap_or("")),
                num_cell(m.low_usd),
                num_cell(m.high_usd),
                text_cell(m.note.as_deref().unwrap_or("")),
                text_cell(m.fomo_handle.as_deref().unwrap_or("")),
                text_cell(m.pumpfun.as_deref().unwrap_or("")),
                text_cell(&m.formats.join(" ")),
            ];
            out.push_str(&cells.join(","));
            out.push_str("\r\n");
        }
        Ok(out)
    }
}

fn num_cell(v: Option<f64>) -> String {
    match v {
        Some(v) if v.fract() == 0.0 => format!("{}", v as i64),
        Some(v) => format!("{v}"),
        None => String::new(),
    }
}

/// CSV text cell. A leading `=`, `+`, `-` or `@` would run as a formula in a spreadsheet:
/// it gets a leading apostrophe.
fn text_cell(s: &str) -> String {
    let mut t = s.replace(['\r', '\n'], " ");
    if t.starts_with(['=', '+', '-', '@']) {
        t.insert(0, '\'');
    }
    if t.contains([',', '"']) {
        format!("\"{}\"", t.replace('"', "\"\""))
    } else {
        t
    }
}

/// Body of `PATCH /api/admin/network/:kol`.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct MemberPatch {
    /// The X handle staff checked.
    pub x_handle: Option<String>,
    pub formats: Option<Vec<String>>,
    pub reach: Option<ReachInput>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ReachInput {
    pub x_followers: Option<i64>,
    pub telegram_subscribers: Option<i64>,
    pub pumpfun_followers: Option<i64>,
    /// Where the numbers come from, e.g. `X profile`.
    pub source: String,
    pub date: NaiveDate,
}

/// `RosterApi` over the tables.
pub struct RosterImpl(pub Arc<Roster>);

#[async_trait]
impl RosterApi for RosterImpl {
    async fn members(&self, person: &Person) -> Result<Vec<NetworkMember>, RosterError> {
        let team = person.is_staff();
        Ok(store::members(&self.0.pool, true)
            .await?
            .iter()
            .map(|m| m.member(team))
            .collect())
    }

    async fn is_member(&self, _person: &Person, kol: &str) -> Result<bool, RosterError> {
        self.0.is_member(kol).await
    }

    async fn verified_handle(
        &self,
        _person: &Person,
        kol: &str,
    ) -> Result<Option<XHandle>, RosterError> {
        Ok(store::member(&self.0.pool, kol)
            .await?
            .filter(|m| m.active)
            .and_then(|m| m.handle()))
    }

    async fn rates(&self, person: &Person, kols: &[String]) -> Result<Vec<Rates>, RosterError> {
        if !person.is_staff() {
            return Err(RosterError::Forbidden);
        }
        Ok(store::members(&self.0.pool, true)
            .await?
            .iter()
            .filter(|m| kols.contains(&m.kol))
            .map(MemberRow::rates)
            .collect())
    }
}

/// Fills the board's «in network?» slot.
pub struct InNetworkFiller(pub Arc<Roster>);

#[async_trait]
impl api_board::InNetwork for InNetworkFiller {
    async fn is_member(&self, _person: &Person, kol: &str) -> bool {
        match self.0.is_member(kol).await {
            Ok(yes) => yes,
            Err(e) => {
                tracing::error!(error = %e, "roster: in_network check failed");
                false
            }
        }
    }
}
