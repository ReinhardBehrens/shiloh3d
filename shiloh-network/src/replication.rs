//! Entity replication identifiers and channels.

use serde::{Deserialize, Serialize};
use shiloh_core::Handle;

use crate::transport::{Packet, Transport, TransportError};

#[derive(Debug, Clone, Copy, Default)]
pub struct NetTag;

pub type NetId = Handle<NetTag>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplicationChannel {
    Reliable,
    Unreliable,
    Snapshot,
}

/// Component-change payload for a single replicated entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationDelta {
    pub net_id: u64,
    pub channel: ReplicationChannel,
    pub bytes: Vec<u8>,
}

/// Accumulates deltas for a tick before flush to [`crate::transport::Transport`].
#[derive(Debug, Default)]
pub struct ReplicationBuffer {
    pub deltas: Vec<ReplicationDelta>,
}

impl ReplicationBuffer {
    pub fn push(&mut self, delta: ReplicationDelta) {
        self.deltas.push(delta);
    }

    pub fn drain(&mut self) -> Vec<ReplicationDelta> {
        std::mem::take(&mut self.deltas)
    }

    /// Serialize each delta and send over `transport` (channel byte = enum discriminant).
    pub fn flush_to(&mut self, transport: &mut dyn Transport) -> Result<usize, TransportError> {
        let deltas = self.drain();
        let n = deltas.len();
        for d in deltas {
            let payload = serde_json::to_vec(&d).map_err(|e| {
                TransportError::Message(format!("replication encode: {e}"))
            })?;
            let channel = match d.channel {
                ReplicationChannel::Reliable => 0,
                ReplicationChannel::Unreliable => 1,
                ReplicationChannel::Snapshot => 2,
            };
            transport.send(Packet { channel, payload })?;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::InMemoryTransport;

    #[test]
    fn flush_roundtrip_memory() {
        let mut buf = ReplicationBuffer::default();
        buf.push(ReplicationDelta {
            net_id: 7,
            channel: ReplicationChannel::Reliable,
            bytes: vec![1, 2, 3],
        });
        let mut a = InMemoryTransport::new();
        let mut b = InMemoryTransport::new();
        assert_eq!(buf.flush_to(&mut a).unwrap(), 1);
        a.deliver_to(&mut b);
        let pkt = b.recv().unwrap().expect("packet");
        let decoded: ReplicationDelta = serde_json::from_slice(&pkt.payload).unwrap();
        assert_eq!(decoded.net_id, 7);
        assert_eq!(decoded.bytes, vec![1, 2, 3]);
    }
}
