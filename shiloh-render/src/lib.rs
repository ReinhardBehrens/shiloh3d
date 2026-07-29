//! Render graph and high-level frame renderer (pure Rust over `shiloh-rhi`).
//!
//! Enable feature `wgpu` for the cross-platform forward GPU path (Windows / macOS / Linux).

#![cfg_attr(not(feature = "wgpu"), forbid(unsafe_code))]
#![deny(rust_2018_idioms)]

pub mod frame;
pub mod graph;
pub mod renderer;

#[cfg(feature = "wgpu")]
pub mod mesh;
#[cfg(feature = "wgpu")]
pub mod gpu;

pub use frame::FrameContext;
pub use graph::{PassNode, RenderGraph, ResourceId};
pub use renderer::Renderer;

#[cfg(feature = "wgpu")]
pub use gpu::ForwardRenderer;
#[cfg(feature = "wgpu")]
pub use mesh::{InstanceRaw, MeshCpu, Vertex};
