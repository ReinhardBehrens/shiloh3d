//! Device and queue traits.

use thiserror::Error;

use crate::buffer::{BufferDesc, BufferHandle};
use crate::command::CommandEncoder;
use crate::texture::{TextureDesc, TextureHandle};

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("out of GPU memory")]
    OutOfMemory,
    #[error("invalid descriptor: {0}")]
    InvalidDescriptor(&'static str),
    #[error("{0}")]
    Backend(String),
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub backend: &'static str,
    pub is_software: bool,
}

pub trait Queue {
    fn submit(&self, encoder: Box<dyn CommandEncoder>);
    fn present(&self);
}

pub trait Device: Send + Sync {
    fn info(&self) -> &DeviceInfo;
    fn create_buffer(&self, desc: &BufferDesc) -> Result<BufferHandle, DeviceError>;
    fn create_texture(&self, desc: &TextureDesc) -> Result<TextureHandle, DeviceError>;
    fn destroy_buffer(&self, handle: BufferHandle);
    fn destroy_texture(&self, handle: TextureHandle);
    fn create_encoder(&self) -> Box<dyn CommandEncoder>;
    fn queue(&self) -> &dyn Queue;
}
