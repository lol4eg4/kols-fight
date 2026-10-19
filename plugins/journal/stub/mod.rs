//! «Off» implementation of [`crate::JournalApi`]: every call answers [`JournalError::Off`].

use crate::{Entry, EntryId, JournalApi, JournalError};
use arena_platform::Person;
use async_trait::async_trait;
use serde_json::Value;

pub struct Stub;

#[async_trait]
impl JournalApi for Stub {
    async fn record(
        &self,
        _person: &Person,
        _block: &str,
        _object: &str,
        _action: &str,
        _before: Option<Value>,
        _after: Option<Value>,
    ) -> Result<EntryId, JournalError> {
        Err(JournalError::Off)
    }

    async fn history(
        &self,
        _person: &Person,
        _block: &str,
        _object: &str,
    ) -> Result<Vec<Entry>, JournalError> {
        Err(JournalError::Off)
    }
}
