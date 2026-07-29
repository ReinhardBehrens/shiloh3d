//! Command recording abstractions.

use crate::texture::TextureHandle;

#[derive(Debug, Clone)]
pub struct RenderPassDesc<'a> {
    pub color: &'a [TextureHandle],
    pub depth: Option<TextureHandle>,
    pub label: Option<&'static str>,
}

/// Backend-agnostic command encoder.
pub trait CommandEncoder {
    fn begin_render_pass(&mut self, desc: &RenderPassDesc<'_>);
    fn end_render_pass(&mut self);
}
