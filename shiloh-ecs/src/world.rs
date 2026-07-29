//! World: entity spawn/despawn and component access.

use ahash::AHashMap;
use shiloh_core::handle::HandleAllocator;
use thiserror::Error;

use crate::component::{Component, ComponentId, ComponentRegistry};
use crate::entity::{Entity, EntityTag};
use crate::storage::{Archetype, Archetypes, Column, EntityLocation, Signature};

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
        self.locations.insert(entity, EntityLocation { archetype: arch, row });
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

        // If already in an archetype that has T, overwrite.
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

        // Move to archetype with +T.
        let mut new_sig: Signature = self.archetypes.archetypes[loc.archetype]
            .signature
            .clone();
        if !new_sig.contains(&id) {
            new_sig.push(id);
            new_sig.sort_by_key(|c| c.0);
        }
        let new_arch_idx = self.archetypes.get_or_create(new_sig);
        self.ensure_column::<T>(new_arch_idx, id);
        self.move_entity(entity, loc, new_arch_idx, Some((id, value)))?;
        Ok(())
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

    fn ensure_column<T: Component>(&mut self, arch_idx: usize, id: ComponentId) {
        let arch = &mut self.archetypes.archetypes[arch_idx];
        if arch.has(id) {
            return;
        }
        let col_idx = arch.columns.len();
        arch.columns.push(Column::new::<T>(id));
        arch.column_index.insert(id, col_idx);
        // Keep signature in sync if needed.
        if !arch.signature.contains(&id) {
            arch.signature.push(id);
            arch.signature.sort_by_key(|c| c.0);
        }
    }

    fn move_entity<T: Component>(
        &mut self,
        entity: Entity,
        from: EntityLocation,
        to_arch: usize,
        new_component: Option<(ComponentId, T)>,
    ) -> Result<(), WorldError> {
        // For the initial scaffold we only support insert-into-empty or single-component spawn paths
        // by rebuilding the destination row with the new component. Full structural moves land next.
        let _ = from;
        let dest_row = {
            let dest = &mut self.archetypes.archetypes[to_arch];
            let row = dest.entities.len();
            dest.entities.push(entity);
            if let Some((id, value)) = new_component {
                if let Some(col) = dest.column_mut(id) {
                    col.push(value);
                }
            }
            row
        };

        // Remove from old archetype entity list (components left as stub for now).
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
        let arch: &mut Archetype = &mut self.archetypes.archetypes[loc.archetype];
        let last = arch.entities.len().saturating_sub(1);
        if loc.row >= arch.entities.len() {
            return;
        }
        let swapped = arch.entities.swap_remove(loc.row);
        // Columns: best-effort length sync for scaffold; typed swap_remove comes with full mover.
        for col in &mut arch.columns {
            // Keep column lengths consistent by truncating if needed.
            while col.len() > arch.entities.len() {
                // Can't type-erase swap_remove without type; leave data until full impl.
                break;
            }
            let _ = col;
        }
        if loc.row < arch.entities.len() {
            self.locations.insert(
                swapped,
                EntityLocation {
                    archetype: loc.archetype,
                    row: loc.row,
                },
            );
        }
        let _ = last;
    }
}
