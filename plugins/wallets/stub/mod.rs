//! «Off» implementation of [`crate::WalletsApi`]: every call answers [`WalletsError::Off`].

use crate::{Candidate, WalletStatus, WalletsApi, WalletsError};
use arena_platform::Person;
use async_trait::async_trait;

pub struct Stub;

#[async_trait]
impl WalletsApi for Stub {
    async fn propose(
        &self,
        _: &Person,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<Candidate, WalletsError> {
        Err(WalletsError::Off)
    }

    async fn status(&self, _: &Person, _: &str) -> Result<WalletStatus, WalletsError> {
        Err(WalletsError::Off)
    }
}
