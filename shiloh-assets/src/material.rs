//! Material asset — editable CPU description for hot reload / packaging.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Shiloh material (authoring + package JSON).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialAsset {
    pub name: String,
    pub albedo_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub albedo_texture: Option<PathBuf>,
    pub metallic_roughness_texture: Option<PathBuf>,
}

impl Default for MaterialAsset {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            albedo_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            albedo_texture: None,
            metallic_roughness_texture: None,
        }
    }
}

impl MaterialAsset {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, MaterialError> {
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), MaterialError> {
        let text = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MaterialError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
