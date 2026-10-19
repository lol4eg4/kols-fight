//! «Off» implementation of [`crate::AuthApi`]: every call answers [`AuthError::Off`].

use crate::{AuthApi, AuthError, Staff};
use arena_platform::Person;
use async_trait::async_trait;

pub struct Stub;

#[async_trait]
impl AuthApi for Stub {
    async fn who(&self, _person: &Person, _session: &str) -> Result<Option<Staff>, AuthError> {
        Err(AuthError::Off)
    }
}
