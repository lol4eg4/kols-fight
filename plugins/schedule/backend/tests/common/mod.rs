//! Stand-ins for the APIs schedule calls, with one shared call log.
#![allow(dead_code)]

use api_journal::{Entry, EntryId, JournalApi, JournalError};
use api_kolexplorer::{KeError, KeRun, KeRunStatus, KolexplorerApi};
use api_score::{Ranking, ScoreApi, ScoreCard, ScoreError};
use api_snapshots::{
    Member, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotStatus, SnapshotView,
    SnapshotsApi, SnapshotsError,
};
use api_solanatracker::{
    KolList, SolanatrackerApi, StError, StFailure, StRun, StRunStatus, StWallet, WalletSummary,
};
use arena_platform::{Api, Apis, Events, Person, Registry};
use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use plugin_schedule::{Options, Schedule};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration as StdDuration,
};

pub type Log = Arc<Mutex<Vec<String>>>;

/// (person id, block/object, action, after)
pub type JournalEntry = (Option<i64>, String, String, Option<Value>);

pub fn today() -> NaiveDate {
    Utc::now().date_naive()
}

pub fn days_ago(n: i64) -> NaiveDate {
    today() - Duration::days(n)
}

// ------------------------------------------------------------------ snapshots

#[derive(Default)]
pub struct SnapState {
    pub status: BTreeMap<NaiveDate, SnapshotStatus>,
    pub members: BTreeMap<NaiveDate, Vec<(String, MemberReason)>>,
}

pub struct FakeSnapshots {
    pub log: Log,
    pub state: Mutex<SnapState>,
    pub fail_close: Mutex<bool>,
}

impl FakeSnapshots {
    pub fn seed(&self, date: NaiveDate, status: SnapshotStatus, members: &[(&str, MemberReason)]) {
        let mut s = self.state.lock().unwrap();
        s.status.insert(date, status);
        s.members.insert(
            date,
            members.iter().map(|(k, r)| (k.to_string(), *r)).collect(),
        );
    }

    pub fn members_of(&self, date: NaiveDate) -> Vec<String> {
        let s = self.state.lock().unwrap();
        s.members
            .get(&date)
            .map(|m| m.iter().map(|(k, _)| k.clone()).collect())
            .unwrap_or_default()
    }

    pub fn status_of(&self, date: NaiveDate) -> Option<SnapshotStatus> {
        self.state.lock().unwrap().status.get(&date).copied()
    }
}

fn snap(date: NaiveDate, status: SnapshotStatus) -> Snapshot {
    Snapshot {
        date,
        status,
        opened_at: Utc::now(),
        closed_at: (status == SnapshotStatus::Closed).then(Utc::now),
    }
}

fn nope(what: &str) -> SnapshotsError {
    SnapshotsError::Internal {
        message: format!("{what} is not used by schedule"),
    }
}

#[async_trait]
impl SnapshotsApi for FakeSnapshots {
    async fn open(&self, _: &Person, date: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        self.log.lock().unwrap().push(format!("open {date}"));
        let mut s = self.state.lock().unwrap();
        s.status.insert(date, SnapshotStatus::Open);
        s.members.entry(date).or_default();
        Ok(snap(date, SnapshotStatus::Open))
    }

    async fn close(&self, _: &Person, date: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        self.log.lock().unwrap().push(format!("close {date}"));
        if *self.fail_close.lock().unwrap() {
            return Err(SnapshotsError::Internal {
                message: "disk full".into(),
            });
        }
        self.state
            .lock()
            .unwrap()
            .status
            .insert(date, SnapshotStatus::Closed);
        Ok(snap(date, SnapshotStatus::Closed))
    }

    async fn put_metrics(
        &self,
        _: &Person,
        _: NaiveDate,
        _: &str,
        _: &Metrics,
        _: MetricSource,
        _: DateTime<Utc>,
    ) -> Result<(), SnapshotsError> {
        Err(nope("put_metrics"))
    }

