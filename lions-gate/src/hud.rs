//! In-play HUD — health, XP, inventory, zone — **no mana bar**.

use egui::{self, Color32, RichText, Vec2};

use crate::bible::BibleState;
use crate::character::Hero;
use crate::loot::Inventory;
use crate::world::ZoneId;

const GOLD: Color32 = Color32::from_rgb(212, 175, 55);

pub struct HudFlags {
    pub inventory_open: bool,
    pub map_open: bool,
}

impl Default for HudFlags {
    fn default() -> Self {
        Self {
            inventory_open: false,
            map_open: false,
        }
    }
}

pub fn draw_play_hud(
    ui: &mut egui::Ui,
    hero: &Hero,
    zone: ZoneId,
    inv: &Inventory,
    bible: &BibleState,
    message: &str,
    flags: &mut HudFlags,
) {
    let full = ui.max_rect();
    let painter = ui.painter();

    // Top-left vitals — Diablo-like orb strip, health only (no mana)
    let panel = egui::Rect::from_min_size(full.min + Vec2::new(16.0, 12.0), Vec2::new(280.0, 86.0));
    painter.rect_filled(panel, 6.0, Color32::from_rgba_unmultiplied(10, 10, 12, 210));
    painter.rect_stroke(
        panel,
        6.0,
        egui::Stroke::new(1.0, GOLD),
        egui::StrokeKind::Outside,
    );

    let hp_frac = (hero.health / hero.max_health).clamp(0.0, 1.0);
    let hp_bar = egui::Rect::from_min_size(panel.min + Vec2::new(14.0, 36.0), Vec2::new(250.0, 14.0));
    painter.rect_filled(hp_bar, 3.0, Color32::from_rgb(40, 20, 20));
    painter.rect_filled(
        egui::Rect::from_min_size(hp_bar.min, Vec2::new(hp_bar.width() * hp_frac, hp_bar.height())),
        3.0,
        Color32::from_rgb(180, 40, 40),
    );

    let xp_frac = hero.xp as f32 / hero.xp_to_level as f32;
    let xp_bar = egui::Rect::from_min_size(panel.min + Vec2::new(14.0, 56.0), Vec2::new(250.0, 8.0));
    painter.rect_filled(xp_bar, 2.0, Color32::from_rgb(30, 30, 40));
    painter.rect_filled(
        egui::Rect::from_min_size(xp_bar.min, Vec2::new(xp_bar.width() * xp_frac, xp_bar.height())),
        2.0,
        Color32::from_rgb(80, 140, 220),
    );

    painter.text(
        panel.min + Vec2::new(14.0, 10.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{} · {}  Lv{}   HP {:.0}/{:.0}",
            hero.name,
            hero.class.display_name(),
            hero.level,
            hero.health,
            hero.max_health
        ),
        egui::FontId::proportional(13.0),
        GOLD,
    );

    // Top-right zone + gold
    let right = egui::Rect::from_min_size(
        egui::pos2(full.max.x - 260.0, full.min.y + 12.0),
        Vec2::new(244.0, 64.0),
    );
    painter.rect_filled(right, 6.0, Color32::from_rgba_unmultiplied(10, 10, 12, 210));
    painter.text(
        right.min + Vec2::new(12.0, 10.0),
        egui::Align2::LEFT_TOP,
        zone.display_name(),
        egui::FontId::proportional(14.0),
        GOLD,
    );
    painter.text(
        right.min + Vec2::new(12.0, 34.0),
        egui::Align2::LEFT_TOP,
        format!(
            "Gold {} · Pack {}/{} · Prayer {:.0}s",
            hero.gold,
            inv.occupied(),
            inv.capacity,
            hero.prayer_cd
        ),
        egui::FontId::proportional(12.0),
        Color32::LIGHT_GRAY,
    );

    // Bottom action bar — no mana gem
    let bar = egui::Rect::from_center_size(
        egui::pos2(full.center().x, full.max.y - 48.0),
        Vec2::new(420.0, 56.0),
    );
    painter.rect_filled(bar, 8.0, Color32::from_rgba_unmultiplied(12, 12, 14, 220));
    painter.rect_stroke(bar, 8.0, egui::Stroke::new(1.0, GOLD), egui::StrokeKind::Outside);
    painter.text(
        bar.center(),
        egui::Align2::CENTER_CENTER,
        "LMB Strike · Space Prayer · F Loot · I Inventory · M Map · B Bible · Esc Menu",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(200, 190, 160),
    );

    // Floating message
    if !message.is_empty() {
        painter.text(
            egui::pos2(full.center().x, full.max.y - 100.0),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(15.0),
            Color32::from_rgb(255, 230, 160),
        );
    }

    // Inventory panel
    if flags.inventory_open {
        egui::Window::new("Inventory")
            .anchor(egui::Align2::RIGHT_CENTER, [-20.0, 0.0])
            .resizable(false)
            .default_width(260.0)
            .show(ui.ctx(), |ui| {
                ui.label(
                    RichText::new("Hero pack — click item for gold value")
                        .small()
                        .weak(),
                );
                for (i, slot) in inv.slots.iter().enumerate() {
                    match slot {
                        Some(item) => {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{i:02}"))
                                        .monospace()
                                        .small()
                                        .weak(),
                                );
                                ui.label(
                                    RichText::new(&item.name)
                                        .color(item.rarity.color())
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} · +{} · {}g",
                                        item.rarity.label(),
                                        item.power,
                                        item.gold_value
                                    ))
                                    .small()
                                    .weak(),
                                );
                            });
                        }
                        None => {
                            ui.label(RichText::new(format!("{i:02}  — empty —")).weak().small());
                        }
                    }
                }
                if ui.button("Close (I)").clicked() {
                    flags.inventory_open = false;
                }
            });
    }

    // World map overlay
    if flags.map_open {
        egui::Window::new("Campaign Map — First Level")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .default_width(420.0)
            .show(ui.ctx(), |ui| {
                ui.label("One world: Town ↔ Forest ↔ Swamp");
                ui.separator();
                for z in [ZoneId::Town, ZoneId::Forest, ZoneId::Swamp] {
                    let here = z == zone;
                    ui.label(
                        RichText::new(format!(
                            "{} {} — {}",
                            if here { "▶" } else { "·" },
                            z.display_name(),
                            z.blurb()
                        ))
                        .color(if here { GOLD } else { Color32::LIGHT_GRAY }),
                    );
                }
                ui.separator();
                ui.label(RichText::new("Walk into gate markers to travel.").small().weak());
                if ui.button("Close (M)").clicked() {
                    flags.map_open = false;
                }
            });
    }

    // Bible overlay while playing
    if bible.open {
        let v = bible.current();
        egui::Window::new("Bible")
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .default_width(480.0)
            .show(ui.ctx(), |ui| {
                ui.label(RichText::new(v.ref_).color(GOLD).strong().size(16.0));
                ui.label(RichText::new(v.text).size(14.0));
                ui.label(
                    RichText::new("B closes · use menu Bible for navigation")
                        .small()
                        .weak(),
                );
            });
    }
}
