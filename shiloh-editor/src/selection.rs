//! Editor selection set with additive / toggle multi-select.

use shiloh_ecs::Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectMode {
    /// Replace the selection with a single entity (default click).
    #[default]
    Replace,
    /// Ctrl-click: toggle membership.
    Toggle,
    /// Shift-click: additive (keep existing, add if missing).
    Add,
}

#[derive(Debug, Default, Clone)]
pub struct Selection {
    pub entities: Vec<Entity>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.entities.clear();
    }

    pub fn select(&mut self, entity: Entity) {
        self.apply(entity, SelectMode::Replace);
    }

    pub fn apply(&mut self, entity: Entity, mode: SelectMode) {
        match mode {
            SelectMode::Replace => {
                self.entities.clear();
                self.entities.push(entity);
            }
            SelectMode::Add => {
                if !self.entities.iter().any(|e| *e == entity) {
                    self.entities.push(entity);
                }
            }
            SelectMode::Toggle => {
                if let Some(i) = self.entities.iter().position(|e| *e == entity) {
                    self.entities.remove(i);
                } else {
                    self.entities.push(entity);
                }
            }
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.iter().any(|e| *e == entity)
    }

    pub fn primary(&self) -> Option<Entity> {
        self.entities.first().copied()
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}
