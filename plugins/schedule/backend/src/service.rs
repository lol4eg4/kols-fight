//! The collection run: open, event, KOL Explorer, Solana Tracker, close, rank.

use crate::store::{self, NewStep};
use api_journal::JournalApi;
use api_kolexplorer::{KeRunStatus, KolexplorerApi};
use api_schedule::SnapshotOpened;
use api_score::ScoreApi;
use api_snapshots::{MemberReason, SnapshotStatus, SnapshotsApi, SnapshotsError};
use api_solanatracker::{KolRunStatus, SolanatrackerApi, StRunStatus};
use arena_platform::{
    Api, BoxError, DataLock, Events, Job, LockSpec, Person, RunError, scheduler::DATA_LOCK,
    scheduler::run_locked,
};
use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, NaiveTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

/// Name of the job; its lock is `job:collect`.
pub const JOB: &str = "collect";

/// How far back start-up recovery looks for an open snapshot.
const RECOVERY_DAYS: i64 = 31;

#[derive(Clone, Debug)]
pub struct Options {
    /// How long the 06:00 run waits for the `arena_data` lock before it fails (30 minutes).
    pub data_wait: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            data_wait: Duration::from_secs(30 * 60),
        }
    }
}

/// Why a manual run did not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartError {
    /// Another run holds the job lock or the data lock; the text says which.
    Busy(String),
    /// The date is not today and has no snapshot to run again.
    UnknownDate(NaiveDate),
    FutureDate(NaiveDate),
    Internal(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::Busy(m) | StartError::Internal(m) => f.write_str(m),
            StartError::UnknownDate(d) => write!(f, "there is no snapshot for {d} to run again"),
            StartError::FutureDate(d) => write!(f, "{d} is in the future"),
        }
    }
}

/// A manual run that has started.
pub struct Started {
    pub run_id: i64,
    /// Ends when the run ends.
    pub done: JoinHandle<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Overall {
    Ok,
    Partial,
    Failed,
}

impl Overall {
    fn as_str(self) -> &'static str {
        match self {
            Overall::Ok => "ok",
            Overall::Partial => "partial",
            Overall::Failed => "failed",
        }
    }
}

struct Outcome {
    status: &'static str,
    rows: Option<i32>,
    error: Option<String>,
    note: Option<String>,
}

impl Outcome {
    fn ok() -> Self {
        Outcome {
            status: "ok",
            rows: None,
            error: None,
            note: None,
        }
    }

    fn failed(error: impl ToString) -> Self {
        Outcome {
            status: "failed",
            rows: None,
            error: Some(error.to_string()),
            note: None,
        }
    }
}

struct Inner {
    pool: PgPool,
    events: Events,
    snapshots: Api<dyn SnapshotsApi>,
    kolexplorer: Api<dyn KolexplorerApi>,
    solanatracker: Api<dyn SolanatrackerApi>,
    score: Api<dyn ScoreApi>,
    journal: Api<dyn JournalApi>,
    options: Options,
}

/// The collection service. Cheap to clone.
#[derive(Clone)]
pub struct Schedule {
    inner: Arc<Inner>,
}

/// Records the steps of one run.
struct Recorder<'a> {
    pool: &'a PgPool,
    run_id: i64,
    seq: i32,
    worst: Overall,
    first_error: Option<String>,
}

impl Recorder<'_> {
    /// Stores a step; `critical` steps (open, close, rank) fail the run when they fail.
    async fn step(&mut self, name: &str, started: DateTime<Utc>, o: Outcome, critical: bool) {
        self.seq += 1;
        let finished = Utc::now();
        if let Err(e) = store::insert_step(
            self.pool,
            &NewStep {
                run_id: self.run_id,
                step: name,
                seq: self.seq,
                status: o.status,
                started_at: started,
                finished_at: finished,
                rows: o.rows,
                error: o.error.as_deref(),
                note: o.note.as_deref(),
            },
        )
        .await
        {
            tracing::error!(run = self.run_id, step = name, error = %e, "cannot store step");
        }
        if o.status != "ok" {
            let level = if o.status == "failed" && critical {
                Overall::Failed
            } else {
                Overall::Partial
            };
            if level == Overall::Failed || self.worst == Overall::Ok {
                self.worst = level;
            }
            if (self.first_error.is_none() || level == Overall::Failed)
                && let Some(err) = o.error
            {
                self.first_error = Some(format!("{name}: {err}"));
            }
        }
    }
}

