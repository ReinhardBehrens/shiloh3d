//! Application lifecycle — headless by default (pure Rust), optional windowing.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod app;
pub mod lifecycle;
pub mod platform;

#[cfg(feature = "icon")]
pub mod icon;
#[cfg(feature = "window")]
pub mod windowed;
#[cfg(feature = "window")]
pub mod winit_map;

pub use app::{App, AppBuilder};
pub use lifecycle::Phase;

#[cfg(feature = "icon")]
pub use icon::{LOGO_PNG, SHILOH_APP_ID, logo_rgba_square};
#[cfg(feature = "window")]
pub use icon::{window_icon, window_icon_arc};
#[cfg(feature = "window")]
pub use windowed::{RhiBackendKind, run_windowed};
#[cfg(feature = "window")]
pub use winit_map::{map_key, map_mouse};
