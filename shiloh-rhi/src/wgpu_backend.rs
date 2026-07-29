//! Optional wgpu backend (enable with `--features wgpu`).
//!
//! Still a Rust crate; links to platform graphics drivers at runtime.

#![cfg(feature = "wgpu")]

use crate::device::{Device, DeviceError, DeviceInfo, Queue};
use crate::buffer::{BufferDesc, BufferHandle};
use crate::command::{CommandEncoder, RenderPassDesc};
use crate::texture::{TextureDesc, TextureHandle};

/// Placeholder wgpu device — filled in when the GPU path is wired.
pub struct WgpuDevice {
    info: DeviceInfo,
}

impl WgpuDevice {
    pub fn stub() -> Self {
        Self {
            info: DeviceInfo {
                name: "wgpu (stub)".into(),
                backend: "wgpu",
                is_software: false,
            },
        }
    }
}

struct WgpuQueue;
struct WgpuEncoder;

impl CommandEncoder for WgpuEncoder {
    fn begin_render_pass(&mut self, _desc: &RenderPassDesc<'_>) {}
    fn end_render_pass(&mut self) {}
}

impl Queue for WgpuQueue {
    fn submit(&self, _encoder: Box<dyn CommandEncoder>) {}
    fn present(&self) {}
}

impl Device for WgpuDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn create_buffer(&self, _desc: &BufferDesc) -> Result<BufferHandle, DeviceError> {
        Err(DeviceError::Backend(
            "wgpu backend not fully wired yet".into(),
        ))
    }

    fn create_texture(&self, _desc: &TextureDesc) -> Result<TextureHandle, DeviceError> {
        Err(DeviceError::Backend(
            "wgpu backend not fully wired yet".into(),
        ))
    }

    fn destroy_buffer(&self, _handle: BufferHandle) {}
    fn destroy_texture(&self, _handle: TextureHandle) {}

    fn create_encoder(&self) -> Box<dyn CommandEncoder> {
        Box::new(WgpuEncoder)
    }

    fn queue(&self) -> &dyn Queue {
        // Leak a static queue for the stub — real impl owns the queue.
        static Q: WgpuQueue = WgpuQueue;
        &Q
    }
}
