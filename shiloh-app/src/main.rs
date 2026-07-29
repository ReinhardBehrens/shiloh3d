//! Shiloh3D runtime binary (headless by default).

use shiloh_app::App;
use shiloh_core::EngineConfig;

fn main() -> anyhow::Result<()> {
    let config = EngineConfig::default();
    App::builder()
        .config(config)
        .max_frames(3)
        .build()
        .run()
}
