//! Editor selection set.

use shiloh_ecs::Entity;

#[derive(Debug, Default, Clone)]
pub struct Selection {
    pub entities: Vec<Entity>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.entities.clear();
    }

    pub fn select(&mut self, entity: Entity) {
        if !self.entities.iter().any(|e| *e == entity) {
            self.entities.push(entity);
        }
    }
}
