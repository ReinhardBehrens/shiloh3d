//! Blender→glTF cook stubs: collision hull + LOD distance metadata (Phase 5).
//!
//! See [`docs/BLENDER_PIPELINE.md`]. Writes `*.shiloh.json` beside a mesh path.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Collision proxy authored or auto-generated from an AABB.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CollisionStub {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    None,
}

/// One LOD rung — distance in world units, mesh path relative to project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LodStub {
    pub distance: f32,
    pub mesh: String,
}

/// Cook metadata beside a glTF (`pine_hero.glb` → `pine_hero.shiloh.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetCookStub {
    pub source: String,
    pub collision: CollisionStub,
    pub lod: Vec<LodStub>,
}

impl AssetCookStub {
    /// Build a box-hull + single-LOD stub from source path and AABB half-extents.
    pub fn from_aabb(source: impl Into<String>, half_extents: [f32; 3]) -> Self {
        let source = source.into();
        let mesh_name = Path::new(&source)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("mesh.glb")
            .to_string();
        Self {
            collision: CollisionStub::Box { half_extents },
            lod: vec![LodStub {
                distance: 0.0,
                mesh: mesh_name,
            }],
            source,
        }
    }

    pub fn meta_path_for(mesh_path: &Path) -> PathBuf {
        let stem = mesh_path.file_stem().and_then(|s| s.to_str()).unwrap_or("asset");
        mesh_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{stem}.shiloh.json"))
    }

    pub fn save_beside(&self, mesh_path: &Path) -> std::io::Result<PathBuf> {
        let path = Self::meta_path_for(mesh_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&path, text)?;
        Ok(path)
    }

    pub fn load_beside(mesh_path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(Self::meta_path_for(mesh_path))?;
        serde_json::from_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Prefer `*_thumb.png` next to the mesh; else `None`.
    pub fn thumbnail_path(mesh_path: &Path) -> Option<PathBuf> {
        let stem = mesh_path.file_stem()?.to_str()?;
        let candidate = mesh_path
            .parent()?
            .join(format!("{stem}_thumb.png"));
        candidate.is_file().then_some(candidate)
    }
}

/// Ensure a cook stub exists for `mesh_path`, generating a box hull when missing.
pub fn ensure_cook_stub(mesh_path: &Path, half_extents: [f32; 3]) -> std::io::Result<AssetCookStub> {
    let meta = AssetCookStub::meta_path_for(mesh_path);
    if meta.is_file() {
        return AssetCookStub::load_beside(mesh_path);
    }
    let source = mesh_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mesh.glb")
        .to_string();
    let stub = AssetCookStub::from_aabb(source, half_extents);
    stub.save_beside(mesh_path)?;
    Ok(stub)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cook_stub_roundtrip() {
        let dir = std::env::temp_dir().join("shiloh_cook_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mesh = dir.join("pine_hero.glb");
        std::fs::write(&mesh, b"fake").unwrap();
        let stub = ensure_cook_stub(&mesh, [0.8, 2.4, 0.8]).unwrap();
        assert!(matches!(stub.collision, CollisionStub::Box { .. }));
        assert_eq!(stub.lod.len(), 1);
        let back = AssetCookStub::load_beside(&mesh).unwrap();
        assert_eq!(back, stub);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
