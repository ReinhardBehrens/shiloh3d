//! Standalone Shiloh3D editor binary.
//!
//! Premium docked shell (egui) with a live offscreen wgpu `SliceRenderer`
//! viewport — scene entities, orbit camera, selection gizmo.
//!
//! Usage: `shiloh-editor [PROJECT_DIR]` (defaults to `./shiloh_project`).

use eframe::egui;
use shiloh_editor::{EditorApp, Project};

/// Window / taskbar icon — same faceted crimson **S** as in-app branding
/// (`logo_shiloh3d.png` via `shiloh_app::logo_rgba_square`).
fn shiloh_window_icon() -> egui::IconData {
    let (w, h, rgba) = shiloh_app::logo_rgba_square(256);
    egui::IconData {
        rgba,
        width: w,
        height: h,
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
            // Wayland/X11: matches packaging/linux/*.desktop StartupWMClass.
            .with_app_id(shiloh_app::SHILOH_APP_ID)
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
