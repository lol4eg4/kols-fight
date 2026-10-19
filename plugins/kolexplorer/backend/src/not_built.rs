//! «Not built» implementation of [`api_kolexplorer::KolexplorerApi`]: every call answers
//! [`KeError::NotBuilt`] until the plugin's lane builds it.

use api_kolexplorer::{KeError, KeRun, KolexplorerApi};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct NotBuilt;

#[async_trait]
impl KolexplorerApi for NotBuilt {
    async fn run(&self, _: &Person, _: NaiveDate) -> Result<KeRun, KeError> {
        Err(KeError::NotBuilt)
    }

    async fn wallet_by_handle(&self, _: &Person, _: &str) -> Result<Option<String>, KeError> {
        Err(KeError::NotBuilt)
    }
}
