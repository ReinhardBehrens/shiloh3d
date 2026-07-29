//! Render graph and high-level frame renderer over `shiloh-rhi`.
//!
//! Feature `wgpu` enables the forward GPU path used by the showcase (**extension**).
//! Shipping desktop titles target the **native** RHI backend; web targets WebGL / WebGPU.
//! See `docs/GRAPHICS.md`.

#![cfg_attr(not(feature = "wgpu"), forbid(unsafe_code))]
#![deny(rust_2018_idioms)]

pub mod frame;
pub mod graph;
pub mod renderer;

#[cfg(feature = "wgpu")]
pub mod mesh;
#[cfg(feature = "wgpu")]
pub mod gpu;
#[cfg(feature = "wgpu")]
pub mod slice;

pub use frame::FrameContext;
pub use graph::{PassNode, RenderGraph, ResourceId};
pub use renderer::Renderer;

#[cfg(feature = "wgpu")]
pub use gpu::ForwardRenderer;
#[cfg(feature = "wgpu")]
pub use mesh::{InstanceRaw, MeshCpu, Vertex};
#[cfg(feature = "wgpu")]
pub use slice::{HudVertex, SliceDrawParams, SliceRenderer, orthographic_light_matrix};
