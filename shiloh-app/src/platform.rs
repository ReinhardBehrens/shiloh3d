//! Platform integration hooks.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformKind {
    /// No OS window — CI, servers, tools.
    Headless,
    /// Native window via winit (feature `window`).
    Desktop,
}

pub fn detect_platform() -> PlatformKind {
    #[cfg(feature = "window")]
    {
        PlatformKind::Desktop
    }
    #[cfg(not(feature = "window"))]
    {
        PlatformKind::Headless
    }
}
