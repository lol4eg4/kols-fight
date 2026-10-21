//! Fake snapshots for the roster tests.
#![allow(dead_code)]

use api_snapshots::{
    MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotView, SnapshotsApi,
    SnapshotsError,
};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use std::sync::Mutex;

#[derive(Default)]
pub struct FakeSnapshots {
    pub members: Mutex<Vec<(NaiveDate, MemberReason, Vec<String>)>>,
}

fn unused() -> SnapshotsError {
    SnapshotsError::Internal {
        message: "not used in this test".into(),
    }
}

#[async_trait]
impl SnapshotsApi for FakeSnapshots {
    async fn open(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(unused())
    }

    async fn close(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(unused())
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
        Err(unused())
    }

    async fn set_members(
        &self,
        _: &Person,
        date: NaiveDate,
        reason: MemberReason,
        kols: &[String],
    ) -> Result<(), SnapshotsError> {
        self.members
            .lock()
            .unwrap()
            .push((date, reason, kols.to_vec()));
        Ok(())
    }

    async fn copy_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
    ) -> Result<(), SnapshotsError> {
        Err(unused())
    }

    async fn snapshot(&self, _: &Person, _: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        Err(unused())
    }

    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Err(unused())
    }

    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Err(unused())
    }

    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        Err(unused())
    }
}
