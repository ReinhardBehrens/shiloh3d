//! Standalone Shiloh3D editor binary.
//!
//! Premium docked shell (egui / glow). Live 3D still runs in `shiloh-demo`'s
//! wgpu window; this process owns outliner, inspector, node graph, assets,
//! and URL import.
//!
//! Usage: `shiloh-editor [PROJECT_DIR]` (defaults to `./shiloh_project`).

use eframe::egui;
use shiloh_editor::{EditorApp, Project};

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
            .with_title("Shiloh3D Studio"),
        ..Default::default()
    };
    options.centered = true;

    eframe::run_native(
        "Shiloh3D Studio",
        options,
        Box::new(move |_cc| Ok(Box::new(EditorApp::new(Some(project))))),
    )
}
