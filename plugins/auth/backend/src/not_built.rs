//! «Not built» implementation of [`api_auth::AuthApi`]: every call answers
//! [`AuthError::NotBuilt`] until the plugin's lane builds it.

use api_auth::{AuthApi, AuthError, Staff};
use arena_platform::Person;
use async_trait::async_trait;

pub struct NotBuilt;

#[async_trait]
impl AuthApi for NotBuilt {
    async fn who(&self, _person: &Person, _session: &str) -> Result<Option<Staff>, AuthError> {
        Err(AuthError::NotBuilt)
    }
}
