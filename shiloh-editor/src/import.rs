//! Download assets from a URL into the project `assets/Imported` folder.

use std::io::Read;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("invalid URL")]
    InvalidUrl,
    #[error("HTTP {0}")]
    Http(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Download `url` into `project_root/assets/Imported/<filename>`.
///
/// Filename is taken from the URL path when possible, otherwise `import.bin`.
/// Allowed for meshes/textures commonly used in world building (.gltf, .glb,
/// .png, .jpg, …).
pub fn import_from_url(project_root: &Path, url: &str) -> Result<PathBuf, ImportError> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(ImportError::InvalidUrl);
    }

    let dest_dir = project_root.join("assets/Imported");
    std::fs::create_dir_all(&dest_dir)?;

    let filename = filename_from_url(url);
    let dest = unique_path(dest_dir.join(&filename));

    let response = ureq::get(url)
        .set("User-Agent", "Shiloh3D-Editor/0.1")
        .call()
        .map_err(|e| ImportError::Http(e.to_string()))?;

    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    if bytes.is_empty() {
        return Err(ImportError::Http("empty response body".into()));
    }
    std::fs::write(&dest, &bytes)?;
    Ok(dest)
}

fn filename_from_url(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    let name = path
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("import.bin");
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "import.bin".into()
    } else {
        safe
    }
}

fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("import");
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for i in 1..1000 {
        let candidate = parent.join(format!("{stem}_{i}{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}
