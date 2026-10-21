//! What a wallets operation can answer.

use api_wallets::WalletsError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The shared `arena_data` lock is held.
    #[error("collection running, try again later")]
    Busy,
    #[error("staff only")]
    Forbidden,
    #[error("{0} is not in the network")]
    NotMember(String),
    #[error("not a Solana address: {0}")]
    InvalidAddress(String),
    #[error("no wallet candidate {0}")]
    NoCandidate(i64),
    /// The move or the input is not allowed; the text says why.
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    Internal(String),
}

macro_rules! internal {
    ($($t:ty),*) => {$(
        impl From<$t> for Error {
            fn from(e: $t) -> Self {
                Error::Internal(e.to_string())
            }
        }
    )*};
}

internal!(
    sqlx::Error,
    api_registry::RegistryError,
    api_journal::JournalError,
    api_roster::RosterError,
    api_kolexplorer::KeError
);

impl From<Error> for WalletsError {
    fn from(e: Error) -> Self {
        match e {
            Error::Busy => WalletsError::Busy,
            Error::NotMember(kol) => WalletsError::NotMember { kol },
            Error::InvalidAddress(address) => WalletsError::InvalidAddress { address },
            other => WalletsError::Internal {
                message: other.to_string(),
            },
        }
    }
}
