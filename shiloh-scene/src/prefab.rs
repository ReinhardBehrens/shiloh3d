//! Prefabs — serialized entity templates spawned into a scene.

use serde::{Deserialize, Serialize};

use crate::hierarchy::set_parent;
use crate::scene::Scene;
use crate::serialize::EntityRecord;
use crate::transform::Transform;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prefab {
    pub name: String,
    pub entities: Vec<EntityRecord>,
}

impl Prefab {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entities: Vec::new(),
        }
    }

    pub fn from_records(name: impl Into<String>, entities: Vec<EntityRecord>) -> Self {
        Self {
            name: name.into(),
            entities,
        }
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), PrefabError> {
        let text = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, PrefabError> {
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Spawn all prefab entities into `scene`, returning root entity indices.
    pub fn spawn_into(&self, scene: &mut Scene) -> Vec<shiloh_ecs::Entity> {
        let spawned: Vec<_> = self
            .entities
            .iter()
            .map(|record| {
                scene.spawn_transform(Transform {
                    translation: glam::Vec3::from_array(record.translation),
                    rotation: glam::Quat::from_array(record.rotation),
                    scale: glam::Vec3::from_array(record.scale),
                    dirty: true,
                })
            })
            .collect();
        for (record, &child) in self.entities.iter().zip(&spawned) {
            if let Some(parent_index) = record.parent
                && let Some(&parent) = spawned.get(parent_index)
            {
                set_parent(&mut scene.world, child, parent);
            }
        }
        spawned
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrefabError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
