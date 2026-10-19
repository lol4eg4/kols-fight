//! «Off» implementation of [`crate::RosterApi`]: every call answers [`RosterError::Off`].

use crate::{KolId, NetworkMember, Rates, RosterApi, RosterError, XHandle};
use arena_platform::Person;
use async_trait::async_trait;

pub struct Stub;

#[async_trait]
impl RosterApi for Stub {
    async fn members(&self, _: &Person) -> Result<Vec<NetworkMember>, RosterError> {
        Err(RosterError::Off)
    }

    async fn is_member(&self, _: &Person, _: &str) -> Result<bool, RosterError> {
        Err(RosterError::Off)
    }

    async fn verified_handle(&self, _: &Person, _: &str) -> Result<Option<XHandle>, RosterError> {
        Err(RosterError::Off)
    }

    async fn rates(&self, _: &Person, _: &[KolId]) -> Result<Vec<Rates>, RosterError> {
        Err(RosterError::Off)
    }
}
