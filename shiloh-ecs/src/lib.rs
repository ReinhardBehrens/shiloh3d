//! Archetype ECS — cache-friendly SoA storage, systems, and a stage schedule.
//!
//! Pure Rust. No FFI.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod component;
pub mod entity;
pub mod query;
pub mod schedule;
pub mod storage;
pub mod system;
pub mod world;

pub use component::{Component, ComponentId, ComponentRegistry};
pub use entity::Entity;
pub use query::Query;
pub use schedule::{Schedule, Stage};
pub use system::{System, SystemFn};
pub use world::World;
