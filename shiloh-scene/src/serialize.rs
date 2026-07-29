//! Scene JSON save/load shared by editor and runtime.

use ahash::AHashMap;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::camera::{Camera, ProjectionKind};
use crate::hierarchy::{Parent, set_parent};
use crate::scene::Scene;
use crate::transform::Transform;

#[derive(Debug, Error)]
pub enum SceneSerdeError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneFile {
    pub name: String,
    pub version: u32,
    pub entities: Vec<EntityRecord>,
    #[serde(default)]
    pub camera: Option<CameraRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRecord {
    pub name: String,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    #[serde(default)]
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraRecord {
    pub projection: String,
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub fov_y_degrees: f32,
    pub ortho_half_extent: f32,
}

impl Default for CameraRecord {
    fn default() -> Self {
        let c = Camera::default();
        Self::from_camera(&c)
    }
}

impl CameraRecord {
    pub fn from_camera(c: &Camera) -> Self {
        Self {
            projection: match c.projection {
                ProjectionKind::Perspective => "perspective".into(),
                ProjectionKind::Orthographic => "orthographic".into(),
                ProjectionKind::Isometric => "isometric".into(),
            },
            eye: c.eye.to_array(),
            target: c.target.to_array(),
            fov_y_degrees: c.fov_y_radians.to_degrees(),
            ortho_half_extent: c.ortho_half_extent,
        }
    }

    pub fn to_camera(&self) -> Camera {
        let mut c = Camera::default();
        c.projection = match self.projection.as_str() {
            "orthographic" => ProjectionKind::Orthographic,
            "isometric" => ProjectionKind::Isometric,
            _ => ProjectionKind::Perspective,
        };
        c.eye = Vec3::from_array(self.eye);
        c.target = Vec3::from_array(self.target);
        c.fov_y_radians = self.fov_y_degrees.to_radians();
        c.ortho_half_extent = self.ortho_half_extent;
        c
    }
}

impl SceneFile {
    pub fn from_scene(scene: &Scene, camera: Option<&Camera>) -> Self {
        // Stable order: one pass over `Transform` fixes each entity's index,
        // which `parent` fields below refer back into.
        let mut order = Vec::new();
        scene.world.for_each::<Transform>(|e, _| order.push(e));

        let index_of: AHashMap<_, usize> = order
            .iter()
            .enumerate()
            .map(|(i, &e)| (e, i))
            .collect();

        let entities = order
            .iter()
            .enumerate()
            .filter_map(|(i, &e)| {
                let t = scene.world.get::<Transform>(e)?;
                let parent = scene
                    .world
                    .get::<Parent>(e)
                    .and_then(|p| index_of.get(&p.0).copied());
                Some(EntityRecord {
                    name: format!("entity_{i}"),
                    translation: t.translation.to_array(),
                    rotation: t.rotation.to_array(),
                    scale: t.scale.to_array(),
                    parent,
                })
            })
            .collect();

        Self {
            name: scene.name.clone(),
            version: 1,
            entities,
            camera: camera.map(CameraRecord::from_camera),
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, SceneSerdeError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(text: &str) -> Result<Self, SceneSerdeError> {
        Ok(serde_json::from_str(text)?)
    }

    pub fn apply_to_scene(&self, scene: &mut Scene) -> Option<Camera> {
        scene.name = self.name.clone();

        // Spawn every entity first so parent indices (which may reference
        // later records) always resolve to a live entity.
        let spawned: Vec<_> = self
            .entities
            .iter()
            .map(|e| {
                let t = Transform {
                    translation: Vec3::from_array(e.translation),
                    rotation: Quat::from_array(e.rotation),
                    scale: Vec3::from_array(e.scale),
                    dirty: true,
                };
                scene.spawn_transform(t)
            })
            .collect();

        for (record, &child) in self.entities.iter().zip(&spawned) {
            if let Some(parent_index) = record.parent
                && let Some(&parent) = spawned.get(parent_index)
            {
                set_parent(&mut scene.world, child, parent);
            }
        }

        self.camera.as_ref().map(|c| c.to_camera())
    }
}

/// Write scene JSON to disk.
pub fn save_scene(
    path: impl AsRef<std::path::Path>,
    scene: &Scene,
    camera: Option<&Camera>,
) -> Result<(), SceneSerdeError> {
    let file = SceneFile::from_scene(scene, camera);
    std::fs::write(path, file.to_json_pretty()?)?;
    Ok(())
}

/// Load scene JSON from disk into a fresh `Scene`.
pub fn load_scene(
    path: impl AsRef<std::path::Path>,
) -> Result<(Scene, Option<Camera>), SceneSerdeError> {
    let text = std::fs::read_to_string(path)?;
    let file = SceneFile::from_json(&text)?;
    let mut scene = Scene::new(file.name.clone());
    let camera = file.apply_to_scene(&mut scene);
    Ok((scene, camera))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::{Parent, propagate_transforms};
    use crate::transform::GlobalTransform;

    #[test]
    fn round_trips_parent_links() {
        let mut scene = Scene::new("parented");
        let root = scene.spawn_transform(Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)));
        let child = scene.spawn_transform(Transform::from_translation(Vec3::new(0.0, 2.0, 0.0)));
        set_parent(&mut scene.world, child, root);

        let file = SceneFile::from_scene(&scene, None);
        assert_eq!(file.entities.len(), 2);
        // Archetype moves triggered by `set_parent` mean iteration order isn't
        // guaranteed to match spawn order — only that `parent` correctly
        // indexes back into this same `entities` list.
        let root_index = file
            .entities
            .iter()
            .position(|e| e.parent.is_none())
            .expect("exactly one root");
        let child_index = 1 - root_index;
        assert_eq!(file.entities[child_index].parent, Some(root_index));

        let json = file.to_json_pretty().unwrap();
        let reloaded = SceneFile::from_json(&json).unwrap();

        let mut restored = Scene::new("restored");
        reloaded.apply_to_scene(&mut restored);
        propagate_transforms(&mut restored.world);

        let mut entities = Vec::new();
        restored.world.for_each::<Transform>(|e, _| entities.push(e));
        assert_eq!(entities.len(), 2);

        let restored_child = entities
            .iter()
            .copied()
            .find(|&e| restored.world.get::<Parent>(e).is_some())
            .expect("child has a parent");
        let global = restored.world.get::<GlobalTransform>(restored_child).unwrap().0;
        let t = global.w_axis.truncate();
        assert!((t.x - 1.0).abs() < 1e-4);
        assert!((t.y - 2.0).abs() < 1e-4);
    }

    #[test]
    fn entities_without_parent_round_trip_none() {
        let mut scene = Scene::new("flat");
        scene.spawn_transform(Transform::default());
        scene.spawn_transform(Transform::default());

        let file = SceneFile::from_scene(&scene, None);
        assert!(file.entities.iter().all(|e| e.parent.is_none()));
    }
}
