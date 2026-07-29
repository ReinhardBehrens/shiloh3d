//! Scene JSON save/load shared by editor and runtime.

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::camera::{Camera, ProjectionKind};
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
        let mut entities = Vec::new();
        scene.world.for_each::<Transform>(|_, t| {
            entities.push(EntityRecord {
                name: format!("entity_{}", entities.len()),
                translation: t.translation.to_array(),
                rotation: t.rotation.to_array(),
                scale: t.scale.to_array(),
                parent: None,
            });
        });
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
        for e in &self.entities {
            let t = Transform {
                translation: Vec3::from_array(e.translation),
                rotation: Quat::from_array(e.rotation),
                scale: Vec3::from_array(e.scale),
                dirty: true,
            };
            scene.spawn_transform(t);
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
