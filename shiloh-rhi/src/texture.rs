//! GPU texture descriptors and handles.

use bitflags::bitflags;
use shiloh_core::Handle;

use crate::format::TextureFormat;

#[derive(Debug, Clone, Copy, Default)]
pub struct TextureTag;

pub type TextureHandle = Handle<TextureTag>;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TextureUsage: u32 {
        const SAMPLED      = 1 << 0;
        const STORAGE      = 1 << 1;
        const RENDER_TARGET = 1 << 2;
        const DEPTH_STENCIL = 1 << 3;
        const COPY_SRC     = 1 << 4;
        const COPY_DST     = 1 << 5;
    }
}

#[derive(Debug, Clone)]
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
    pub mip_levels: u32,
    pub format: TextureFormat,
    pub usage: TextureUsage,
    pub label: Option<&'static str>,
}
