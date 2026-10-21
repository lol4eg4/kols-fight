//! «Not built» implementation of [`api_wallets::WalletsApi`]: every call answers
//! [`WalletsError::NotBuilt`] until the plugin's lane builds it.

use api_wallets::{Candidate, WalletStatus, WalletsApi, WalletsError};
use arena_platform::Person;
use async_trait::async_trait;

pub struct NotBuilt;

#[async_trait]
impl WalletsApi for NotBuilt {
    async fn propose(&self, _: &Person, _: &str, _: &str, _: &str) -> Result<Candidate, WalletsError> {
        Err(WalletsError::NotBuilt)
    }

    async fn status(&self, _: &Person, _: &str) -> Result<WalletStatus, WalletsError> {
        Err(WalletsError::NotBuilt)
    }
}
