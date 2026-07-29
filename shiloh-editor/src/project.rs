//! Editor project on disk.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub name: String,
    pub engine_version: String,
    pub default_scene: String,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: ProjectManifest,
}

impl Project {
    pub fn create(root: impl Into<PathBuf>, name: impl Into<String>) -> Result<Self, ProjectError> {
        let root = root.into();
        std::fs::create_dir_all(root.join("assets"))?;
        std::fs::create_dir_all(root.join("scenes"))?;
        let manifest = ProjectManifest {
            name: name.into(),
            engine_version: env!("CARGO_PKG_VERSION").into(),
            default_scene: "scenes/main.json".into(),
        };
        let text = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(root.join("shiloh.project.json"), text)?;
        Ok(Self { root, manifest })
    }

    pub fn load(root: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let root = root.as_ref().to_path_buf();
        let text = std::fs::read_to_string(root.join("shiloh.project.json"))?;
        let manifest = serde_json::from_str(&text)?;
        Ok(Self { root, manifest })
    }
}
