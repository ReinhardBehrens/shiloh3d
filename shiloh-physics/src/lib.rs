//! Physics abstraction — pluggable Rust backends (rapier later), stub world now.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod body;
pub mod world;

pub use body::{RigidBody, RigidBodyKind};
pub use world::{PhysicsBackend, PhysicsWorld, StubPhysics};
