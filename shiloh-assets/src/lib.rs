//! Asset importing, handle cache, packages, optional hot reload.
//!
//! Pure Rust by default.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod cache;
pub mod handle;
pub mod importer;
pub mod material;
pub mod package;

#[cfg(feature = "gltf")]
pub mod gltf_import;

#[cfg(feature = "hot-reload")]
pub mod hot_reload;

pub use cache::AssetCache;
pub use handle::{AssetId, AssetState};
pub use importer::{ImportError, Importer};
pub use material::{MaterialAsset, MaterialError};
pub use package::{AssetPackage, PackageError};

#[cfg(feature = "gltf")]
pub use gltf_import::{
    ImportedAnimation, ImportedGltf, ImportedMaterial, ImportedPrimitive, ImportedSkin,
    ImportedVertex, GltfError, load_gltf,
};

#[cfg(feature = "hot-reload")]
pub use hot_reload::HotReloader;
