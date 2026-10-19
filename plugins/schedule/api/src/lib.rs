//! Collection schedule (layer 2). Schedule has no API; this crate holds the event it emits.

use arena_platform::Event;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// «Snapshot opened»: emitted after the snapshot for `date` opens and before collection, so
/// other blocks add their members first (roster adds network KOLs).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotOpened {
    pub date: NaiveDate,
}

impl Event for SnapshotOpened {
    const NAME: &'static str = "snapshot_opened";
}
