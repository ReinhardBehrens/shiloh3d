//! Godot-style editor layout save/restore (Phase 5).
//!
//! Borrowed from Godot 4: Editor Layouts under the project folder.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Dock widths / visibility persisted as JSON under `.shiloh/layouts/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorLayout {
    pub name: String,
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_height: f32,
    pub distraction_free: bool,
    pub grid_snap: bool,
    pub snap_size: f32,
}

impl Default for EditorLayout {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            left_width: 260.0,
            right_width: 300.0,
            bottom_height: 180.0,
            distraction_free: false,
            grid_snap: true,
            snap_size: 0.5,
        }
    }
}

impl EditorLayout {
    pub fn layouts_dir(project_root: &Path) -> PathBuf {
        project_root.join(".shiloh").join("layouts")
    }

    pub fn path_for(project_root: &Path, name: &str) -> PathBuf {
        let safe: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        Self::layouts_dir(project_root).join(format!("{safe}.json"))
    }

    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let dir = Self::layouts_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let path = Self::path_for(project_root, &self.name);
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }

    pub fn load(project_root: &Path, name: &str) -> std::io::Result<Self> {
        let path = Self::path_for(project_root, name);
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn list(project_root: &Path) -> Vec<String> {
        let dir = Self::layouts_dir(project_root);
        let Ok(rd) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_layout_json() {
        let dir = std::env::temp_dir().join("shiloh_layout_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let layout = EditorLayout {
            name: "Focus".into(),
            left_width: 200.0,
            distraction_free: true,
            ..Default::default()
        };
        layout.save(&dir).unwrap();
        let back = EditorLayout::load(&dir, "Focus").unwrap();
        assert!(back.distraction_free);
        assert!((back.left_width - 200.0).abs() < 1e-3);
        assert!(EditorLayout::list(&dir).contains(&"Focus".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
