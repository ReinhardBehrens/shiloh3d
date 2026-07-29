//! Standalone Shiloh3D editor binary.
//!
//! Uses `eframe` (egui, `glow` backend) for editor chrome. This process is
//! deliberately separate from `shiloh-demo`'s wgpu `SliceRenderer` window —
//! see `docs/TECH_STACK.md` for why editor UI and the game's GPU surface
//! don't currently share a window.
//!
//! Usage: `shiloh-editor [PROJECT_DIR]` (defaults to `./shiloh_project`).

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

    eframe::run_native(
        "Shiloh3D Editor",
        eframe::NativeOptions::default(),
        Box::new(move |_cc| Ok(Box::new(EditorApp::new(Some(project))))),
    )
}
