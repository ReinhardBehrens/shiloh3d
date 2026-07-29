//! Scene root wrapping an ECS world.

use shiloh_ecs::{Entity, World};

use crate::transform::Transform;

pub struct Scene {
    pub world: World,
    pub name: String,
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            world: World::new(),
            name: name.into(),
        }
    }

    pub fn spawn_transform(&mut self, transform: Transform) -> Entity {
        self.world.spawn(transform)
    }
}
