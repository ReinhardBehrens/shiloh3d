//! Scene editor and project management (pure Rust data model; UI later).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod project;
pub mod selection;

pub use project::{Project, ProjectManifest};
pub use selection::Selection;
