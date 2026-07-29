//! Engine configuration loaded from TOML (pure Rust, optional `config` feature).

#[cfg(feature = "config")]
use serde::Deserialize;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config feature is disabled")]
    FeatureDisabled,
    #[error("failed to read config: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "config")]
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
}

/// Top-level engine configuration.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "config", derive(Deserialize))]
#[cfg_attr(feature = "config", serde(default))]
pub struct EngineConfig {
    pub app_name: String,
    pub fixed_update_hz: f64,
    pub job_workers: Option<usize>,
    pub assets_root: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            app_name: "Shiloh3D".into(),
            fixed_update_hz: 60.0,
            job_workers: None,
            assets_root: "assets".into(),
        }
    }
}

impl EngineConfig {
    /// Loads configuration from a TOML file.
    pub fn load_from_path(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        #[cfg(feature = "config")]
        {
            let text = std::fs::read_to_string(path)?;
            Ok(toml::from_str(&text)?)
        }
        #[cfg(not(feature = "config"))]
        {
            let _ = path;
            Err(ConfigError::FeatureDisabled)
        }
    }

    /// Parses configuration from a TOML string.
    pub fn parse_toml(text: &str) -> Result<Self, ConfigError> {
        #[cfg(feature = "config")]
        {
            Ok(toml::from_str(text)?)
        }
        #[cfg(not(feature = "config"))]
        {
            let _ = text;
            Err(ConfigError::FeatureDisabled)
        }
    }
}
