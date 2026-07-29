//! Frame-scoped render context.

use shiloh_rhi::Device;

/// Per-frame rendering inputs.
pub struct FrameContext<'a> {
    pub device: &'a dyn Device,
    pub frame_index: u64,
    pub width: u32,
    pub height: u32,
}
