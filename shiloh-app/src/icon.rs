//! Shared OS window / taskbar icon — same mark as in-app Shiloh3D branding.
//!
//! Source: repo-root `logo_shiloh3d.png` (faceted crimson **S**). Used by
//! Studio, demo, and the windowed host so the Linux/Windows/macOS taskbar
//! matches the product logo.

/// Wayland / X11 app id + `.desktop` `StartupWMClass` — keep in sync with
/// `packaging/linux/shiloh-editor.desktop`.
pub const SHILOH_APP_ID: &str = "shiloh3d-studio";

/// Embedded product logo (PNG).
pub const LOGO_PNG: &[u8] = include_bytes!("../../logo_shiloh3d.png");

/// Decode the Shiloh3D logo to RGBA pixels suitable for window icons.
///
/// Letterboxes to a square on transparent so the mark is not stretched
/// in the OS taskbar / dock.
pub fn logo_rgba_square(target_side: u32) -> (u32, u32, Vec<u8>) {
    let rgba = image::load_from_memory(LOGO_PNG)
        .expect("embedded Shiloh3D logo (logo_shiloh3d.png)")
        .into_rgba8();
    let (w, h) = rgba.dimensions();
    let side = w.max(h);
    let mut square = image::RgbaImage::from_pixel(side, side, image::Rgba([0, 0, 0, 0]));
    let ox = i64::from((side - w) / 2);
    let oy = i64::from((side - h) / 2);
    image::imageops::overlay(&mut square, &rgba, ox, oy);
    let thumb = image::imageops::thumbnail(&square, target_side, target_side);
    (target_side, target_side, thumb.into_raw())
}

/// `winit` window icon for the OS taskbar / dock / Alt-Tab.
#[cfg(feature = "window")]
pub fn window_icon() -> winit::window::Icon {
    let (w, h, rgba) = logo_rgba_square(256);
    winit::window::Icon::from_rgba(rgba, w, h).expect("Shiloh3D logo → winit Icon")
}

/// Shared `Arc<Icon>` for window attributes.
#[cfg(feature = "window")]
pub fn window_icon_arc() -> std::sync::Arc<winit::window::Icon> {
    std::sync::Arc::new(window_icon())
}
