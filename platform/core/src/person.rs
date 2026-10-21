//! Who a request or an API call acts for, and route tags.

use axum::{extract::FromRequestParts, http::request::Parts};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

/// Role of a person. An admin has everything a staff member has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Not signed in: a client on the public page.
    Nobody,
    /// Team member.
    Staff,
    /// Team administrator.
    Admin,
}

impl Role {
    /// Name used in config rules and JSON: `nobody`, `staff`, `admin`.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Nobody => "nobody",
            Role::Staff => "staff",
            Role::Admin => "admin",
        }
    }
}

/// The person a request or an API call acts for. Every API call takes `&Person`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Person {
    /// Staff id from the auth plugin; `None` for nobody and for [`Person::SYSTEM`].
    pub id: Option<i64>,
    pub role: Role,
}

impl Person {
    /// Nobody: the public page.
    pub const NOBODY: Person = Person {
        id: None,
        role: Role::Nobody,
    };
    /// The server itself: scheduled jobs and install steps. Admin role, no id.
    pub const SYSTEM: Person = Person {
        id: None,
        role: Role::Admin,
    };

    pub fn staff(id: i64) -> Self {
        Person {
            id: Some(id),
            role: Role::Staff,
        }
    }

    pub fn admin(id: i64) -> Self {
        Person {
            id: Some(id),
            role: Role::Admin,
        }
    }

    /// Staff or admin.
    pub fn is_staff(&self) -> bool {
        matches!(self.role, Role::Staff | Role::Admin)
    }

    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin
    }
}

impl Default for Person {
    fn default() -> Self {
        Person::NOBODY
    }
}

/// Handlers take `Person` as an extractor. The server's tag guard puts it into the
/// request extensions; without it the person is nobody.
impl<S: Send + Sync> FromRequestParts<S> for Person {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts
            .extensions
            .get::<Person>()
            .copied()
            .unwrap_or_default())
    }
}

/// Audience tag of a route or screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    /// Accepts nobody.
    Public,
    /// Needs staff or admin.
    Staff,
    /// Needs admin.
    Admin,
}

impl Tag {
    pub const ALL: [Tag; 3] = [Tag::Public, Tag::Staff, Tag::Admin];

    /// Whether `person` may use a route with this tag.
    pub fn admits(self, person: &Person) -> bool {
        match self {
            Tag::Public => true,
            Tag::Staff => person.is_staff(),
            Tag::Admin => person.is_admin(),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Tag::Public => "public",
            Tag::Staff => "staff",
            Tag::Admin => "admin",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_admit_roles() {
        let staff = Person::staff(1);
        let admin = Person::admin(2);
        assert!(Tag::Public.admits(&Person::NOBODY));
        assert!(!Tag::Staff.admits(&Person::NOBODY));
        assert!(Tag::Staff.admits(&staff));
        assert!(Tag::Staff.admits(&admin));
        assert!(!Tag::Admin.admits(&staff));
        assert!(Tag::Admin.admits(&admin));
        assert!(Tag::Admin.admits(&Person::SYSTEM));
    }
}
