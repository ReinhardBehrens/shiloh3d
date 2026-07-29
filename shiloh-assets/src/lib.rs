//! Asset importing, handle cache, packages, optional hot reload.
//!
//! Pure Rust by default.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod cache;
pub mod handle;
pub mod importer;
pub mod package;

#[cfg(feature = "hot-reload")]
pub mod hot_reload;

pub use cache::AssetCache;
pub use handle::{AssetId, AssetState};
pub use importer::{ImportError, Importer};
pub use package::AssetPackage;
