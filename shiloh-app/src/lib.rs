//! Application lifecycle — headless by default (pure Rust), optional windowing.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod app;
pub mod lifecycle;
pub mod platform;

pub use app::{App, AppBuilder};
pub use lifecycle::Phase;