    async fn set_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
        _: &[String],
    ) -> Result<(), SnapshotsError> {
        Err(nope("set_members"))
    }

    async fn copy_members(
        &self,
        _: &Person,
        date: NaiveDate,
        reason: MemberReason,
    ) -> Result<(), SnapshotsError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("copy_members {date}"));
        let mut s = self.state.lock().unwrap();
        let prev = s
            .members
            .range(..date)
            .next_back()
            .map(|(_, m)| m.clone())
            .unwrap_or_default();
        let into = s.members.entry(date).or_default();
        for (k, r) in prev {
            if r == reason {
                into.push((k, r));
            }
        }
        Ok(())
    }

    async fn snapshot(&self, _: &Person, date: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        let s = self.state.lock().unwrap();
        let Some(status) = s.status.get(&date).copied() else {
            return Err(SnapshotsError::NoSnapshot { date });
        };
        let members = s
            .members
            .get(&date)
            .map(|m| {
                m.iter()
                    .map(|(k, r)| Member {
                        kol: k.clone(),
                        reasons: vec![*r],
                        row: None::<MetricRow>,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(SnapshotView {
            snapshot: snap(date, status),
            members,
        })
    }

    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        let s = self.state.lock().unwrap();
        Ok(s.status
            .iter()
            .filter(|(_, st)| **st == SnapshotStatus::Closed)
            .map(|(d, _)| *d)
            .next_back())
    }

    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Err(nope("dates"))
    }

    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        Err(nope("history"))
    }
}

// ------------------------------------------------------------ the two sources

pub struct FakeKe {
    pub log: Log,
    pub result: Mutex<Result<KeRunStatus, String>>,
}

#[async_trait]
impl KolexplorerApi for FakeKe {
    async fn run(&self, _: &Person, date: NaiveDate) -> Result<KeRun, KeError> {
        self.log.lock().unwrap().push("kolexplorer".into());
        let now = Utc::now();
        match self.result.lock().unwrap().clone() {
            Ok(KeRunStatus::Ok) => Ok(KeRun {
                date,
                started_at: now,
                finished_at: now,
                status: KeRunStatus::Ok,
                rows: 100,
                profiles_failed: 2,
                error: None,
            }),
            Ok(KeRunStatus::Failed) => Ok(KeRun {
                date,
                started_at: now,
                finished_at: now,
                status: KeRunStatus::Failed,
                rows: 0,
                profiles_failed: 0,
                error: Some("only 12 rows on the page".into()),
            }),
            Err(m) => Err(KeError::Internal { message: m }),
        }
    }

    async fn wallet_by_handle(&self, _: &Person, _: &str) -> Result<Option<String>, KeError> {
        Ok(None)
    }
}

pub struct FakeSt {
    pub log: Log,
    pub result: Mutex<(StRunStatus, Option<StFailure>)>,
}

#[async_trait]
impl SolanatrackerApi for FakeSt {
    async fn run(&self, _: &Person, date: NaiveDate) -> Result<StRun, StError> {
        self.log.lock().unwrap().push("solanatracker".into());
        let now = Utc::now();
        let (status, failure) = self.result.lock().unwrap().clone();
        Ok(StRun {
            date,
            started_at: now,
            finished_at: now,
            status,
            failure,
            kols: vec![],
            list_rows: 14,
            calls: 20,
        })
    }

    async fn kol_list(&self, _: &Person, _: NaiveDate) -> Result<Option<KolList>, StError> {
        Ok(None)
    }

    async fn wallet_by_handle(&self, _: &Person, _: &str) -> Result<Option<StWallet>, StError> {
        Ok(None)
    }

    async fn wallet_summary(&self, _: &Person, _: &str) -> Result<WalletSummary, StError> {
        Err(StError::NotBuilt)
    }
}

// ------------------------------------------------------------ score, journal

pub struct FakeScore {
    pub log: Log,
}

#[async_trait]
impl ScoreApi for FakeScore {
    async fn score(&self, _: &Person, _: &Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::NotBuilt)
    }

    async fn rank(&self, _: &Person, date: NaiveDate) -> Result<(), ScoreError> {
        self.log.lock().unwrap().push(format!("rank {date}"));
        Ok(())
    }

    async fn ranking(&self, _: &Person, _: NaiveDate) -> Result<Ranking, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
}

