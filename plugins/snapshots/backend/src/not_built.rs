//! «Not built» implementation of [`api_snapshots::SnapshotsApi`]: every call answers
//! [`SnapshotsError::NotBuilt`] until the plugin's lane builds it.

use api_snapshots::{
    KolId, MemberReason, MetricRow, MetricSource, Metrics, Snapshot, SnapshotView, SnapshotsApi,
    SnapshotsError,
};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};

pub struct NotBuilt;

#[async_trait]
impl SnapshotsApi for NotBuilt {
    async fn open(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }

    async fn close(&self, _: &Person, _: NaiveDate) -> Result<Snapshot, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
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
        Err(SnapshotsError::NotBuilt)
    }

    async fn set_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
        _: &[KolId],
    ) -> Result<(), SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }

    async fn copy_members(
        &self,
        _: &Person,
        _: NaiveDate,
        _: MemberReason,
    ) -> Result<(), SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }

    async fn snapshot(&self, _: &Person, _: NaiveDate) -> Result<SnapshotView, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }

    async fn latest_date(&self, _: &Person) -> Result<Option<NaiveDate>, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }

    async fn dates(&self, _: &Person) -> Result<Vec<NaiveDate>, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }

    async fn history(
        &self,
        _: &Person,
        _: &str,
        _: NaiveDate,
        _: NaiveDate,
    ) -> Result<Vec<MetricRow>, SnapshotsError> {
        Err(SnapshotsError::NotBuilt)
    }
}
