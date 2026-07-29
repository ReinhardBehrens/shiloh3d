//! WebGL backend — browser **reach** path (wasm32 / canvas).
//!
//! Complements the wgpu extension (WebGPU). Prefer WebGL when WebGPU is
//! unavailable; both are selected through the same RHI traits.

use crate::buffer::{BufferDesc, BufferHandle};
use crate::command::{CommandEncoder, RenderPassDesc};
use crate::device::{Device, DeviceError, DeviceInfo, Queue};
use crate::texture::{TextureDesc, TextureHandle};

/// Placeholder until `web_sys` / glow WebGL wiring lands.
pub struct WebGlDevice {
    info: DeviceInfo,
}

impl WebGlDevice {
    pub fn stub() -> Self {
        Self {
            info: DeviceInfo {
                name: "Shiloh WebGL (stub)".into(),
                backend: "webgl",
                is_software: false,
            },
        }
    }
}

struct WebGlQueue;
struct WebGlEncoder;

impl CommandEncoder for WebGlEncoder {
    fn begin_render_pass(&mut self, _desc: &RenderPassDesc<'_>) {}
    fn end_render_pass(&mut self) {}
}

impl Queue for WebGlQueue {
    fn submit(&self, _encoder: Box<dyn CommandEncoder>) {}
    fn present(&self) {}
}

impl Device for WebGlDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn create_buffer(&self, _desc: &BufferDesc) -> Result<BufferHandle, DeviceError> {
        Err(DeviceError::Backend(
            "webgl backend not wired yet".into(),
        ))
    }

    fn create_texture(&self, _desc: &TextureDesc) -> Result<TextureHandle, DeviceError> {
        Err(DeviceError::Backend(
            "webgl backend not wired yet".into(),
        ))
    }

    fn destroy_buffer(&self, _handle: BufferHandle) {}
    fn destroy_texture(&self, _handle: TextureHandle) {}

    fn create_encoder(&self) -> Box<dyn CommandEncoder> {
        Box::new(WebGlEncoder)
    }

    fn queue(&self) -> &dyn Queue {
        static Q: WebGlQueue = WebGlQueue;
        &Q
    }
}
