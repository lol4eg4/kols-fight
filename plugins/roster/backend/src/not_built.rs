//! «Not built» implementation of [`api_roster::RosterApi`]: every call answers
//! [`RosterError::NotBuilt`] until the plugin's lane builds it.

use api_roster::{KolId, NetworkMember, Rates, RosterApi, RosterError, XHandle};
use arena_platform::Person;
use async_trait::async_trait;

pub struct NotBuilt;

#[async_trait]
impl RosterApi for NotBuilt {
    async fn members(&self, _: &Person) -> Result<Vec<NetworkMember>, RosterError> {
        Err(RosterError::NotBuilt)
    }

    async fn is_member(&self, _: &Person, _: &str) -> Result<bool, RosterError> {
        Err(RosterError::NotBuilt)
    }

    async fn verified_handle(&self, _: &Person, _: &str) -> Result<Option<XHandle>, RosterError> {
        Err(RosterError::NotBuilt)
    }

    async fn rates(&self, _: &Person, _: &[KolId]) -> Result<Vec<Rates>, RosterError> {
        Err(RosterError::NotBuilt)
    }
}
