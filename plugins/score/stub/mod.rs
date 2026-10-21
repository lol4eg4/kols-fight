//! «Off» implementation of [`crate::ScoreApi`]: every call answers [`ScoreError::Off`].

use crate::{Metrics, Ranking, ScoreApi, ScoreCard, ScoreError};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct Stub;

#[async_trait]
impl ScoreApi for Stub {
    async fn score(&self, _: &Person, _: &Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::Off)
    }

    async fn rank(&self, _: &Person, _: NaiveDate) -> Result<(), ScoreError> {
        Err(ScoreError::Off)
    }

    async fn ranking(&self, _: &Person, _: NaiveDate) -> Result<Ranking, ScoreError> {
        Err(ScoreError::Off)
    }

    async fn ranked_dates(&self, _: &Person) -> Result<Vec<NaiveDate>, ScoreError> {
        Err(ScoreError::Off)
    }
}
