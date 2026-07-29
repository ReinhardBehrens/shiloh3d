//! Query stubs for iterating matching archetypes.

use ahash::AHashSet;

use crate::component::{Component, ComponentId};
use crate::world::World;

/// Declares which component types a system wants to read/write.
pub struct Query<Q> {
    _marker: core::marker::PhantomData<Q>,
}

impl<Q> Default for Query<Q> {
    fn default() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

/// Trait implemented for query parameter tuples (expanded over time).
pub trait QueryData {
    fn required(world: &World) -> AHashSet<ComponentId>;
}

impl<T: Component> QueryData for &T {
    fn required(world: &World) -> AHashSet<ComponentId> {
        let mut set = AHashSet::new();
        if let Some(id) = world.registry.id::<T>() {
            set.insert(id);
        }
        set
    }
}

impl<T: Component> QueryData for &mut T {
    fn required(world: &World) -> AHashSet<ComponentId> {
        <&T as QueryData>::required(world)
    }
}
