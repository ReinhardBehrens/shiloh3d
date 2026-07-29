//! Scenes, hierarchy, transforms, and prefabs (pure Rust + glam).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod camera;
pub mod hierarchy;
pub mod prefab;
pub mod scene;
pub mod serialize;
pub mod transform;

pub use camera::{Camera, ProjectionKind};
pub use hierarchy::{Children, Parent, propagate_transforms, set_parent};
pub use prefab::Prefab;
pub use scene::Scene;
pub use serialize::{
    CameraRecord, EntityRecord, SceneFile, SceneSerdeError, load_scene, save_scene,
};
pub use transform::{GlobalTransform, Transform};
