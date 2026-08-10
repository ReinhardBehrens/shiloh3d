//! Lions Gate app — menus + Diablo-style play loop on the shared world map.

use glam::Vec2;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use egui::{self, Color32, Pos2, RichText, Sense};

use crate::bible::BibleState;
use crate::character::{Hero, HeroClass};
use crate::hud::{self, HudFlags};
use crate::loot::{self, GroundLoot, Inventory, ItemKind};
use crate::menu::{self, MenuAction};
use crate::world::{WorldMap, Zone, ZoneId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    MainMenu,
    CharacterSelect,
    ChurchService,
    Multiplayer,
    BibleMenu,
    Options,
    Credits,
    Playing,
}

pub struct LionsGateApp {
    screen: Screen,
    hero: Option<Hero>,
    select_class: HeroClass,
    select_name: String,
    has_save: bool,
    zone: Zone,
    player_pos: Vec2,
    world: WorldMap,
    inventory: Inventory,
    ground_loot: Vec<GroundLoot>,
    bible: BibleState,
    hud: HudFlags,
    message: String,
    message_ttl: f32,
    attack_cd: f32,
    rng: StdRng,
    // combat flash
    hit_flash: f32,
}

impl LionsGateApp {
    pub fn new() -> Self {
        Self {
            screen: Screen::MainMenu,
            hero: None,
            select_class: HeroClass::Knight,
            select_name: "Daniel".into(),
            has_save: false,
            zone: Zone::build(ZoneId::Town),
            player_pos: Vec2::new(450.0, 520.0),
            world: WorldMap::default(),
            inventory: Inventory::new(24),
            ground_loot: Vec::new(),
            bible: BibleState::default(),
            hud: HudFlags::default(),
            message: String::new(),
            message_ttl: 0.0,
            attack_cd: 0.0,
            rng: StdRng::seed_from_u64(7),
            hit_flash: 0.0,
        }
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.message_ttl = 2.5;
    }

    fn start_campaign(&mut self) {
        let name = self.select_name.clone();
        let class = self.select_class;
        self.hero = Some(Hero::new(name, class));
        self.inventory = Inventory::new(24);
        self.ground_loot.clear();
        self.zone = Zone::build(ZoneId::Town);
        self.player_pos = self.zone.spawn;
        self.world = WorldMap::default();
        self.world.discover(ZoneId::Town);
        self.has_save = true;
        self.screen = Screen::Playing;
        self.bible.open = false;
        self.toast("Welcome to Lion's Haven. Take the north gate to the Forest.");
    }

    fn continue_campaign(&mut self) {
        if self.hero.is_none() {
            self.start_campaign();
            return;
        }
        self.screen = Screen::Playing;
    }

    fn travel(&mut self, to: ZoneId) {
        self.zone = Zone::build(to);
        self.player_pos = self.zone.spawn;
        self.ground_loot.clear();
        self.world.discover(to);
        self.toast(format!("Entered {}", to.display_name()));
    }

