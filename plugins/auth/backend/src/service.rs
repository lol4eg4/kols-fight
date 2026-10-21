//! Auth rules: password hashes, sessions, the login rate limit and staff management.
//! No HTTP here; [`crate::http`] maps these results to responses.

use api_auth::Staff;
use api_journal::JournalApi;
use arena_platform::{Api, Person, Role};
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::{
        SaltString,
        rand_core::{OsRng, RngCore},
    },
};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::sync::Arc;

/// Block name in the journal.
const BLOCK: &str = "auth";

/// Session lifetime.
pub const SESSION_DAYS: i64 = 14;
/// Failed logins from one address, inside [`WINDOW_MINUTES`], that lock it.
pub const MAX_FAILURES: i32 = 5;
/// Window in which failures count.
pub const WINDOW_MINUTES: i64 = 15;
/// How long an address stays locked.
pub const LOCK_MINUTES: i64 = 15;
/// Shortest password an admin may set for a new staff member.
pub const MIN_PASSWORD: usize = 10;
/// Longest password (argon2 work grows with input; this bounds it).
pub const MAX_PASSWORD: usize = 128;

/// Time source; tests move it.
pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// Wall clock.
pub fn system_clock() -> Clock {
    Arc::new(Utc::now)
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoginError {
    /// Unknown login, wrong password or revoked. One answer for all: no hint.
    BadCredentials,
    /// The address is locked; minutes left, at least 1.
    Locked {
        minutes: i64,
    },
    Internal(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum StaffError {
    /// Input the admin can fix; the text is shown to them.
    Invalid(&'static str),
    /// Login already used (also by revoked staff).
    Taken,
    NotFound,
    /// An admin cannot revoke themselves.
    SelfRevoke,
    AlreadyRevoked,
    /// The change was saved but the journal refused it.
    JournalFailed,
    Internal(String),
}

impl From<sqlx::Error> for LoginError {
    fn from(e: sqlx::Error) -> Self {
        LoginError::Internal(e.to_string())
    }
}

impl From<sqlx::Error> for StaffError {
    fn from(e: sqlx::Error) -> Self {
        StaffError::Internal(e.to_string())
    }
}

/// One row of the staff list. Never carries the hash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StaffRow {
    pub id: i64,
    pub login: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub struct AuthService {
    pool: PgPool,
    journal: Api<dyn JournalApi>,
    clock: Clock,
    params: Params,
    /// Verified when the login is unknown, so both paths cost the same.
    dummy_hash: String,
}

/// Login: 3 to 64 characters of a-z, 0-9 and `. _ - @`, after trim and lower-casing.
pub fn normalize_login(raw: &str) -> Result<String, &'static str> {
    let login = raw.trim().to_lowercase();
    let ok_len = (3..=64).contains(&login.chars().count());
    let ok_chars = login.chars().all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | '@')
    });
    if ok_len && ok_chars {
        Ok(login)
    } else {
        Err("Login must be 3 to 64 characters: letters, digits, . _ - @")
    }
}

pub fn parse_role(raw: &str) -> Option<Role> {
    match raw {
        "staff" => Some(Role::Staff),
        "admin" => Some(Role::Admin),
        _ => None,
    }
}

fn role_str(role: Role) -> &'static str {
    role.as_str()
}

fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 32 random bytes from the OS, as 64 hex characters.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex(&bytes)
}

