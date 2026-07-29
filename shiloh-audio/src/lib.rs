//! Spatial audio and mixing — pure Rust mixer stub (cpal/rodio later, still Rust).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod mixer;
pub mod source;

pub use mixer::AudioMixer;
pub use source::{AudioSource, Listener};