    fn tick_play(&mut self, ctx: &egui::Context, dt: f32) {
        if self.hero.as_ref().is_none_or(|h| !h.alive()) {
            if let Some(hero) = self.hero.as_mut() {
                hero.health = hero.max_health * 0.5;
            }
            self.toast("You fell — faith restores you in Town.");
            self.travel(ZoneId::Town);
            return;
        }

        if let Some(hero) = self.hero.as_mut() {
            hero.prayer_cd = (hero.prayer_cd - dt).max(0.0);
        }
        self.attack_cd = (self.attack_cd - dt).max(0.0);
        self.hit_flash = (self.hit_flash - dt).max(0.0);
        if self.message_ttl > 0.0 {
            self.message_ttl -= dt;
            if self.message_ttl <= 0.0 {
                self.message.clear();
            }
        }

        let move_speed = self
            .hero
            .as_ref()
            .map(|h| h.class.move_speed())
            .unwrap_or(180.0);

        let mut move_dir = Vec2::ZERO;
        let mut toggle_inv = false;
        let mut toggle_map = false;
        let mut toggle_bible = false;
        let mut to_menu = false;
        let mut pray = false;
        let mut attack = false;
        let mut loot_key = false;
        ctx.input(|i| {
            if i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp) {
                move_dir.y -= 1.0;
            }
            if i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown) {
                move_dir.y += 1.0;
            }
            if i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft) {
                move_dir.x -= 1.0;
            }
            if i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight) {
                move_dir.x += 1.0;
            }
            toggle_inv = i.key_pressed(egui::Key::I);
            toggle_map = i.key_pressed(egui::Key::M);
            toggle_bible = i.key_pressed(egui::Key::B);
            to_menu = i.key_pressed(egui::Key::Escape);
            pray = i.key_pressed(egui::Key::Space);
            attack = i.pointer.primary_clicked() || i.key_pressed(egui::Key::E);
            loot_key = i.key_pressed(egui::Key::F);
        });
        if toggle_inv {
            self.hud.inventory_open = !self.hud.inventory_open;
        }
        if toggle_map {
            self.hud.map_open = !self.hud.map_open;
        }
        if toggle_bible {
            self.bible.open = !self.bible.open;
        }
        if to_menu {
            self.screen = Screen::MainMenu;
            return;
        }

        if move_dir.length_squared() > 0.0 {
            let dir = move_dir.normalize();
            self.player_pos += dir * move_speed * dt;
            self.player_pos = self
                .player_pos
                .clamp(Vec2::splat(24.0), self.zone.size - Vec2::splat(24.0));
        }

        if pray {
            if let Some(hero) = self.hero.as_mut() {
                if hero.prayer_cd <= 0.0 {
                    let heal = match hero.class {
                        HeroClass::Shepherd => 35.0,
                        HeroClass::Knight => 22.0,
                        HeroClass::Pathfinder => 18.0,
                    };
                    hero.health = (hero.health + heal).min(hero.max_health);
                    hero.prayer_cd = hero.class.prayer_cooldown();
                    self.toast(format!("Prayer restores +{heal:.0} HP"));
                }
            }
        }

        let mut travel_to: Option<ZoneId> = None;
        let mut toast_msg: Option<String> = None;

        if attack && self.attack_cd <= 0.0 {
            if let Some(idx) = nearest_enemy(&self.zone.enemies, self.player_pos, 70.0) {
                let power = equipped_power(&self.inventory) as f32;
                let dmg = self
                    .hero
                    .as_ref()
                    .map(|h| h.class.damage() + power)
                    .unwrap_or(10.0);
                let enemy = &mut self.zone.enemies[idx];
                enemy.health -= dmg;
                self.attack_cd = 0.35;
                self.hit_flash = 0.15;
                if enemy.health <= 0.0 {
                    let xp = enemy.xp;
                    let name = enemy.name;
                    let pos = enemy.pos;
                    let swamp = self.zone.id == ZoneId::Swamp;
                    self.zone.enemies.remove(idx);
                    if let Some(hero) = self.hero.as_mut() {
                        hero.add_xp(xp);
                    }
                    let drop = loot::roll_drop(&mut self.rng, swamp);
                    if matches!(drop.kind, ItemKind::GoldPile) {
                        if let Some(hero) = self.hero.as_mut() {
                            hero.gold += drop.gold_value;
                        }
                        toast_msg = Some(format!(
                            "Defeated {name} · +{xp} XP · {}g",
                            drop.gold_value
                        ));
                    } else {
                        toast_msg = Some(format!("Defeated {name} · +{xp} XP · loot dropped"));
                        self.ground_loot.push(GroundLoot {
                            item: drop,
                            pos: pos + Vec2::new(self.rng.gen_range(-12.0..12.0), 8.0),
                        });
                    }
                }
            }
        }

        if loot_key {
            if let Some(i) = nearest_loot(&self.ground_loot, self.player_pos, 48.0) {
                let ground = self.ground_loot.remove(i);
                match self.inventory.try_add(ground.item.clone()) {
                    Ok(()) => toast_msg = Some(format!("Looted {}", ground.item.name)),
                    Err(item) => {
                        if let Some(hero) = self.hero.as_mut() {
                            if matches!(item.kind, ItemKind::GoldPile) {
                                hero.gold += item.gold_value;
                                toast_msg = Some(format!("+{}g", item.gold_value));
                            } else {
                                hero.gold += item.gold_value / 2;
                                toast_msg = Some("Pack full — sold for half gold".into());
                            }
                        }
                    }
                }
            } else {
                for p in &self.zone.portals {
                    if self.player_pos.distance(p.pos) < 40.0 {
                        travel_to = Some(p.to);
                        break;
                    }
                }
            }
        }

        for e in &mut self.zone.enemies {
            let to_player = self.player_pos - e.pos;
            let dist = to_player.length();
            if dist < 220.0 && dist > 1.0 {
                e.pos += to_player.normalize() * 70.0 * dt;
            }
            if dist < 28.0 {
                if let Some(hero) = self.hero.as_mut() {
                    hero.health -= e.damage * dt * 0.55;
                }
            }
        }

        if let Some(msg) = toast_msg {
            self.toast(msg);
        }
        if let Some(to) = travel_to {
            self.travel(to);
        }
    }

    fn draw_world(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
        let rect = resp.rect;
        let g = self.zone.id.ground_color();
        painter.rect_filled(rect, 0.0, Color32::from_rgb(g[0], g[1], g[2]));

        // Simple “bigger world” parallax tint
        match self.zone.id {
            ZoneId::Town => {
                painter.circle_filled(rect.center() + egui::vec2(0.0, -40.0), 90.0, Color32::from_rgb(90, 80, 60));
            }
            ZoneId::Forest => {
                for i in 0..18 {
                    let x = rect.min.x + 40.0 + (i as f32 * 55.0) % rect.width();
                    let y = rect.min.y + 60.0 + ((i * 37) % 200) as f32;
                    painter.circle_filled(Pos2::new(x, y), 18.0, Color32::from_rgb(20, 55, 28));
                }
            }
            ZoneId::Swamp => {
                painter.rect_filled(
                    egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width() * 0.5, 80.0)),
                    20.0,
                    Color32::from_rgb(30, 50, 45),
                );
            }
        }

        let scale_x = rect.width() / self.zone.size.x;
        let scale_y = rect.height() / self.zone.size.y;
        let to_screen = |p: Vec2| {
            Pos2::new(rect.min.x + p.x * scale_x, rect.min.y + p.y * scale_y)
        };

        for p in &self.zone.portals {
            let c = to_screen(p.pos);
            painter.rect_filled(
                egui::Rect::from_center_size(c, egui::vec2(48.0, 28.0)),
                4.0,
                Color32::from_rgb(180, 150, 60),
            );
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                p.label,
                egui::FontId::proportional(11.0),
                Color32::BLACK,
            );
        }

        for n in &self.zone.npcs {
            let c = to_screen(n.pos);
            painter.circle_filled(c, 12.0, Color32::from_rgb(100, 160, 220));
            painter.text(
                c + egui::vec2(0.0, -18.0),
                egui::Align2::CENTER_CENTER,
                n.name,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
            if self.player_pos.distance(n.pos) < 50.0 {
                self.message = n.line.into();
                self.message_ttl = 0.5;
            }
        }

        for e in &self.zone.enemies {
            let c = to_screen(e.pos);
            painter.circle_filled(c, 14.0, Color32::from_rgb(140, 40, 40));
            let frac = (e.health / e.max_health).clamp(0.0, 1.0);
            painter.rect_filled(
                egui::Rect::from_min_size(c + egui::vec2(-16.0, -24.0), egui::vec2(32.0, 4.0)),
                1.0,
                Color32::DARK_GRAY,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(c + egui::vec2(-16.0, -24.0), egui::vec2(32.0 * frac, 4.0)),
                1.0,
                Color32::RED,
            );
        }

        for loot in &self.ground_loot {
            let c = to_screen(loot.pos);
            painter.circle_filled(c, 7.0, loot.item.rarity.color());
        }

        let pc = to_screen(self.player_pos);
        let hero_color = if self.hit_flash > 0.0 {
            Color32::WHITE
        } else {
            Color32::from_rgb(220, 190, 90)
        };
        painter.circle_filled(pc, 16.0, hero_color);
        painter.circle_stroke(pc, 16.0, egui::Stroke::new(2.0, Color32::from_rgb(80, 50, 10)));
        if let Some(h) = &self.hero {
            painter.text(
                pc + egui::vec2(0.0, -26.0),
                egui::Align2::CENTER_CENTER,
                &h.name,
                egui::FontId::proportional(12.0),
                Color32::from_rgb(255, 230, 160),
            );
        }
    }
}

