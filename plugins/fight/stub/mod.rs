//! «Off» implementation of [`crate::FightApi`]: every call answers [`FightError::Off`].

use crate::{FightApi, FightError, FightResult, KolId};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct Stub;

#[async_trait]
impl FightApi for Stub {
    async fn fight(
        &self,
        _: &Person,
        _: &[KolId],
        _: Option<NaiveDate>,
    ) -> Result<FightResult, FightError> {
        Err(FightError::Off)
    }
}
