//! Large-world partitioning hooks (Phase 4 foundation).

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Stable id for a streamed world cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId {
    pub x: i32,
    pub z: i32,
}

impl ChunkId {
    pub fn from_world_pos(pos: Vec3, chunk_size: f32) -> Self {
        let size = chunk_size.max(1.0);
        Self {
            x: (pos.x / size).floor() as i32,
            z: (pos.z / size).floor() as i32,
        }
    }

    pub fn neighbors(self) -> [ChunkId; 8] {
        [
            ChunkId {
                x: self.x - 1,
                z: self.z - 1,
            },
            ChunkId {
                x: self.x,
                z: self.z - 1,
            },
            ChunkId {
                x: self.x + 1,
                z: self.z - 1,
            },
            ChunkId {
                x: self.x - 1,
                z: self.z,
            },
            ChunkId {
                x: self.x + 1,
                z: self.z,
            },
            ChunkId {
                x: self.x - 1,
                z: self.z + 1,
            },
            ChunkId {
                x: self.x,
                z: self.z + 1,
            },
            ChunkId {
                x: self.x + 1,
                z: self.z + 1,
            },
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkState {
    Unloaded,
    Loading,
    Resident,
    Evicting,
}

/// Streaming budget for a focus point (camera / player).
#[derive(Debug, Clone)]
pub struct WorldPartition {
    pub chunk_size: f32,
    pub load_radius: i32,
    pub focus: Vec3,
    pub resident: Vec<(ChunkId, ChunkState)>,
}

impl Default for WorldPartition {
    fn default() -> Self {
        Self {
            chunk_size: 64.0,
            load_radius: 2,
            focus: Vec3::ZERO,
            resident: Vec::new(),
        }
    }
}

impl WorldPartition {
    pub fn set_focus(&mut self, focus: Vec3) {
        self.focus = focus;
        let center = ChunkId::from_world_pos(focus, self.chunk_size);
        let r = self.load_radius;
        let mut wanted = Vec::new();
        for z in -r..=r {
            for x in -r..=r {
                wanted.push(ChunkId {
                    x: center.x + x,
                    z: center.z + z,
                });
            }
        }
        let prev = std::mem::take(&mut self.resident);
        let mut next = Vec::new();
        for id in &wanted {
            let state = prev
                .iter()
                .find(|(c, _)| c == id)
                .map(|(_, s)| match s {
                    ChunkState::Unloaded | ChunkState::Evicting => ChunkState::Loading,
                    other => *other,
                })
                .unwrap_or(ChunkState::Loading);
            next.push((*id, state));
        }
        // Keep out-of-range residents as Evicting until tick_streaming drops them.
        for (id, state) in prev {
            if wanted.contains(&id) {
                continue;
            }
            if matches!(state, ChunkState::Resident | ChunkState::Loading | ChunkState::Evicting) {
                next.push((id, ChunkState::Evicting));
            }
        }
        self.resident = next;
    }

    /// Promote `Loading` → `Resident` and drop `Evicting` chunks.
    /// Returns `(newly_resident, evicted)`.
    pub fn tick_streaming(&mut self) -> (Vec<ChunkId>, Vec<ChunkId>) {
        let mut newly = Vec::new();
        let mut evicted = Vec::new();
        let mut next = Vec::new();
        for (id, state) in std::mem::take(&mut self.resident) {
            match state {
                ChunkState::Loading => {
                    newly.push(id);
                    next.push((id, ChunkState::Resident));
                }
                ChunkState::Evicting | ChunkState::Unloaded => {
                    evicted.push(id);
                }
                ChunkState::Resident => next.push((id, ChunkState::Resident)),
            }
        }
        self.resident = next;
        (newly, evicted)
    }

    pub fn mark_resident(&mut self, id: ChunkId) {
        if let Some((_, s)) = self.resident.iter_mut().find(|(c, _)| *c == id) {
            *s = ChunkState::Resident;
        } else {
            self.resident.push((id, ChunkState::Resident));
        }
    }

    pub fn resident_ids(&self) -> impl Iterator<Item = ChunkId> + '_ {
        self.resident
            .iter()
            .filter(|(_, s)| *s == ChunkState::Resident)
            .map(|(id, _)| *id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_loads_and_evicts() {
        let mut wp = WorldPartition {
            chunk_size: 10.0,
            load_radius: 1,
            ..Default::default()
        };
        wp.set_focus(Vec3::ZERO);
        assert_eq!(wp.resident.len(), 9);
        let (new1, evict1) = wp.tick_streaming();
        assert_eq!(new1.len(), 9);
        assert!(evict1.is_empty());
        assert!(wp.resident.iter().all(|(_, s)| *s == ChunkState::Resident));

        wp.set_focus(Vec3::new(100.0, 0.0, 0.0));
        let (_new2, evict2) = wp.tick_streaming();
        assert!(!evict2.is_empty());
        assert!(wp.resident.iter().all(|(id, _)| id.x >= 9));
    }
}