impl Schedule {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: PgPool,
        events: Events,
        snapshots: Api<dyn SnapshotsApi>,
        kolexplorer: Api<dyn KolexplorerApi>,
        solanatracker: Api<dyn SolanatrackerApi>,
        score: Api<dyn ScoreApi>,
        journal: Api<dyn JournalApi>,
        options: Options,
    ) -> Self {
        Schedule {
            inner: Arc::new(Inner {
                pool,
                events,
                snapshots,
                kolexplorer,
                solanatracker,
                score,
                journal,
                options,
            }),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    pub fn today(&self) -> NaiveDate {
        Utc::now().date_naive()
    }

    /// The 06:00 UTC job. The scheduler holds `job:collect`; the run waits for `arena_data`.
    pub fn job(&self) -> Job {
        let me = self.clone();
        Job::daily(
            JOB,
            NaiveTime::from_hms_opt(6, 0, 0).expect("06:00"),
            DataLock::No,
            move || {
                let me = me.clone();
                async move { me.run_scheduled().await.map_err(BoxError::from) }
            },
        )
    }

    /// Today's collection under the data lock, which it waits for up to `data_wait`.
    /// A lock that stays held fails the run visibly: a failed run row.
    pub async fn run_scheduled(&self) -> Result<(), String> {
        let date = self.today();
        let me = self.clone();
        let wait = self.inner.options.data_wait;
        let locked = run_locked(
            &self.inner.pool,
            vec![LockSpec::wait(DATA_LOCK, wait)],
            async move { me.run_scheduled_locked(date).await },
        )
        .await;
        match locked {
            Ok(Ok(Overall::Failed)) => Err("collection run failed".into()),
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => {
                let text = match &e {
                    RunError::Busy { .. } => {
                        format!("{e} (waited {} min)", wait.as_secs().div_ceil(60))
                    }
                    other => other.to_string(),
                };
                self.record_failed(date, "scheduled", &text).await;
                Err(text)
            }
        }
    }

    async fn record_failed(&self, date: NaiveDate, trigger: &str, error: &str) {
        if let Err(e) =
            store::insert_failed_run(&self.inner.pool, date, trigger, Utc::now(), error).await
        {
            tracing::error!(error = %e, "cannot store the failed run");
        }
    }

    /// «Run now»: starts a run for `date` (today, or a date that has a snapshot) in the
    /// background. Answers busy at once when a run or the data lock is held. The sources
    /// only give today's numbers, so an earlier date is only closed and ranked again.
    pub async fn start_manual(
        &self,
        person: Person,
        date: NaiveDate,
    ) -> Result<Started, StartError> {
        if date > self.today() {
            return Err(StartError::FutureDate(date));
        }
        if date != self.today() {
            match self
                .inner
                .snapshots
                .pick(&Person::SYSTEM)
                .snapshot(&Person::SYSTEM, date)
                .await
            {
                Ok(_) => {}
                Err(SnapshotsError::NoSnapshot { .. }) => {
                    return Err(StartError::UnknownDate(date));
                }
                Err(e) => return Err(StartError::Internal(e.to_string())),
            }
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        let me = self.clone();
        let pool = self.inner.pool.clone();
        let handle = tokio::spawn(async move {
            run_locked(
                &pool,
                vec![LockSpec::job(JOB), LockSpec::try_once(DATA_LOCK)],
                async move { me.run_manual(person, date, tx).await },
            )
            .await
        });
        // The run sends its id once it holds the locks; a busy lock ends the task first.
        match rx.await {
            Ok(Some(run_id)) => Ok(Started {
                run_id,
                done: tokio::spawn(async move {
                    let _ = handle.await;
                }),
            }),
            Ok(None) | Err(_) => match handle.await {
                Ok(Err(RunError::Busy { lock })) => {
                    Err(StartError::Busy(RunError::Busy { lock }.to_string()))
                }
                Ok(Err(e)) => Err(StartError::Internal(e.to_string())),
                Ok(Ok(())) => Err(StartError::Internal("the run did not start".into())),
                Err(e) => Err(StartError::Internal(e.to_string())),
            },
        }
    }

    async fn run_manual(
        &self,
        person: Person,
        date: NaiveDate,
        started: tokio::sync::oneshot::Sender<Option<i64>>,
    ) {
        let pool = &self.inner.pool;
        let run_id = match store::insert_run(pool, date, "manual", person.id, Utc::now()).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(error = %e, "cannot store the run");
                let _ = started.send(None);
                return;
            }
        };
        let recorded = self
            .inner
            .journal
            .pick(&person)
            .record(
                &person,
                "schedule",
                "collection",
                "run_now",
                None,
                Some(json!({ "date": date, "run_id": run_id })),
            )
            .await;
        let _ = started.send(Some(run_id));
        match recorded {
            Ok(_) if date < self.today() => {
                self.close_rank_steps(run_id, date).await;
            }
            Ok(_) => {
                self.run_steps(run_id, date).await;
            }
            Err(e) => {
                let text = format!("journal: {e}");
                let _ = store::finish_run(pool, run_id, "failed", Some(&text), Utc::now()).await;
            }
        }
    }

    /// Today's scheduled run with its row; the caller holds the data lock.
    async fn run_scheduled_locked(&self, date: NaiveDate) -> Result<Overall, String> {
        let run_id = store::insert_run(&self.inner.pool, date, "scheduled", None, Utc::now())
            .await
            .map_err(|e| e.to_string())?;
        Ok(self.run_steps(run_id, date).await)
    }

    /// The steps, in order, then the run's final status.
    async fn run_steps(&self, run_id: i64, date: NaiveDate) -> Overall {
        let p = &Person::SYSTEM;
        let i = &self.inner;
        let snapshots = i.snapshots.pick(p);
        let mut rec = Recorder {
            pool: &i.pool,
            run_id,
            seq: 0,
            worst: Overall::Ok,
            first_error: None,
        };

        // 1. open the snapshot (a closed date opens again)
        let t = Utc::now();
        match snapshots.open(p, date).await {
            Ok(_) => rec.step("open", t, Outcome::ok(), true).await,
            Err(e) => {
                rec.step("open", t, Outcome::failed(e), true).await;
                return self.finish(rec).await;
            }
        }

        // 2. «snapshot opened»: other blocks add their members first
        let t = Utc::now();
        let outcome = match i.events.emit(&SnapshotOpened { date }).await {
            Ok(()) => Outcome::ok(),
            Err(e) => Outcome::failed(e),
        };
        rec.step("event", t, outcome, false).await;

        // 3. KOL Explorer; a failure copies the top-100 members from the previous date
        let t = Utc::now();
        let mut outcome = match i.kolexplorer.pick(p).run(p, date).await {
            Ok(run) if run.status == KeRunStatus::Ok => Outcome {
                status: "ok",
                rows: Some(run.rows as i32),
                error: None,
                note: (run.profiles_failed > 0)
                    .then(|| format!("{} profile reads failed", run.profiles_failed)),
            },
            Ok(run) => Outcome {
                rows: Some(run.rows as i32),
                ..Outcome::failed(run.error.unwrap_or_else(|| "run failed".into()))
            },
            Err(e) => Outcome::failed(e),
        };
        if outcome.status == "failed" {
            outcome.note = Some(self.carry_members(date).await);
        }
        rec.step("kolexplorer", t, outcome, false).await;

        // 4. Solana Tracker
        let t = Utc::now();
        let outcome = match i.solanatracker.pick(p).run(p, date).await {
            Ok(run) => {
                let failed = run
                    .kols
                    .iter()
                    .filter(|k| k.status == KolRunStatus::Failed)
                    .count();
                Outcome {
                    status: match run.status {
                        StRunStatus::Ok => "ok",
                        StRunStatus::Partial => "partial",
                        StRunStatus::Failed => "failed",
                    },
                    rows: Some(run.list_rows as i32),
                    error: run.failure.map(|f| f.to_string()),
                    note: (failed > 0)
                        .then(|| format!("{failed} of {} KOLs failed", run.kols.len())),
                }
            }
            Err(e) => Outcome::failed(e),
        };
        rec.step("solanatracker", t, outcome, false).await;

        // 5. close, with carry-forward
        let t = Utc::now();
        match snapshots.close(p, date).await {
            Ok(_) => rec.step("close", t, Outcome::ok(), true).await,
            Err(e) => {
                rec.step("close", t, Outcome::failed(e), true).await;
                return self.finish(rec).await;
            }
        }

        // 6. rank the closed snapshot
        let t = Utc::now();
        let outcome = match i.score.pick(p).rank(p, date).await {
            Ok(()) => Outcome::ok(),
            Err(e) => Outcome::failed(e),
        };
        rec.step("rank", t, outcome, true).await;
        self.finish(rec).await
    }

    async fn finish(&self, rec: Recorder<'_>) -> Overall {
        let result = store::finish_run(
            rec.pool,
            rec.run_id,
            rec.worst.as_str(),
            rec.first_error.as_deref(),
            Utc::now(),
        )
        .await;
        if let Err(e) = result {
            tracing::error!(run = rec.run_id, error = %e, "cannot store the run result");
        }
        rec.worst
    }

    /// After a failed KOL Explorer read: copies the top-100 members from the previous date,
    /// but only when the date has none. Returns the note for the step.
    async fn carry_members(&self, date: NaiveDate) -> String {
        let p = &Person::SYSTEM;
        let snapshots = self.inner.snapshots.pick(p);
        let view = match snapshots.snapshot(p, date).await {
            Ok(v) => v,
            Err(e) => return format!("members not copied: {e}"),
        };
        let have = view
            .members
            .iter()
            .filter(|m| m.reasons.contains(&MemberReason::Top100))
            .count();
        if have > 0 {
            return format!("kept the {have} top-100 members already on {date}");
        }
        match snapshots.copy_members(p, date, MemberReason::Top100).await {
            Ok(()) => "top-100 members copied from the previous snapshot".into(),
            Err(e) => format!("members not copied: {e}"),
        }
    }

    /// At start: runs left `running` are marked interrupted, and every open snapshot older
    /// than today (also one a run left open) is closed with carry-forward and ranked.
    pub async fn recover(&self) {
        let me = self.clone();
        let locked = run_locked(
            &self.inner.pool,
            vec![LockSpec::job(JOB), LockSpec::try_once(DATA_LOCK)],
            async move { me.recover_locked().await },
        )
        .await;
        if let Err(e) = locked {
            // A run or a sync holds the lock: it owns its snapshot.
            tracing::warn!("start-up recovery skipped: {e}");
        }
    }

    async fn recover_locked(&self) {
        let p = &Person::SYSTEM;
        let i = &self.inner;
        let snapshots = i.snapshots.pick(p);
        let today = self.today();
        let mut dates = match store::mark_interrupted(&i.pool, Utc::now()).await {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(error = %e, "recovery: cannot mark interrupted runs");
                Vec::new()
            }
        };
        match snapshots.latest_date(p).await {
            Ok(Some(latest)) => {
                let mut d = latest + ChronoDuration::days(1);
                while d < today && (today - d).num_days() <= RECOVERY_DAYS {
                    dates.push(d);
                    d += ChronoDuration::days(1);
                }
            }
            Ok(None) => {}
            Err(e) => {
                self.record_failed(today, "recovery", &format!("latest snapshot: {e}"))
                    .await
            }
        }
        dates.retain(|d| *d < today);
        dates.sort();
        dates.dedup();
        for date in dates {
            let open = match snapshots.snapshot(p, date).await {
                Ok(v) => v.snapshot.status == SnapshotStatus::Open,
                Err(SnapshotsError::NoSnapshot { .. }) => false,
                Err(e) => {
                    self.record_failed(date, "recovery", &format!("snapshot {date}: {e}"))
                        .await;
                    false
                }
            };
            if open {
                self.close_and_rank(date).await;
            }
        }
    }

    async fn close_and_rank(&self, date: NaiveDate) {
        let run_id =
            match store::insert_run(&self.inner.pool, date, "recovery", None, Utc::now()).await {
                Ok(id) => id,
                Err(e) => {
                    tracing::error!(error = %e, "recovery: cannot store the run");
                    return;
                }
            };
        self.close_rank_steps(run_id, date).await;
    }

    /// Close and rank only: run `run_id` for an earlier date.
    async fn close_rank_steps(&self, run_id: i64, date: NaiveDate) -> Overall {
        let p = &Person::SYSTEM;
        let i = &self.inner;
        let mut rec = Recorder {
            pool: &i.pool,
            run_id,
            seq: 0,
            worst: Overall::Ok,
            first_error: None,
        };
        let t = Utc::now();
        match i.snapshots.pick(p).close(p, date).await {
            Ok(_) => rec.step("close", t, Outcome::ok(), true).await,
            Err(e) => {
                rec.step("close", t, Outcome::failed(e), true).await;
                return self.finish(rec).await;
            }
        }
        let t = Utc::now();
        let outcome = match i.score.pick(p).rank(p, date).await {
            Ok(()) => Outcome::ok(),
            Err(e) => Outcome::failed(e),
        };
        rec.step("rank", t, outcome, true).await;
        self.finish(rec).await
    }
}
