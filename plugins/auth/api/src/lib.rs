//! Auth API (layer 0): staff members, their roles and sessions.
//!
//! The auth plugin also fills the platform slot `platform/person_of_request`
//! ([`arena_platform::PersonOfRequest`]).

use arena_platform::{Person, Role};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[path = "../../stub/mod.rs"]
pub mod stub;

/// A signed-in staff member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staff {
    /// Staff id; the `id` of their [`Person`].
    pub id: i64,
    pub login: String,
    /// `staff` or `admin`.
    pub role: Role,
}

impl Staff {
    /// The person this staff member acts as.
    pub fn person(&self) -> Person {
        Person {
            id: Some(self.id),
            role: self.role,
        }
    }
}

/// Auth call failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// Auth is off for this app or person (stub).
    #[error("auth is off")]
    Off,
    /// The auth plugin is not built yet.
    #[error("auth is not built yet")]
    NotBuilt,
    #[error("auth: {message}")]
    Internal { message: String },
}

/// Auth operations.
#[async_trait]
pub trait AuthApi: Send + Sync {
    /// Who holds session token `session` (the `arena_session` cookie value).
    /// `None` = nobody: unknown, expired or revoked session.
    async fn who(&self, person: &Person, session: &str) -> Result<Option<Staff>, AuthError>;
}
