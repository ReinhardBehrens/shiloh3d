//! Shiloh3D CLI library — packaging helpers.

pub mod pack;

pub use pack::{DESKTOP_TARGETS, DEFAULT_BINS, DesktopTarget, PackOptions, pack_desktop};
