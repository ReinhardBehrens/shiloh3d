//! Render Hardware Interface — backend-agnostic GPU types.
//!
//! Default build is **pure Rust** (`NullDevice`). Enable feature `wgpu` for the
//! wgpu backend (still Rust API; talks to platform graphics drivers).

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod buffer;
pub mod command;
pub mod device;
pub mod format;
pub mod null;
pub mod texture;

#[cfg(feature = "wgpu")]
pub mod wgpu_backend;

pub use buffer::{BufferDesc, BufferHandle, BufferUsage};
pub use command::{CommandEncoder, RenderPassDesc};
pub use device::{Device, DeviceInfo, Queue};
pub use format::TextureFormat;
pub use null::NullDevice;
pub use texture::{TextureDesc, TextureHandle, TextureUsage};
