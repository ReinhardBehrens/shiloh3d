//! World: entity spawn/despawn and component access.

use ahash::{AHashMap, AHashSet};
use shiloh_core::handle::HandleAllocator;
use thiserror::Error;

use crate::component::{Component, ComponentId, ComponentRegistry};
use crate::entity::{Entity, EntityTag};
use crate::storage::{Archetypes, Column, EntityLocation, Signature};

#[derive(Debug, Error)]
pub enum WorldError {
    #[error("entity is not alive")]
    DeadEntity,
    #[error("component not present on entity")]
    MissingComponent,
}

/// Primary ECS container.
pub struct World {
    entities: HandleAllocator<EntityTag>,
    locations: AHashMap<Entity, EntityLocation>,
    pub(crate) archetypes: Archetypes,
    pub registry: ComponentRegistry,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: HandleAllocator::new(),
            locations: AHashMap::new(),
            archetypes: Archetypes::new(),
            registry: ComponentRegistry::new(),
        }
    }

    pub fn spawn_empty(&mut self) -> Entity {
        let entity = self.entities.alloc();
        let arch = 0usize;
        let row = self.archetypes.archetypes[arch].entities.len();
        self.archetypes.archetypes[arch].entities.push(entity);
        self.locations
            .insert(entity, EntityLocation { archetype: arch, row });
        entity
    }

    pub fn spawn<T: Component>(&mut self, component: T) -> Entity {
        let entity = self.spawn_empty();
        self.insert(entity, component).expect("fresh entity");
        entity
    }

    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.is_live(entity) {
            return false;
        }
        if let Some(loc) = self.locations.remove(&entity) {
            self.swap_remove_row(loc);
        }
        self.entities.free(entity);
        true
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_live(entity)
    }

    pub fn insert<T: Component>(&mut self, entity: Entity, value: T) -> Result<(), WorldError> {
        if !self.is_alive(entity) {
            return Err(WorldError::DeadEntity);
        }
        let id = self.registry.register::<T>();
        let loc = *self.locations.get(&entity).ok_or(WorldError::DeadEntity)?;

        {
            let arch = &mut self.archetypes.archetypes[loc.archetype];
            if arch.has(id) {
                if let Some(col) = arch.column_mut(id) {
                    if let Some(slot) = col.get_mut::<T>(loc.row) {
                        *slot = value;
                        return Ok(());
                    }
                }
            }
        }

        let mut new_sig: Signature = self.archetypes.archetypes[loc.archetype].signature.clone();
        if !new_sig.contains(&id) {
            new_sig.push(id);
            new_sig.sort_by_key(|c| c.0);
        }
        let new_arch_idx = self.archetypes.get_or_create(new_sig);
        self.ensure_column::<T>(new_arch_idx, id);
        let src_sig = self.archetypes.archetypes[loc.archetype].signature.clone();
        for cid in &src_sig {
            self.ensure_column_exists(new_arch_idx, *cid, loc.archetype);
        }
        self.move_entity(entity, loc, new_arch_idx, Some((id, value)))
    }

    pub fn remove<T: Component>(&mut self, entity: Entity) -> Result<(), WorldError> {
        if !self.is_alive(entity) {
            return Err(WorldError::DeadEntity);
        }
        let id = self.registry.id::<T>().ok_or(WorldError::MissingComponent)?;
        let loc = *self.locations.get(&entity).ok_or(WorldError::DeadEntity)?;
        if !self.archetypes.archetypes[loc.archetype].has(id) {
            return Err(WorldError::MissingComponent);
        }

        let mut new_sig: Signature = self.archetypes.archetypes[loc.archetype]
            .signature
            .iter()
            .copied()
            .filter(|c| *c != id)
            .collect();
        new_sig.sort_by_key(|c| c.0);
        let new_arch_idx = self.archetypes.get_or_create(new_sig);
        let src_sig = self.archetypes.archetypes[loc.archetype].signature.clone();
        for cid in &src_sig {
            if *cid != id {
                self.ensure_column_exists(new_arch_idx, *cid, loc.archetype);
            }
        }
        self.move_entity::<T>(entity, loc, new_arch_idx, None)
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        let id = self.registry.id::<T>()?;
        let loc = self.locations.get(&entity)?;
        self.archetypes.archetypes[loc.archetype]
            .column(id)?
            .get::<T>(loc.row)
    }

    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        let id = self.registry.id::<T>()?;
        let loc = *self.locations.get(&entity)?;
        self.archetypes.archetypes[loc.archetype]
            .column_mut(id)?
            .get_mut::<T>(loc.row)
    }

    pub fn entity_count(&self) -> usize {
        self.locations.len()
    }

    /// Iterate all entities that have component `T`.
    pub fn for_each<T: Component>(&self, mut f: impl FnMut(Entity, &T)) {
        let Some(id) = self.registry.id::<T>() else {
            return;
        };
        let required = AHashSet::from([id]);
        for arch_idx in self.archetypes.matching(&required) {
            let arch = &self.archetypes.archetypes[arch_idx];
            let Some(col) = arch.column(id) else {
                continue;
            };
            let slice = col.as_slice::<T>();
            for (row, entity) in arch.entities.iter().enumerate() {
                if let Some(c) = slice.get(row) {
                    f(*entity, c);
                }
            }
        }
    }

    /// Mutable iterate entities with `T`.
    pub fn for_each_mut<T: Component>(&mut self, mut f: impl FnMut(Entity, &mut T)) {
        let Some(id) = self.registry.id::<T>() else {
            return;
        };
        let required = AHashSet::from([id]);
        let arches = self.archetypes.matching(&required);
        for arch_idx in arches {
            let arch = &mut self.archetypes.archetypes[arch_idx];
            let entities = arch.entities.clone();
            let Some(col) = arch.column_mut(id) else {
                continue;
            };
            let slice = col.as_mut_slice::<T>();
            for (row, entity) in entities.iter().enumerate() {
                if let Some(c) = slice.get_mut(row) {
                    f(*entity, c);
                }
            }
        }
    }

    /// Entities that have all of the given registered component ids.
    pub fn entities_with(&self, required: &AHashSet<ComponentId>) -> Vec<Entity> {
        let mut out = Vec::new();
        for arch_idx in self.archetypes.matching(required) {
            out.extend(
                self.archetypes.archetypes[arch_idx]
                    .entities
                    .iter()
                    .copied(),
            );
        }
        out
    }

    fn ensure_column<T: Component>(&mut self, arch_idx: usize, id: ComponentId) {
        let arch = &mut self.archetypes.archetypes[arch_idx];
        if arch.has(id) {
            return;
        }
        let col_idx = arch.columns.len();
        arch.columns.push(Column::new::<T>(id));
        arch.column_index.insert(id, col_idx);
    }

    fn ensure_column_exists(&mut self, dst: usize, id: ComponentId, src_arch: usize) {
        if self.archetypes.archetypes[dst].has(id) {
            return;
        }
        let src_col_idx = match self.archetypes.archetypes[src_arch]
            .column_index
            .get(&id)
            .copied()
        {
            Some(i) => i,
            None => return,
        };
        let shell = self.archetypes.archetypes[src_arch].columns[src_col_idx].empty_like();
        let dst_arch = &mut self.archetypes.archetypes[dst];
        let col_idx = dst_arch.columns.len();
        dst_arch.columns.push(shell);
        dst_arch.column_index.insert(id, col_idx);
    }

    fn move_entity<T: Component>(
        &mut self,
        entity: Entity,
        from: EntityLocation,
        to_arch: usize,
        new_component: Option<(ComponentId, T)>,
    ) -> Result<(), WorldError> {
        let dest_row = self.archetypes.archetypes[to_arch].entities.len();
        let skip_id = new_component.as_ref().map(|(id, _)| *id);

        let src_cols: Vec<ComponentId> = self.archetypes.archetypes[from.archetype]
            .columns
            .iter()
            .map(|c| c.component)
            .collect();

        for cid in src_cols {
            if skip_id == Some(cid) {
                continue;
            }
            let src_i = self.archetypes.archetypes[from.archetype]
                .column_index
                .get(&cid)
                .copied();
            let dst_i = self.archetypes.archetypes[to_arch]
                .column_index
                .get(&cid)
                .copied();
            if let (Some(si), Some(di)) = (src_i, dst_i) {
                assert_ne!(from.archetype, to_arch);
                let arches = &mut self.archetypes.archetypes;
                let (src_arch, dst_arch) = if from.archetype < to_arch {
                    let (a, b) = arches.split_at_mut(to_arch);
                    (&a[from.archetype], &mut b[0])
                } else {
                    let (a, b) = arches.split_at_mut(from.archetype);
                    (&b[0], &mut a[to_arch])
                };
                let src_col = &src_arch.columns[si];
                let dst_col = &mut dst_arch.columns[di];
                dst_col.push_clone_from(src_col, from.row);
            }
        }

        if let Some((id, value)) = new_component {
            if let Some(col) = self.archetypes.archetypes[to_arch].column_mut(id) {
                col.push(value);
            }
        }

        self.archetypes.archetypes[to_arch].entities.push(entity);
        self.swap_remove_row(from);
        self.locations.insert(
            entity,
            EntityLocation {
                archetype: to_arch,
                row: dest_row,
            },
        );
        Ok(())
    }

    fn swap_remove_row(&mut self, loc: EntityLocation) {
        let arch = &mut self.archetypes.archetypes[loc.archetype];
        if loc.row >= arch.entities.len() {
            return;
        }
        arch.entities.swap_remove(loc.row);
        for col in &mut arch.columns {
            if loc.row < col.len() {
                col.swap_remove_erased(loc.row);
            }
        }
        if loc.row < arch.entities.len() {
            let swapped = arch.entities[loc.row];
            self.locations.insert(
                swapped,
                EntityLocation {
                    archetype: loc.archetype,
                    row: loc.row,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Pos(f32);
    #[derive(Clone, Debug, PartialEq)]
    struct Vel(f32);

    #[test]
    fn spawn_insert_get_query() {
        let mut world = World::new();
        let e = world.spawn(Pos(1.0));
        world.insert(e, Vel(2.0)).unwrap();
        assert_eq!(world.get::<Pos>(e), Some(&Pos(1.0)));
        assert_eq!(world.get::<Vel>(e), Some(&Vel(2.0)));

        let mut n = 0;
        world.for_each::<Pos>(|_, p| {
            assert_eq!(p, &Pos(1.0));
            n += 1;
        });
        assert_eq!(n, 1);

        world.remove::<Vel>(e).unwrap();
        assert!(world.get::<Vel>(e).is_none());
        assert_eq!(world.get::<Pos>(e), Some(&Pos(1.0)));
        assert!(world.despawn(e));
        assert!(!world.is_alive(e));
    }

    #[test]
    fn swap_remove_preserves_others() {
        let mut world = World::new();
        let a = world.spawn(Pos(1.0));
        let b = world.spawn(Pos(2.0));
        let c = world.spawn(Pos(3.0));
        assert!(world.despawn(a));
        assert_eq!(world.get::<Pos>(b), Some(&Pos(2.0)));
        assert_eq!(world.get::<Pos>(c), Some(&Pos(3.0)));
        assert_eq!(world.entity_count(), 2);
    }
}
