//! «Off» implementation of [`crate::KolexplorerApi`]: every call answers [`KeError::Off`].

use crate::{KeError, KeRun, KolexplorerApi};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct Stub;

#[async_trait]
impl KolexplorerApi for Stub {
    async fn run(&self, _: &Person, _: NaiveDate) -> Result<KeRun, KeError> {
        Err(KeError::Off)
    }

    async fn wallet_by_handle(&self, _: &Person, _: &str) -> Result<Option<String>, KeError> {
        Err(KeError::Off)
    }
}
