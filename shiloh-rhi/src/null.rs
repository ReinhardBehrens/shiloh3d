//! Pure-Rust null device — compiles everywhere, useful for headless / CI / tooling.

use parking_lot::Mutex;
use shiloh_core::HandleAllocator;

use crate::buffer::{BufferDesc, BufferHandle, BufferTag};
use crate::command::{CommandEncoder, RenderPassDesc};
use crate::device::{Device, DeviceError, DeviceInfo, Queue};
use crate::texture::{TextureDesc, TextureHandle, TextureTag};

struct NullEncoder;

impl CommandEncoder for NullEncoder {
    fn begin_render_pass(&mut self, _desc: &RenderPassDesc<'_>) {}
    fn end_render_pass(&mut self) {}
}

struct NullQueue;

impl Queue for NullQueue {
    fn submit(&self, _encoder: Box<dyn CommandEncoder>) {}
    fn present(&self) {}
}

/// Software / no-op GPU device.
pub struct NullDevice {
    info: DeviceInfo,
    buffers: Mutex<HandleAllocator<BufferTag>>,
    textures: Mutex<HandleAllocator<TextureTag>>,
    queue: NullQueue,
}

impl Default for NullDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl NullDevice {
    pub fn new() -> Self {
        Self {
            info: DeviceInfo {
                name: "Shiloh Null Device".into(),
                backend: "null",
                is_software: true,
            },
            buffers: Mutex::new(HandleAllocator::new()),
            textures: Mutex::new(HandleAllocator::new()),
            queue: NullQueue,
        }
    }
}

impl Device for NullDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn create_buffer(&self, desc: &BufferDesc) -> Result<BufferHandle, DeviceError> {
        if desc.size == 0 {
            return Err(DeviceError::InvalidDescriptor("buffer size must be > 0"));
        }
        Ok(self.buffers.lock().alloc())
    }

    fn create_texture(&self, desc: &TextureDesc) -> Result<TextureHandle, DeviceError> {
        if desc.width == 0 || desc.height == 0 {
            return Err(DeviceError::InvalidDescriptor("texture extent must be > 0"));
        }
        Ok(self.textures.lock().alloc())
    }

    fn destroy_buffer(&self, handle: BufferHandle) {
        self.buffers.lock().free(handle);
    }

    fn destroy_texture(&self, handle: TextureHandle) {
        self.textures.lock().free(handle);
    }

    fn create_encoder(&self) -> Box<dyn CommandEncoder> {
        Box::new(NullEncoder)
    }

    fn queue(&self) -> &dyn Queue {
        &self.queue
    }
}
