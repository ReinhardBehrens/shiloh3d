//! Main menu + character select — gold/dark chrome from Lions Gate reference.

use egui::{self, Color32, RichText, Sense, Vec2};

use crate::character::HeroClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Continue,
    NewCampaign,
    ChurchService,
    Multiplayer,
    Bible,
    Options,
    Credits,
    Exit,
}

const GOLD: Color32 = Color32::from_rgb(212, 175, 55);
const GOLD_DIM: Color32 = Color32::from_rgb(160, 130, 50);
const PANEL: Color32 = Color32::from_rgb(18, 16, 14);
const BANNER: Color32 = Color32::from_rgb(12, 10, 10);

pub fn draw_main_menu(ui: &mut egui::Ui, has_save: bool) -> Option<MenuAction> {
    let mut action = None;
    let rect = ui.max_rect();
    let painter = ui.painter();

    // Warm celestial backdrop
    painter.rect_filled(rect, 0.0, Color32::from_rgb(28, 22, 14));
    painter.rect_filled(
        egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), rect.height() * 0.55)),
        0.0,
        Color32::from_rgb(48, 36, 18),
    );

    // Side banners
    let banner_w = 70.0;
    painter.rect_filled(
        egui::Rect::from_min_size(rect.min + Vec2::new(24.0, 40.0), Vec2::new(banner_w, 280.0)),
        4.0,
        BANNER,
    );
    painter.rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(rect.max.x - 24.0 - banner_w, rect.min.y + 40.0),
            Vec2::new(banner_w, 280.0),
        ),
        4.0,
        BANNER,
    );
    painter.text(
        rect.min + Vec2::new(36.0, 80.0),
        egui::Align2::LEFT_TOP,
        "BE STRONG\nIN THE LORD\n\nEPH 6:10",
        egui::FontId::proportional(11.0),
        GOLD,
    );
    painter.text(
        egui::pos2(rect.max.x - 88.0, rect.min.y + 80.0),
        egui::Align2::LEFT_TOP,
        "THY KINGDOM\nCOME\n\nMAT 6:10",
        egui::FontId::proportional(11.0),
        GOLD,
    );

    ui.vertical_centered(|ui| {
        ui.add_space(36.0);
        ui.label(
            RichText::new("THE LIONS GATE")
                .size(36.0)
                .color(GOLD)
                .strong(),
        );
        ui.label(
            RichText::new("IGNITED BY THE SPIRIT. ROOTED IN THE WORD.")
                .small()
                .color(GOLD_DIM),
        );
        ui.add_space(18.0);

        let items: &[(&str, MenuAction, bool)] = &[
            ("✝  CONTINUE", MenuAction::Continue, has_save),
            ("🛡  NEW CAMPAIGN", MenuAction::NewCampaign, true),
            ("✝  CHURCH SERVICE", MenuAction::ChurchService, true),
            ("👥  MULTIPLAYER", MenuAction::Multiplayer, true),
            ("📖  BIBLE", MenuAction::Bible, true),
            ("⚙  OPTIONS", MenuAction::Options, true),
            ("🦁  CREDITS", MenuAction::Credits, true),
            ("🚪  EXIT GAME", MenuAction::Exit, true),
        ];

        for (label, act, enabled) in items {
            let fill = if *enabled {
                PANEL
            } else {
                Color32::from_rgb(30, 28, 26)
            };
            let text_color = if *enabled { GOLD } else { Color32::DARK_GRAY };
            let btn = egui::Button::new(RichText::new(*label).size(16.0).color(text_color))
                .fill(fill)
                .stroke(egui::Stroke::new(1.5, GOLD_DIM))
                .min_size(Vec2::new(320.0, 40.0));
            let resp = ui.add_enabled(*enabled, btn);
            if resp.clicked() {
                action = Some(*act);
            }
            ui.add_space(6.0);
        }
    });

    action
}

pub fn draw_character_select(
    ui: &mut egui::Ui,
    selected: &mut HeroClass,
    name: &mut String,
) -> Option<bool> {
    // None = stay, Some(true) = confirm, Some(false) = back
    let mut result = None;
    ui.vertical_centered(|ui| {
        ui.add_space(24.0);
        ui.label(RichText::new("CHOOSE YOUR HERO").size(28.0).color(GOLD).strong());
        ui.label(
            RichText::new("No mana — health, prayer cooldown, and the Word.")
                .small()
                .color(GOLD_DIM),
        );
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Name").color(GOLD));
            ui.add(
                egui::TextEdit::singleline(name)
                    .desired_width(220.0)
                    .hint_text("Hero name"),
            );
        });
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            for class in HeroClass::all() {
                let on = *selected == *class;
                ui.vertical(|ui| {
                    ui.set_width(200.0);
                    let frame = egui::Frame::NONE
                        .fill(if on {
                            Color32::from_rgb(40, 32, 16)
                        } else {
                            PANEL
                        })
                        .stroke(egui::Stroke::new(
                            if on { 2.0 } else { 1.0 },
                            if on { GOLD } else { GOLD_DIM },
                        ))
                        .inner_margin(12.0);
                    frame.show(ui, |ui| {
                        if ui
                            .add(
                                egui::Label::new(
                                    RichText::new(class.display_name())
                                        .size(18.0)
                                        .color(GOLD)
                                        .strong(),
                                )
                                .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            *selected = *class;
                        }
                        ui.label(RichText::new(class.blurb()).small().color(Color32::LIGHT_GRAY));
                        ui.label(
                            RichText::new(format!(
                                "HP {:.0} · DMG {:.0} · SPD {:.0}",
                                class.max_health(),
                                class.damage(),
                                class.move_speed()
                            ))
                            .small()
                            .color(GOLD_DIM),
                        );
                    });
                });
                ui.add_space(8.0);
            }
        });

        ui.add_space(20.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("← Back").color(GOLD))
                        .fill(PANEL)
                        .min_size(Vec2::new(120.0, 36.0)),
                )
                .clicked()
            {
                result = Some(false);
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("Enter Lion's Haven →").color(Color32::BLACK))
                        .fill(GOLD)
                        .min_size(Vec2::new(220.0, 36.0)),
                )
                .clicked()
            {
                if name.trim().is_empty() {
                    *name = format!("Hero {}", class_short(*selected));
                }
                result = Some(true);
            }
        });
    });
    result
}

fn class_short(c: HeroClass) -> &'static str {
    match c {
        HeroClass::Knight => "Knight",
        HeroClass::Pathfinder => "Pathfinder",
        HeroClass::Shepherd => "Shepherd",
    }
}

pub fn draw_simple_panel(ui: &mut egui::Ui, title: &str, body: &str) -> bool {
    let mut back = false;
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new(title).size(26.0).color(GOLD).strong());
        ui.add_space(12.0);
        ui.label(RichText::new(body).color(Color32::LIGHT_GRAY));
        ui.add_space(20.0);
        if ui
            .add(
                egui::Button::new(RichText::new("← Back to Menu").color(GOLD))
                    .fill(PANEL)
                    .min_size(Vec2::new(180.0, 36.0)),
            )
            .clicked()
        {
            back = true;
        }
    });
    back
}
