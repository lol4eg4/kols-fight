//! «Off» implementation of [`crate::SolanatrackerApi`]: every call answers [`StError::Off`].

use crate::{KolList, SolanatrackerApi, StError, StRun, StWallet, WalletSummary};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct Stub;

#[async_trait]
impl SolanatrackerApi for Stub {
    async fn run(&self, _: &Person, _: NaiveDate) -> Result<StRun, StError> {
        Err(StError::Off)
    }

    async fn kol_list(&self, _: &Person, _: NaiveDate) -> Result<Option<KolList>, StError> {
        Err(StError::Off)
    }

    async fn wallet_by_handle(&self, _: &Person, _: &str) -> Result<Option<StWallet>, StError> {
        Err(StError::Off)
    }

    async fn wallet_summary(&self, _: &Person, _: &str) -> Result<WalletSummary, StError> {
        Err(StError::Off)
    }
}