fn nearest_enemy(enemies: &[crate::world::Enemy], pos: Vec2, max_d: f32) -> Option<usize> {
    enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| e.pos.distance(pos) <= max_d)
        .min_by(|(_, a), (_, b)| {
            a.pos
                .distance(pos)
                .partial_cmp(&b.pos.distance(pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

fn nearest_loot(loot: &[GroundLoot], pos: Vec2, max_d: f32) -> Option<usize> {
    loot.iter()
        .enumerate()
        .filter(|(_, l)| l.pos.distance(pos) <= max_d)
        .min_by(|(_, a), (_, b)| {
            a.pos
                .distance(pos)
                .partial_cmp(&b.pos.distance(pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

fn equipped_power(inv: &Inventory) -> i32 {
    inv.slots
        .iter()
        .flatten()
        .map(|i| i.power)
        .sum::<i32>()
        .min(40)
}

impl eframe::App for LionsGateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dark gold theme
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = Color32::from_rgb(16, 14, 12);
        visuals.panel_fill = Color32::from_rgb(20, 18, 14);
        ctx.set_visuals(visuals);

        let dt = ctx.input(|i| i.stable_dt).min(0.05);

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.screen {
                Screen::MainMenu => {
                    if let Some(act) = menu::draw_main_menu(ui, self.has_save || self.hero.is_some())
                    {
                        match act {
                            MenuAction::Continue => self.continue_campaign(),
                            MenuAction::NewCampaign => self.screen = Screen::CharacterSelect,
                            MenuAction::ChurchService => self.screen = Screen::ChurchService,
                            MenuAction::Multiplayer => self.screen = Screen::Multiplayer,
                            MenuAction::Bible => {
                                self.bible.open = true;
                                self.screen = Screen::BibleMenu;
                            }
                            MenuAction::Options => self.screen = Screen::Options,
                            MenuAction::Credits => self.screen = Screen::Credits,
                            MenuAction::Exit => {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    }
                }
                Screen::CharacterSelect => {
                    if let Some(ok) = menu::draw_character_select(
                        ui,
                        &mut self.select_class,
                        &mut self.select_name,
                    ) {
                        if ok {
                            self.start_campaign();
                        } else {
                            self.screen = Screen::MainMenu;
                        }
                    }
                }
                Screen::ChurchService => {
                    if menu::draw_simple_panel(
                        ui,
                        "CHURCH SERVICE",
                        "Gather in Lion's Haven chapel.\nWorship stub — full liturgy & co-op service comes with network slice.\n\n\"Where two or three are gathered…\"",
                    ) {
                        self.screen = Screen::MainMenu;
                    }
                }
                Screen::Multiplayer => {
                    if menu::draw_simple_panel(
                        ui,
                        "MULTIPLAYER",
                        "Host / Join campaign co-op (Shiloh net layer).\nStub lobby — same Town→Forest→Swamp map, shared loot rules.\n\nComing online with shiloh-network replication.",
                    ) {
                        self.screen = Screen::MainMenu;
                    }
                }
                Screen::BibleMenu => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new("BIBLE")
                                .size(28.0)
                                .color(Color32::from_rgb(212, 175, 55))
                                .strong(),
                        );
                        let v = self.bible.current();
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(v.ref_)
                                .size(18.0)
                                .color(Color32::from_rgb(212, 175, 55)),
                        );
                        ui.label(RichText::new(v.text).size(16.0));
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            if ui.button("← Prev").clicked() {
                                self.bible.prev();
                            }
                            if ui.button("Next →").clicked() {
                                self.bible.next();
                            }
                        });
                        if ui.button("← Back to Menu").clicked() {
                            self.bible.open = false;
                            self.screen = Screen::MainMenu;
                        }
                    });
                }
                Screen::Options => {
                    if menu::draw_simple_panel(
                        ui,
                        "OPTIONS",
                        "Master volume · fullscreen · accessibility.\n(Placeholder — wires to Shiloh settings later.)",
                    ) {
                        self.screen = Screen::MainMenu;
                    }
                }
                Screen::Credits => {
                    if menu::draw_simple_panel(
                        ui,
                        "CREDITS",
                        "The Lions Gate — built on Shiloh3D\nChristian-owned · bundleable engine\nPowered by Poly Haven (CC0) for outdoor props\n\nIgnited by the Spirit. Rooted in the Word.",
                    ) {
                        self.screen = Screen::MainMenu;
                    }
                }
                Screen::Playing => {
                    self.tick_play(ctx, dt);
                    self.draw_world(ui);
                    if let Some(hero) = &self.hero {
                        hud::draw_play_hud(
                            ui,
                            hero,
                            self.zone.id,
                            &self.inventory,
                            &self.bible,
                            &self.message,
                            &mut self.hud,
                        );
                    }
                }
            }
        });

        ctx.request_repaint();
    }
}
