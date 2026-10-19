//! «Not built» implementation of [`api_journal::JournalApi`]: every call answers
//! [`JournalError::NotBuilt`] until the plugin's lane builds it.

use api_journal::{Entry, EntryId, JournalApi, JournalError};
use arena_platform::Person;
use async_trait::async_trait;
use serde_json::Value;

pub struct NotBuilt;

#[async_trait]
impl JournalApi for NotBuilt {
    async fn record(
        &self,
        _person: &Person,
        _block: &str,
        _object: &str,
        _action: &str,
        _before: Option<Value>,
        _after: Option<Value>,
    ) -> Result<EntryId, JournalError> {
        Err(JournalError::NotBuilt)
    }

    async fn history(
        &self,
        _person: &Person,
        _block: &str,
        _object: &str,
    ) -> Result<Vec<Entry>, JournalError> {
        Err(JournalError::NotBuilt)
    }
}
