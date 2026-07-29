//! Component trait and type registry.

use core::any::{Any, TypeId};
use ahash::AHashMap;

/// Marker for ECS components. `'static + Send + Sync` for parallel systems.
pub trait Component: Any + Send + Sync + 'static {}

impl<T: Any + Send + Sync + 'static> Component for T {}

/// Dense component type identifier assigned by the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u16);

/// Maps Rust `TypeId` → dense `ComponentId`.
#[derive(Debug, Default)]
pub struct ComponentRegistry {
    to_id: AHashMap<TypeId, ComponentId>,
    type_names: Vec<&'static str>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Component>(&mut self) -> ComponentId {
        let tid = TypeId::of::<T>();
        if let Some(id) = self.to_id.get(&tid) {
            return *id;
        }
        let id = ComponentId(self.type_names.len() as u16);
        self.to_id.insert(tid, id);
        self.type_names.push(core::any::type_name::<T>());
        id
    }

    pub fn id<T: Component>(&self) -> Option<ComponentId> {
        self.to_id.get(&TypeId::of::<T>()).copied()
    }

    pub fn name(&self, id: ComponentId) -> Option<&'static str> {
        self.type_names.get(id.0 as usize).copied()
    }

    pub fn len(&self) -> usize {
        self.type_names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.type_names.is_empty()
    }
}
