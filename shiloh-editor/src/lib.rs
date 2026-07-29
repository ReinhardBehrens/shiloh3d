//! Scene editor and project management.
//!
//! First UI toolkit target: **egui**, confined to this crate. Games and tools should
//! depend on `shiloh-editor` types (project, selection), not on egui directly.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod project;
pub mod selection;

pub use project::{Project, ProjectManifest};
pub use selection::Selection;
