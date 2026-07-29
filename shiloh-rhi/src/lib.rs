//! Render Hardware Interface — backend-agnostic GPU types.
//!
//! # Policy
//!
//! - **Bootstrap:** wgpu + WGSL behind these traits (advised starting point)  
//! - **Shipping desktop:** native Vulkan / D3D12 / Metal on the same traits  
//! - **Web:** WebGL + WebGPU (wgpu)  
//! - **CI:** [`NullDevice`]  
//!
//! Do **not** re-export `wgpu` / `winit` from this crate’s public API for games.
//! See `docs/TECH_STACK.md` and `docs/GRAPHICS.md`.

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

#[cfg(feature = "native")]
pub mod native;

#[cfg(feature = "webgl")]
pub mod webgl;

pub use buffer::{BufferDesc, BufferHandle, BufferUsage};
pub use command::{CommandEncoder, RenderPassDesc};
pub use device::{Device, DeviceInfo, Queue};
pub use format::TextureFormat;
pub use null::NullDevice;
pub use texture::{TextureDesc, TextureHandle, TextureUsage};
