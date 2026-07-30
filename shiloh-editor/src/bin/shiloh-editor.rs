//! Standalone Shiloh3D editor binary.
//!
//! Premium docked shell (egui) with a live offscreen wgpu `SliceRenderer`
//! viewport — scene entities, orbit camera, selection gizmo.
//!
//! Usage: `shiloh-editor [PROJECT_DIR]` (defaults to `./shiloh_project`).

use eframe::egui;
use shiloh_editor::{EditorApp, Project};

/// Window / taskbar icon — faceted crimson **S** mark (`logo_shiloh3d.png`).
fn shiloh_window_icon() -> egui::IconData {
    const LOGO_PNG: &[u8] = include_bytes!("../../../logo_shiloh3d.png");
    let rgba = image::load_from_memory(LOGO_PNG)
        .expect("embedded Shiloh3D logo")
        .into_rgba8();
    let (w, h) = rgba.dimensions();
    let side = w.max(h);
    let mut square = image::RgbaImage::from_pixel(side, side, image::Rgba([0, 0, 0, 0]));
    let ox = ((side - w) / 2) as i64;
    let oy = ((side - h) / 2) as i64;
    image::imageops::overlay(&mut square, &rgba, ox, oy);
    let size = 256u32;
    let thumb = image::imageops::thumbnail(&square, size, size);
    egui::IconData {
        rgba: thumb.into_raw(),
        width: size,
        height: size,
    }
}

fn main() -> eframe::Result<()> {
    let project_root = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("shiloh_project"));

    let project = Project::load(&project_root).unwrap_or_else(|_| {
        Project::create(&project_root, "Shiloh3D Project")
            .expect("failed to create editor project directory")
    });

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_title("Shiloh3D Studio")
            .with_icon(shiloh_window_icon()),
        ..Default::default()
    };
    options.centered = true;

    eframe::run_native(
        "Shiloh3D Studio",
        options,
        Box::new(move |_cc| Ok(Box::new(EditorApp::new(Some(project))))),
    )
}