fn token_shape_ok(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Argon2id PHC string for `password`.
pub fn hash_password(params: &Params, password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params.clone())
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Whether `password` matches the PHC string `stored`. The parameters come from `stored`.
pub fn verify_password(stored: &str, password: &str) -> bool {
    match PasswordHash::new(stored) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

impl AuthService {
    /// Service with production argon2 parameters and the wall clock.
    pub fn new(pool: PgPool, journal: Api<dyn JournalApi>) -> Self {
        Self::with(pool, journal, system_clock(), Params::DEFAULT)
    }

    /// Service with a given clock and argon2 parameters (tests use cheap ones).
    pub fn with(pool: PgPool, journal: Api<dyn JournalApi>, clock: Clock, params: Params) -> Self {
        let dummy_hash =
            hash_password(&params, &new_token()).expect("argon2 hashes with valid parameters");
        AuthService {
            pool,
            journal,
            clock,
            params,
            dummy_hash,
        }
    }

    fn now(&self) -> DateTime<Utc> {
        (self.clock)()
    }

    async fn hash(&self, password: &str) -> Result<String, String> {
        let (params, password) = (self.params.clone(), password.to_owned());
        tokio::task::spawn_blocking(move || hash_password(&params, &password))
            .await
            .map_err(|e| e.to_string())?
    }

    async fn verify(&self, stored: String, password: &str) -> bool {
        let password = password.to_owned();
        tokio::task::spawn_blocking(move || verify_password(&stored, &password))
            .await
            .unwrap_or(false)
    }

    async fn journal(
        &self,
        actor: &Person,
        object: i64,
        action: &str,
        before: Option<Value>,
        after: Option<Value>,
    ) -> bool {
        match self
            .journal
            .pick(actor)
            .record(actor, BLOCK, &object.to_string(), action, before, after)
            .await
        {
            Ok(_) => true,
            Err(e) => {
                tracing::error!(action, error = %e, "auth: journal write failed");
                false
            }
        }
    }

    /// Minutes left on a lock for `ip`, if it is locked now.
    async fn locked_minutes(
        &self,
        ip: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<i64>, sqlx::Error> {
        let until: Option<Option<DateTime<Utc>>> =
            sqlx::query_scalar("SELECT locked_until FROM auth.login_limit WHERE ip = $1")
                .bind(ip)
                .fetch_optional(&self.pool)
                .await?;
        Ok(until.flatten().filter(|u| *u > now).map(|u| {
            let secs = (u - now).num_seconds();
            ((secs + 59) / 60).max(1)
        }))
    }

    /// Counts a failed login; the fifth inside the window locks the address.
    async fn record_failure(&self, ip: &str, now: DateTime<Utc>) -> Result<(), sqlx::Error> {
        let window_start = now - Duration::minutes(WINDOW_MINUTES);
        let failures: i32 = sqlx::query_scalar(
            "INSERT INTO auth.login_limit AS l (ip, failures, window_start) VALUES ($1, 1, $2) \
             ON CONFLICT (ip) DO UPDATE SET \
               failures = CASE WHEN l.window_start <= $3 THEN 1 ELSE l.failures + 1 END, \
               window_start = CASE WHEN l.window_start <= $3 THEN $2 ELSE l.window_start END \
             RETURNING failures",
        )
        .bind(ip)
        .bind(now)
        .bind(window_start)
        .fetch_one(&self.pool)
        .await?;
        if failures >= MAX_FAILURES {
            sqlx::query(
                "UPDATE auth.login_limit SET locked_until = $2, failures = 0, window_start = $3 \
                 WHERE ip = $1",
            )
            .bind(ip)
            .bind(now + Duration::minutes(LOCK_MINUTES))
            .bind(now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Checks login and password for a request from `ip`. On success returns the staff
    /// member and a new session token (the cookie value).
    pub async fn login(
        &self,
        ip: &str,
        login: &str,
        password: &str,
    ) -> Result<(Staff, String), LoginError> {
        let now = self.now();
        if let Some(minutes) = self.locked_minutes(ip, now).await? {
            return Err(LoginError::Locked { minutes });
        }
        let login = login.trim().to_lowercase();
        // An absurd password is a failure without any hashing work.
        let row = if password.len() > MAX_PASSWORD * 4 {
            None
        } else {
            sqlx::query(
                "SELECT id, password_hash, role FROM auth.staff \
                 WHERE login = $1 AND revoked_at IS NULL",
            )
            .bind(&login)
            .fetch_optional(&self.pool)
            .await?
        };
        let good = if password.len() > MAX_PASSWORD * 4 {
            false
        } else {
            let stored = match &row {
                Some(r) => r.try_get::<String, _>("password_hash")?,
                None => self.dummy_hash.clone(),
            };
            self.verify(stored, password).await
        };
        let Some(row) = row.filter(|_| good) else {
            self.record_failure(ip, now).await?;
            // Old rows of idle addresses.
            sqlx::query(
                "DELETE FROM auth.login_limit WHERE window_start < $1 \
                 AND (locked_until IS NULL OR locked_until < $1)",
            )
            .bind(now - Duration::hours(1))
            .execute(&self.pool)
            .await?;
            return Err(LoginError::BadCredentials);
        };
        let staff = Staff {
            id: row.try_get("id")?,
            login,
            role: parse_role(&row.try_get::<String, _>("role")?).unwrap_or(Role::Staff),
        };
        let token = new_token();
        sqlx::query("DELETE FROM auth.session WHERE expires_at <= $1")
            .bind(now)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO auth.session (token_hash, staff_id, created_at, expires_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(token_hash(&token))
        .bind(staff.id)
        .bind(now)
        .bind(now + Duration::days(SESSION_DAYS))
        .execute(&self.pool)
        .await?;
        self.journal(
            &staff.person(),
            staff.id,
            "login",
            None,
            Some(json!({ "login": staff.login })),
        )
        .await;
        Ok((staff, token))
    }

    /// Who holds this session token: `None` when unknown, expired, or revoked.
    pub async fn who(&self, token: &str) -> Result<Option<Staff>, sqlx::Error> {
        if !token_shape_ok(token) {
            return Ok(None);
        }
        let row = sqlx::query(
            "SELECT st.id, st.login, st.role FROM auth.session se \
             JOIN auth.staff st ON st.id = se.staff_id \
             WHERE se.token_hash = $1 AND se.expires_at > $2 AND st.revoked_at IS NULL",
        )
        .bind(token_hash(token))
        .bind(self.now())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| {
            Ok(Staff {
                id: r.try_get("id")?,
                login: r.try_get("login")?,
                role: parse_role(&r.try_get::<String, _>("role")?).unwrap_or(Role::Staff),
            })
        })
        .transpose()
    }

    /// Ends the session behind `token`.
    pub async fn logout(&self, who: &Staff, token: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM auth.session WHERE token_hash = $1")
            .bind(token_hash(token))
            .execute(&self.pool)
            .await?;
        self.journal(&who.person(), who.id, "logout", None, None)
            .await;
        Ok(())
    }

    pub async fn list_staff(&self) -> Result<Vec<StaffRow>, sqlx::Error> {
        sqlx::query("SELECT id, login, role, created_at, revoked_at FROM auth.staff ORDER BY id")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| {
                Ok(StaffRow {
                    id: r.try_get("id")?,
                    login: r.try_get("login")?,
                    role: parse_role(&r.try_get::<String, _>("role")?).unwrap_or(Role::Staff),
                    created_at: r.try_get("created_at")?,
                    revoked_at: r.try_get("revoked_at")?,
                })
            })
            .collect()
    }

    /// Adds a staff member with a temporary password. Journaled without the password.
    pub async fn add_staff(
        &self,
        actor: &Person,
        login: &str,
        role: Role,
        password: &str,
    ) -> Result<StaffRow, StaffError> {
        let login = normalize_login(login).map_err(StaffError::Invalid)?;
        if !matches!(role, Role::Staff | Role::Admin) {
            return Err(StaffError::Invalid("Role must be staff or admin"));
        }
        let len = password.chars().count();
        if !(MIN_PASSWORD..=MAX_PASSWORD).contains(&len) {
            return Err(StaffError::Invalid("Password must be 10 to 128 characters"));
        }
        let hash = self.hash(password).await.map_err(StaffError::Internal)?;
        let created_at = self.now();
        let row = sqlx::query(
            "INSERT INTO auth.staff (login, password_hash, role, created_at) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (login) DO NOTHING RETURNING id",
        )
        .bind(&login)
        .bind(&hash)
        .bind(role_str(role))
        .bind(created_at)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Err(StaffError::Taken);
        };
        let id: i64 = row.try_get("id")?;
        let saved = self
            .journal(
                actor,
                id,
                "add",
                None,
                Some(json!({ "login": login, "role": role_str(role) })),
            )
            .await;
        if !saved {
            return Err(StaffError::JournalFailed);
        }
        Ok(StaffRow {
            id,
            login,
            role,
            created_at,
            revoked_at: None,
        })
    }

    /// Revokes access and deletes that person's sessions in the same transaction.
    pub async fn revoke_staff(&self, actor: &Person, id: i64) -> Result<(), StaffError> {
        if actor.id == Some(id) {
            return Err(StaffError::SelfRevoke);
        }
        let now = self.now();
        let mut tx = self.pool.begin().await?;
        let row =
            sqlx::query("SELECT login, role, revoked_at FROM auth.staff WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(row) = row else {
            return Err(StaffError::NotFound);
        };
        if row
            .try_get::<Option<DateTime<Utc>>, _>("revoked_at")?
            .is_some()
        {
            return Err(StaffError::AlreadyRevoked);
        }
        let (login, role): (String, String) = (row.try_get("login")?, row.try_get("role")?);
        sqlx::query("UPDATE auth.staff SET revoked_at = $2 WHERE id = $1")
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM auth.session WHERE staff_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        let saved = self
            .journal(
                actor,
                id,
                "revoke",
                Some(json!({ "login": login, "role": role, "revoked": false })),
                Some(json!({ "login": login, "role": role, "revoked": true })),
            )
            .await;
        if !saved {
            return Err(StaffError::JournalFailed);
        }
        Ok(())
    }

    /// Creates the first admin from the server settings when no active admin exists.
    /// A staff row with that login (for example the revoked first admin) is reset to an
    /// active admin with the configured password: the settings are the recovery path.
    /// Returns the admin's id when one was created or reset.
    pub async fn ensure_first_admin(
        &self,
        login: &str,
        password: &str,
    ) -> Result<Option<i64>, StaffError> {
        let have: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM auth.staff WHERE role = 'admin' AND revoked_at IS NULL)",
        )
        .fetch_one(&self.pool)
        .await?;
        if have {
            return Ok(None);
        }
        let login = normalize_login(login).map_err(StaffError::Invalid)?;
        let hash = self.hash(password).await.map_err(StaffError::Internal)?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO auth.staff (login, password_hash, role, created_at) \
             VALUES ($1, $2, 'admin', $3) \
             ON CONFLICT (login) DO UPDATE SET role = 'admin', password_hash = EXCLUDED.password_hash, \
               revoked_at = NULL \
             RETURNING id",
        )
        .bind(&login)
        .bind(&hash)
        .bind(self.now())
        .fetch_one(&self.pool)
        .await?;
        self.journal(
            &Person::SYSTEM,
            id,
            "first_admin",
            None,
            Some(json!({ "login": login, "role": "admin" })),
        )
        .await;
        Ok(Some(id))
    }
}
