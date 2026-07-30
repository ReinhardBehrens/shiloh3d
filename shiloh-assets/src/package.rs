//! Asset package manifest (JSON) + project cook helper.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPackage {
    pub name: String,
    pub version: String,
    pub assets: Vec<PathBuf>,
    #[serde(default)]
    pub entry_scene: Option<PathBuf>,
}

impl AssetPackage {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".into(),
            assets: Vec::new(),
            entry_scene: None,
        }
    }

    /// Collect files under `project_root` (assets/, scenes/, materials) into a package
    /// and write `out_dir/package.json` plus copied files under `out_dir/data/`.
    pub fn cook_project(
        project_root: &Path,
        out_dir: &Path,
        name: impl Into<String>,
    ) -> Result<Self, PackageError> {
        std::fs::create_dir_all(out_dir.join("data"))?;
        let mut pkg = Self::new(name);
        let mut collected = Vec::new();
        for sub in ["assets", "scenes", "scripts"] {
            let dir = project_root.join(sub);
            if dir.exists() {
                collect_files(&dir, &dir, &mut collected)?;
            }
        }
        for src in &collected {
            let rel = src
                .strip_prefix(project_root)
                .unwrap_or(src.as_path())
                .to_path_buf();
            let dest = out_dir.join("data").join(&rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(src, &dest)?;
            pkg.assets.push(rel);
        }
        let scene = project_root.join("scenes/main.json");
        if scene.exists() {
            pkg.entry_scene = Some(PathBuf::from("scenes/main.json"));
        }
        let text = serde_json::to_string_pretty(&pkg)?;
        std::fs::write(out_dir.join("package.json"), text)?;
        // Crash hook marker for packaged runs.
        std::fs::write(
            out_dir.join("README_PACKAGE.txt"),
            "Shiloh3D cooked package — run with crash reports under ./crashes/\n",
        )?;
        Ok(pkg)
    }
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), PackageError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
