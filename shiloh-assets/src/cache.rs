//! Reference-counted asset cache keyed by path hash.

use ahash::AHashMap;
use parking_lot::RwLock;
use shiloh_core::HandleAllocator;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::handle::{AssetId, AssetState, AssetTag};

struct Entry {
    path: PathBuf,
    state: AssetState,
    // Type-erased payload placeholder.
    #[allow(dead_code)]
    data: Option<Arc<[u8]>>,
}

pub struct AssetCache {
    alloc: RwLock<HandleAllocator<AssetTag>>,
    by_path: RwLock<AHashMap<PathBuf, AssetId>>,
    entries: RwLock<AHashMap<AssetId, Entry>>,
}

impl Default for AssetCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            alloc: RwLock::new(HandleAllocator::new()),
            by_path: RwLock::new(AHashMap::new()),
            entries: RwLock::new(AHashMap::new()),
        }
    }

    pub fn load_bytes(&self, path: impl AsRef<Path>) -> std::io::Result<AssetId> {
        let path = path.as_ref().to_path_buf();
        if let Some(id) = self.by_path.read().get(&path).copied() {
            return Ok(id);
        }
        let bytes = std::fs::read(&path)?;
        let id = self.alloc.write().alloc();
        self.by_path.write().insert(path.clone(), id);
        self.entries.write().insert(
            id,
            Entry {
                path,
                state: AssetState::Ready,
                data: Some(Arc::from(bytes.into_boxed_slice())),
            },
        );
        Ok(id)
    }

    pub fn state(&self, id: AssetId) -> Option<AssetState> {
        self.entries.read().get(&id).map(|e| e.state)
    }

    pub fn path(&self, id: AssetId) -> Option<PathBuf> {
        self.entries.read().get(&id).map(|e| e.path.clone())
    }
}
