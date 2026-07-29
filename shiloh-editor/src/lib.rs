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
pub mod ui;

pub use play_mode::{EditorMode, PlaySession};
pub use project::{Project, ProjectManifest};
pub use selection::Selection;
#[cfg(feature = "ui")]
pub use ui::EditorApp;
