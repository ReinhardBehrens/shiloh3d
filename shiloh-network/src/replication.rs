//! Entity replication identifiers and channels.

use serde::{Deserialize, Serialize};
use shiloh_core::Handle;

#[derive(Debug, Clone, Copy, Default)]
pub struct NetTag;

pub type NetId = Handle<NetTag>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplicationChannel {
    Reliable,
    Unreliable,
    Snapshot,
}
