//! Physics abstraction — pluggable backends.
//!
//! Public API is Shiloh-owned (`PhysicsBackend`, `RigidBody`, …).
//! Rapier (or any other solver) must stay behind this façade — do not re-export it.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod body;
pub mod world;

pub use body::{RigidBody, RigidBodyKind};
pub use world::{PhysicsBackend, PhysicsWorld, StubPhysics};
