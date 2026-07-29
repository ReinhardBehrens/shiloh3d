//! Archetype storage: entities sharing the same component set live in one SoA table.

use ahash::{AHashMap, AHashSet};
use smallvec::SmallVec;
use std::any::Any;

use crate::component::ComponentId;
use crate::entity::Entity;

/// Sorted set of component IDs identifying an archetype.
pub type Signature = SmallVec<[ComponentId; 8]>;

/// Type-erased column of a single component type (SoA).
pub struct Column {
    pub component: ComponentId,
    data: Box<dyn Any + Send + Sync>,
    len: usize,
}

impl Column {
    pub fn new<T: Send + Sync + 'static>(component: ComponentId) -> Self {
        Self {
            component,
            data: Box::new(Vec::<T>::new()),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push<T: Send + Sync + 'static>(&mut self, value: T) {
        self.data
            .downcast_mut::<Vec<T>>()
            .expect("column type mismatch")
            .push(value);
        self.len += 1;
    }

    pub fn swap_remove<T: Send + Sync + 'static>(&mut self, row: usize) -> T {
        let v = self
            .data
            .downcast_mut::<Vec<T>>()
            .expect("column type mismatch")
            .swap_remove(row);
        self.len -= 1;
        v
    }

    pub fn get<T: Send + Sync + 'static>(&self, row: usize) -> Option<&T> {
        self.data.downcast_ref::<Vec<T>>()?.get(row)
    }

    pub fn get_mut<T: Send + Sync + 'static>(&mut self, row: usize) -> Option<&mut T> {
        self.data.downcast_mut::<Vec<T>>()?.get_mut(row)
    }

    pub fn as_slice<T: Send + Sync + 'static>(&self) -> &[T] {
        self.data
            .downcast_ref::<Vec<T>>()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn as_mut_slice<T: Send + Sync + 'static>(&mut self) -> &mut [T] {
        self.data
            .downcast_mut::<Vec<T>>()
            .map(Vec::as_mut_slice)
            .unwrap_or(&mut [])
    }
}

/// One archetype: entity list + SoA columns.
pub struct Archetype {
    pub signature: Signature,
    pub entities: Vec<Entity>,
    pub columns: Vec<Column>,
    pub(crate) column_index: AHashMap<ComponentId, usize>,
}

impl Archetype {
    pub fn empty() -> Self {
        Self {
            signature: Signature::new(),
            entities: Vec::new(),
            columns: Vec::new(),
            column_index: AHashMap::new(),
        }
    }

    pub fn with_signature(signature: Signature) -> Self {
        // Columns are created lazily via World::ensure_column — do not pre-index
        // component IDs here or `has()` will lie about empty column storage.
        Self {
            signature,
            entities: Vec::new(),
            columns: Vec::new(),
            column_index: AHashMap::new(),
        }
    }

    pub fn has(&self, id: ComponentId) -> bool {
        self.column_index.contains_key(&id)
    }

    pub fn column(&self, id: ComponentId) -> Option<&Column> {
        self.column_index.get(&id).map(|&i| &self.columns[i])
    }

    pub fn column_mut(&mut self, id: ComponentId) -> Option<&mut Column> {
        self.column_index.get(&id).copied().map(|i| &mut self.columns[i])
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

/// Maps entities → (archetype, row).
#[derive(Debug, Clone, Copy)]
pub struct EntityLocation {
    pub archetype: usize,
    pub row: usize,
}

/// World-wide archetype graph keyed by signature.
#[derive(Default)]
pub struct Archetypes {
    pub archetypes: Vec<Archetype>,
    by_signature: AHashMap<Signature, usize>,
}

impl Archetypes {
    pub fn new() -> Self {
        let mut a = Self::default();
        // Archetype 0 = empty signature.
        a.archetypes.push(Archetype::empty());
        a.by_signature.insert(Signature::new(), 0);
        a
    }

    pub fn get_or_create(&mut self, signature: Signature) -> usize {
        if let Some(&idx) = self.by_signature.get(&signature) {
            return idx;
        }
        let idx = self.archetypes.len();
        self.by_signature.insert(signature.clone(), idx);
        self.archetypes.push(Archetype::with_signature(signature));
        idx
    }

    pub fn matching(&self, required: &AHashSet<ComponentId>) -> Vec<usize> {
        self.archetypes
            .iter()
            .enumerate()
            .filter(|(_, arch)| required.iter().all(|id| arch.has(*id)))
            .map(|(i, _)| i)
            .collect()
    }
}
