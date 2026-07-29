//! Asset package manifest (JSON, pure Rust serde).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPackage {
    pub name: String,
    pub version: String,
    pub assets: Vec<PathBuf>,
}

impl AssetPackage {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".into(),
            assets: Vec::new(),
        }
    }
}
