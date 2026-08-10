//! Scene editor and project management.
//!
//! First UI toolkit target: **egui**, confined to this crate. Games and tools should
//! depend on `shiloh-editor` types (project, selection), not on egui directly.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod play_mode;
pub mod project;
pub mod selection;
#[cfg(feature = "ui")]
pub mod asset_cook;
#[cfg(feature = "ui")]
pub mod import;
#[cfg(feature = "ui")]
pub mod layouts;
#[cfg(feature = "ui")]
pub mod node_graph;
#[cfg(feature = "ui")]
pub mod script_editor;
#[cfg(feature = "ui")]
pub mod ui;
#[cfg(feature = "ui")]
pub mod gltf_mesh;
#[cfg(feature = "ui")]
pub mod viewport;
#[cfg(feature = "ui")]
pub mod world_items;

pub use play_mode::{EditorMode, PlaySession};
pub use project::{Project, ProjectManifest};
pub use selection::Selection;
#[cfg(feature = "ui")]
pub use asset_cook::{ensure_cook_stub, AssetCookStub, CollisionStub, LodStub};
#[cfg(feature = "ui")]
pub use layouts::EditorLayout;
#[cfg(feature = "ui")]
pub use ui::EditorApp;
