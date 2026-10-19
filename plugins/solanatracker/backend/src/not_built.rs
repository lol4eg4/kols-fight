//! «Not built» implementation of [`api_solanatracker::SolanatrackerApi`]: every call answers
//! [`StError::NotBuilt`] until the plugin's lane builds it.

use api_solanatracker::{KolList, SolanatrackerApi, StError, StRun, StWallet, WalletSummary};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct NotBuilt;

#[async_trait]
impl SolanatrackerApi for NotBuilt {
    async fn run(&self, _: &Person, _: NaiveDate) -> Result<StRun, StError> {
        Err(StError::NotBuilt)
    }

    async fn kol_list(&self, _: &Person, _: NaiveDate) -> Result<Option<KolList>, StError> {
        Err(StError::NotBuilt)
    }

    async fn wallet_by_handle(&self, _: &Person, _: &str) -> Result<Option<StWallet>, StError> {
        Err(StError::NotBuilt)
    }

    async fn wallet_summary(&self, _: &Person, _: &str) -> Result<WalletSummary, StError> {
        Err(StError::NotBuilt)
    }
}
