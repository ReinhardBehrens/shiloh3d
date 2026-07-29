//! Keyboard, mouse, controller, touch — double-buffered, platform-agnostic.
//!
//! Pure Rust state machine; platform backends feed events from `shiloh-app`.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod action;
pub mod device;
pub mod state;

pub use action::{Action, ActionMap};
pub use device::{GamepadButton, KeyCode, MouseButton};
pub use state::InputState;
