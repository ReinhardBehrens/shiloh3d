//! Importer trait for format-specific loaders (glTF, images, audio — all Rust crates later).

use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported format: {0}")]
    Unsupported(&'static str),
    #[error("{0}")]
    Message(String),
}

pub trait Importer: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &[&'static str];
    fn import(&self, path: &Path) -> Result<Vec<u8>, ImportError>;
}

/// Identity importer — reads raw bytes.
pub struct RawImporter;

impl Importer for RawImporter {
    fn name(&self) -> &'static str {
        "raw"
    }

    fn extensions(&self) -> &[&'static str] {
        &["*"]
    }

    fn import(&self, path: &Path) -> Result<Vec<u8>, ImportError> {
        Ok(std::fs::read(path)?)
    }
}
