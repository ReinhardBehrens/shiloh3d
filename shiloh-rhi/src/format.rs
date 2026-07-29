//! Texture / buffer format enums (backend-neutral).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Depth32Float,
    Depth24PlusStencil8,
}
