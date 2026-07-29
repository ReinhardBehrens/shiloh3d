//! Spatial audio and mixing.
//!
//! Public mixer/listener API is Shiloh-owned. Kira / cpal / other native backends
//! plug in behind this crate — do not re-export them from the public API.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod mixer;
pub mod source;

pub use mixer::AudioMixer;
pub use source::{AudioClip, AudioSource, Listener};
