//! Skeletons, clip blending, and animation state machines (pure Rust).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod blend;
pub mod clip;
pub mod skeleton;
pub mod skin;
pub mod state_machine;

pub use blend::BlendTree;
pub use clip::{AnimationClip, JointTracks, QuatTrack, Vec3Track};
pub use skeleton::{Joint, Pose, Skeleton};
pub use skin::{SkinPalette, bind_palette};
pub use state_machine::{AnimState, AnimStateMachine, AnimTransition};
