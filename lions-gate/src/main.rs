//! The Lions Gate — Christian Diablo-style ARPG vertical slice.
//!
//! # World
//! One contiguous campaign map: **Town** ↔ **Forest** ↔ **Swamp** (first level).
//!
//! # Loop
//! Explore → defeat foes → loot → inventory → return to town / advance.
//!
//! # HUD
//! Health + inventory + Bible — **no mana**. Skills use short cooldowns (prayer / strike).
//!
//! Menu chrome follows `docs/references/lions-gate-main-menu.png`.

mod bible;
mod character;
mod game;
mod hud;
mod loot;
mod menu;
mod world;

use eframe::egui;
use shiloh_core::logging;

use crate::game::LionsGateApp;

fn window_icon() -> egui::IconData {
    let (w, h, rgba) = shiloh_app::logo_rgba_square(256);
    egui::IconData {
        rgba,
        width: w,
        height: h,
    }
}

fn main() -> eframe::Result<()> {
    logging::init();

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1024.0, 640.0])
            .with_title("The Lions Gate")
            .with_app_id(shiloh_app::SHILOH_APP_ID)
            .with_icon(window_icon()),
        ..Default::default()
    };
    options.centered = true;

    eframe::run_native(
        "The Lions Gate",
        options,
        Box::new(|_cc| Ok(Box::new(LionsGateApp::new()))),
    )
}