pub struct FakeJournal {
    pub log: Log,
    pub entries: Mutex<Vec<JournalEntry>>,
    pub fail: Mutex<bool>,
}

#[async_trait]
impl JournalApi for FakeJournal {
    async fn record(
        &self,
        person: &Person,
        block: &str,
        object: &str,
        action: &str,
        _: Option<Value>,
        after: Option<Value>,
    ) -> Result<EntryId, JournalError> {
        if *self.fail.lock().unwrap() {
            return Err(JournalError::Internal {
                message: "journal down".into(),
            });
        }
        self.log.lock().unwrap().push("journal".into());
        self.entries.lock().unwrap().push((
            person.id,
            format!("{block}/{object}"),
            action.to_string(),
            after,
        ));
        Ok(1)
    }

    async fn history(&self, _: &Person, _: &str, _: &str) -> Result<Vec<Entry>, JournalError> {
        Ok(vec![])
    }
}

// ------------------------------------------------------------------- the rig

pub struct Fakes {
    pub log: Log,
    pub snapshots: Arc<FakeSnapshots>,
    pub ke: Arc<FakeKe>,
    pub st: Arc<FakeSt>,
    pub score: Arc<FakeScore>,
    pub journal: Arc<FakeJournal>,
    pub events: Events,
}

impl Fakes {
    pub fn new() -> Self {
        let log: Log = Arc::default();
        Fakes {
            snapshots: Arc::new(FakeSnapshots {
                log: log.clone(),
                state: Mutex::default(),
                fail_close: Mutex::new(false),
            }),
            ke: Arc::new(FakeKe {
                log: log.clone(),
                result: Mutex::new(Ok(KeRunStatus::Ok)),
            }),
            st: Arc::new(FakeSt {
                log: log.clone(),
                result: Mutex::new((StRunStatus::Ok, None)),
            }),
            score: Arc::new(FakeScore { log: log.clone() }),
            journal: Arc::new(FakeJournal {
                log: log.clone(),
                entries: Mutex::default(),
                fail: Mutex::new(false),
            }),
            events: Events::default(),
            log,
        }
    }

    /// A «snapshot opened» listener that writes to the call log, like roster would.
    pub fn listen(&self) {
        let log = self.log.clone();
        self.events
            .on::<api_schedule::SnapshotOpened, _, _>("roster", move |e| {
                let log = log.clone();
                async move {
                    log.lock().unwrap().push(format!("event {}", e.date));
                    Ok(())
                }
            });
    }

    pub fn schedule(&self, pool: sqlx::PgPool, data_wait: StdDuration) -> Schedule {
        let apis = Apis::new(Registry::new("test", &[]));
        apis.provide::<dyn SnapshotsApi>(
            "snapshots",
            vec![("default", self.snapshots.clone() as Arc<dyn SnapshotsApi>)],
            Arc::new(api_snapshots::stub::Stub),
        )
        .unwrap();
        apis.provide::<dyn KolexplorerApi>(
            "kolexplorer",
            vec![("default", self.ke.clone() as Arc<dyn KolexplorerApi>)],
            Arc::new(api_kolexplorer::stub::Stub),
        )
        .unwrap();
        apis.provide::<dyn SolanatrackerApi>(
            "solanatracker",
            vec![("default", self.st.clone() as Arc<dyn SolanatrackerApi>)],
            Arc::new(api_solanatracker::stub::Stub),
        )
        .unwrap();
        apis.provide::<dyn ScoreApi>(
            "score",
            vec![("default", self.score.clone() as Arc<dyn ScoreApi>)],
            Arc::new(api_score::stub::Stub),
        )
        .unwrap();
        apis.provide::<dyn JournalApi>(
            "journal",
            vec![("default", self.journal.clone() as Arc<dyn JournalApi>)],
            Arc::new(api_journal::stub::Stub),
        )
        .unwrap();
        fn get<T: ?Sized + Send + Sync + 'static>(a: &Apis) -> Api<T> {
            a.get().unwrap()
        }
        Schedule::new(
            pool,
            self.events.clone(),
            get(&apis),
            get(&apis),
            get(&apis),
            get(&apis),
            get(&apis),
            Options { data_wait },
        )
    }

    pub fn calls(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}
