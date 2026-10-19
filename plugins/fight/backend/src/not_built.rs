//! «Not built» implementation of [`api_fight::FightApi`]: every call answers
//! [`FightError::NotBuilt`] until the plugin's lane builds it.

use api_fight::{FightApi, FightError, FightResult, KolId};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct NotBuilt;

#[async_trait]
impl FightApi for NotBuilt {
    async fn fight(
        &self,
        _: &Person,
        _: &[KolId],
        _: Option<NaiveDate>,
    ) -> Result<FightResult, FightError> {
        Err(FightError::NotBuilt)
    }
}
