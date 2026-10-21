//! Board API (layer 1). The board has no API of its own; this crate holds the API slot
//! `board/in_network` («in network?»), filled by roster. Without a filler: not a member.

use arena_platform::Person;
use async_trait::async_trait;

/// Slot name as in `plugin.toml` (`api:in_network`).
pub const IN_NETWORK_SLOT: &str = "in_network";

/// API slot `board/in_network`: is the KOL in our network?
#[async_trait]
pub trait InNetwork: Send + Sync {
    async fn is_member(&self, person: &Person, kol: &str) -> bool;
}

/// Default of the slot: never a member.
pub struct NotInNetwork;

#[async_trait]
impl InNetwork for NotInNetwork {
    async fn is_member(&self, _person: &Person, _kol: &str) -> bool {
        false
    }
}
