//! Transport trait + in-memory loopback (no sockets required).

use std::collections::VecDeque;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("not connected")]
    NotConnected,
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone)]
pub struct Packet {
    pub channel: u8,
    pub payload: Vec<u8>,
}

pub trait Transport: Send + Sync {
    fn send(&mut self, packet: Packet) -> Result<(), TransportError>;
    fn recv(&mut self) -> Result<Option<Packet>, TransportError>;
}

/// Pure-Rust loopback queue for tests and single-process multiplayer simulation.
#[derive(Debug, Default)]
pub struct InMemoryTransport {
    inbox: VecDeque<Packet>,
    outbox: VecDeque<Packet>,
}

impl InMemoryTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves outbox → peer inbox (call from test harness / local server).
    pub fn deliver_to(&mut self, peer: &mut InMemoryTransport) {
        while let Some(p) = self.outbox.pop_front() {
            peer.inbox.push_back(p);
        }
    }
}

impl Transport for InMemoryTransport {
    fn send(&mut self, packet: Packet) -> Result<(), TransportError> {
        self.outbox.push_back(packet);
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<Packet>, TransportError> {
        Ok(self.inbox.pop_front())
    }
}
