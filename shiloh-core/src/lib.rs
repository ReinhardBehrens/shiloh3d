//! Shiloh3D core: IDs, time, logging, jobs, and configuration.
//!
//! Pure Rust foundation shared by every engine crate. No FFI, no C bindings.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod config;
pub mod frame_alloc;
pub mod handle;
pub mod jobs;
pub mod logging;
pub mod time;

pub use config::{ConfigError, EngineConfig};
pub use frame_alloc::FrameAllocator;
pub use handle::{Handle, HandleAllocator};
pub use jobs::{JobHandle, JobSystem, JobSystemBuilder};
pub use time::{FixedTimestep, FrameTime, Instant, Time};
