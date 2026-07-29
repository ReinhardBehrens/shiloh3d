//! GPU buffer descriptors and handles.

use bitflags::bitflags;
use shiloh_core::Handle;

#[derive(Debug, Clone, Copy, Default)]
pub struct BufferTag;

pub type BufferHandle = Handle<BufferTag>;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BufferUsage: u32 {
        const VERTEX   = 1 << 0;
        const INDEX    = 1 << 1;
        const UNIFORM  = 1 << 2;
        const STORAGE  = 1 << 3;
        const COPY_SRC = 1 << 4;
        const COPY_DST = 1 << 5;
    }
}

#[derive(Debug, Clone)]
pub struct BufferDesc {
    pub size: u64,
    pub usage: BufferUsage,
    pub label: Option<&'static str>,
}
