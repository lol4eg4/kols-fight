//! «Off» implementation of [`crate::SnapshotsApi`]: every call answers [`SnapshotsError::Off`].

use crate::{
    KolId, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotView, SnapshotsApi,
    SnapshotsError,
};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};

pub struct Stub;

#[async_trait]
impl SnapshotsApi for Stub {
    async fn open(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(SnapshotsError::Off)
    }

    async fn close(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(SnapshotsError::Off)
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
        Err(SnapshotsError::Off)
    }

    async fn set_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
        _: &[KolId],
    ) -> Result<(), SnapshotsError> {
        Err(SnapshotsError::Off)
    }

    async fn copy_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
    ) -> Result<(), SnapshotsError> {
        Err(SnapshotsError::Off)
    }

    async fn snapshot(&self, _: &Person, _: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        Err(SnapshotsError::Off)
    }

    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Err(SnapshotsError::Off)
    }

    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Err(SnapshotsError::Off)
    }

    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        Err(SnapshotsError::Off)
    }
}
