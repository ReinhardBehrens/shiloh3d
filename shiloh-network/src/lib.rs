//! Replication and multiplayer transport — pure Rust message layer.
//!
//! Transport backends (QUIC via `quinn`, WebTransport, etc.) plug in later.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod replication;
pub mod transport;

pub use replication::{NetId, ReplicationBuffer, ReplicationChannel, ReplicationDelta};
pub use transport::{InMemoryTransport, Packet, Transport};
