//! «Not built» implementation of [`api_score::ScoreApi`]: every call answers
//! [`ScoreError::NotBuilt`] until the plugin's lane builds it.

use api_score::{Metrics, Ranking, ScoreApi, ScoreCard, ScoreError};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct NotBuilt;

#[async_trait]
impl ScoreApi for NotBuilt {
    async fn score(&self, _: &Person, _: &Metrics) -> Result<ScoreCard, ScoreError> {
        Err(ScoreError::NotBuilt)
    }

    async fn rank(&self, _: &Person, _: NaiveDate) -> Result<(), ScoreError> {
        Err(ScoreError::NotBuilt)
    }

    async fn ranking(&self, _: &Person, _: NaiveDate) -> Result<Ranking, ScoreError> {
        Err(ScoreError::NotBuilt)
    }
}
