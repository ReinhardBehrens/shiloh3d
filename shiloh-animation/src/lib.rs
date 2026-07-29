//! Skeletons, clip blending, and animation state machines (pure Rust).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod blend;
pub mod clip;
pub mod skeleton;
pub mod state_machine;

pub use blend::BlendTree;
pub use clip::AnimationClip;
pub use skeleton::{Joint, Skeleton};
pub use state_machine::{AnimState, AnimStateMachine, AnimTransition};
