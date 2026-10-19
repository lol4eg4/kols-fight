//! «Not built» implementation of [`api_registry::RegistryApi`]: every call answers
//! [`RegistryError::NotBuilt`] until the plugin's lane builds it.

use api_registry::{KolId, KolProfile, KolUpsert, Reach, RegistryApi, RegistryError, WalletSource};
use arena_platform::Person;
use async_trait::async_trait;
use chrono::NaiveDate;

pub struct NotBuilt;

#[async_trait]
impl RegistryApi for NotBuilt {
    async fn upsert_kol(&self, _: &Person, _: &KolUpsert) -> Result<KolId, RegistryError> {
        Err(RegistryError::NotBuilt)
    }

    async fn get_kol(&self, _: &Person, _: &str) -> Result<Option<KolProfile>, RegistryError> {
        Err(RegistryError::NotBuilt)
    }

    async fn list_kols(&self, _: &Person, _: &[KolId]) -> Result<Vec<KolProfile>, RegistryError> {
        Err(RegistryError::NotBuilt)
    }

    async fn find_by_handle(&self, _: &Person, _: &str) -> Result<Option<KolId>, RegistryError> {
        Err(RegistryError::NotBuilt)
    }

    async fn find_by_wallet(&self, _: &Person, _: &str) -> Result<Option<KolId>, RegistryError> {
        Err(RegistryError::NotBuilt)
    }

    async fn publish_wallets(
        &self,
        _: &Person,
        _: &str,
        _: Option<&str>,
        _: &[String],
        _: WalletSource,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::NotBuilt)
    }

    async fn set_reach(
        &self,
        _: &Person,
        _: &str,
        _: &Reach,
        _: &str,
        _: NaiveDate,
    ) -> Result<(), RegistryError> {
        Err(RegistryError::NotBuilt)
    }

    async fn set_formats(&self, _: &Person, _: &str, _: &[String]) -> Result<(), RegistryError> {
        Err(RegistryError::NotBuilt)
    }
}
