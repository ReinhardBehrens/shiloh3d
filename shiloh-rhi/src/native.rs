//! Native GPU backend — **primary** shipping path (Vulkan / D3D12 / Metal).
//!
//! Scaffold only. Real device creation lands behind OS-specific crates;
//! `wgpu` remains an optional extension, not a replacement for this module.

use crate::buffer::{BufferDesc, BufferHandle};
use crate::command::{CommandEncoder, RenderPassDesc};
use crate::device::{Device, DeviceError, DeviceInfo, Queue};
use crate::texture::{TextureDesc, TextureHandle};

/// Placeholder until ash / d3d12 / metal wiring lands.
pub struct NativeDevice {
    info: DeviceInfo,
}

impl NativeDevice {
    pub fn stub_for_platform() -> Self {
        Self {
            info: DeviceInfo {
                name: "Shiloh Native (stub)".into(),
                backend: "native",
                is_software: false,
            },
        }
    }
}

struct NativeQueue;
struct NativeEncoder;

impl CommandEncoder for NativeEncoder {
    fn begin_render_pass(&mut self, _desc: &RenderPassDesc<'_>) {}
    fn end_render_pass(&mut self) {}
}

impl Queue for NativeQueue {
    fn submit(&self, _encoder: Box<dyn CommandEncoder>) {}
    fn present(&self) {}
}

impl Device for NativeDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn create_buffer(&self, _desc: &BufferDesc) -> Result<BufferHandle, DeviceError> {
        Err(DeviceError::Backend(
            "native backend not wired yet — use wgpu extension or null for now".into(),
        ))
    }

    fn create_texture(&self, _desc: &TextureDesc) -> Result<TextureHandle, DeviceError> {
        Err(DeviceError::Backend(
            "native backend not wired yet — use wgpu extension or null for now".into(),
        ))
    }

    fn destroy_buffer(&self, _handle: BufferHandle) {}
    fn destroy_texture(&self, _handle: TextureHandle) {}

    fn create_encoder(&self) -> Box<dyn CommandEncoder> {
        Box::new(NativeEncoder)
    }

    fn queue(&self) -> &dyn Queue {
        static Q: NativeQueue = NativeQueue;
        &Q
    }
}
