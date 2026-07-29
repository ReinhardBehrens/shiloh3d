//! Scenes, hierarchy, transforms, and prefabs (pure Rust + glam).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod hierarchy;
pub mod prefab;
pub mod scene;
pub mod transform;
pub mod camera;

pub use camera::Camera;
pub use hierarchy::{Children, Parent};
pub use prefab::Prefab;
pub use scene::Scene;
pub use transform::{GlobalTransform, Transform};
