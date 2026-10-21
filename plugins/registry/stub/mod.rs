//! «Off» implementation of [`crate::RegistryApi`]: every call answers [`RegistryError::Off`].

use crate::{KolId, KolProfile, KolUpsert, Reach, RegistryApi, RegistryError, WalletSource};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct Stub;

#[async_trait]
impl RegistryApi for Stub {
    async fn upsert_kol(&self, _: &Person, _: &KolUpsert) -> Result<KolId, RegistryError> {
        Err(RegistryError::Off)
    }

    async fn get_kol(&self, _: &Person, _: &str) -> Result<Option<KolProfile>, RegistryError> {
        Err(RegistryError::Off)
    }

    async fn list_kols(&self, _: &Person, _: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        Err(RegistryError::Off)
    }

    async fn find_by_handle(&self, _: &Person, _: &str) -> Result<Option<KolId>, RegistryError> {
        Err(RegistryError::Off)
    }

    async fn find_by_wallet(&self, _: &Person, _: &str) -> Result<Option<KolId>, RegistryError> {
        Err(RegistryError::Off)
    }

    async fn publish_wallets(
        &self,
        _: &Person,
        _: &str,
        _: Option<(&str, WalletSource)>,
        _: &[(String, WalletSource)],
    ) -> Result<(), RegistryError> {
        Err(RegistryError::Off)
    }

    async fn set_reach(
        &self,
        _: &Person,
        _: &str,
        _: &Reach,
        _: &str,
        _: NaiveDate,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::Off)
    }

    async fn set_formats(&self, _: &Person, _: &str, _: &[String]) -> Result<(), RegistryError> {
        Err(RegistryError::Off)
    }
}
