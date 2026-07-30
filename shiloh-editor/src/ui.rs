//! Premium docked editor UI — mockup-faithful shell.
//!
//! Layout: menu · outliner + filesystem · viewport + assets/console ·
//! inspector / node graph · status. Live features: play snapshot, world-item
//! spawn, URL import, interactive node graph.

use std::path::PathBuf;
use std::time::Instant;

use ahash::AHashMap;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use shiloh_ecs::Entity;
use shiloh_scene::{
    Camera, Scene, SceneFile, Transform, propagate_transforms, save_scene, set_parent,
};

use crate::import::import_from_url;
use crate::node_graph::NodeGraph;
use crate::play_mode::PlaySession;
use crate::project::Project;
use crate::selection::Selection;
use crate::world_items::{
    WorldItem, WorldItemCategory, builtin_world_items, ensure_project_layout,
    scan_project_assets,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RightTab {
    Inspector,
    Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Assets,
    Console,
}

#[derive(Debug, Clone)]
struct ConsoleLine {
    level: &'static str,
    message: String,
    color: Color32,
}

/// Top-level egui application: owns the in-memory scene being edited.
pub struct EditorApp {
    pub project: Option<Project>,
    pub scene: Scene,
    pub selection: Selection,
    pub play: PlaySession,
    pub camera: Option<Camera>,
    pub status: String,
    entity_names: AHashMap<Entity, String>,
    scene_path: Option<PathBuf>,
    hidden: AHashMap<Entity, bool>,
    locked: AHashMap<Entity, bool>,
    right_tab: RightTab,
    bottom_tab: BottomTab,
    node_graph: NodeGraph,
    world_items: Vec<WorldItem>,
    category_filter: Option<WorldItemCategory>,
    import_url: String,
    console: Vec<ConsoleLine>,
    frame_start: Instant,
    fps_smooth: f32,
    /// Stylized light props for inspector demo (DirectionalLight fields).
    light_intensity: f32,
    light_temperature: f32,
    light_color: [f32; 3],
    fog_enabled: bool,
}

impl EditorApp {
    pub fn new(project: Option<Project>) -> Self {
        if let Some(ref p) = project {
            let _ = ensure_project_layout(&p.root);
        }

        let scene_path = project
            .as_ref()
            .map(|p| p.root.join(&p.manifest.default_scene));

        let mut app = Self {
            project,
            scene: Scene::new("Forest_Valley"),
            selection: Selection::default(),
            play: PlaySession::default(),
            camera: Some(Camera::default()),
            status: "Ready".into(),
            entity_names: AHashMap::default(),
            scene_path,
            hidden: AHashMap::default(),
            locked: AHashMap::default(),
            right_tab: RightTab::Inspector,
            bottom_tab: BottomTab::Assets,
            node_graph: NodeGraph::new_demo(),
            world_items: builtin_world_items(),
            category_filter: None,
            import_url: String::new(),
            console: Vec::new(),
            frame_start: Instant::now(),
            fps_smooth: 60.0,
            light_intensity: 3.2,
            light_temperature: 5500.0,
            light_color: [1.0, 0.96, 0.90],
            fog_enabled: true,
        };

        app.log("INFO", "Shiloh3D Editor started", Color32::from_rgb(220, 225, 235));
        app.log(
            "INFO",
            "RHI: wgpu bootstrap · Vulkan preferred on desktop",
            Color32::from_rgb(220, 225, 235),
        );
        app.log(
            "DEBUG",
            "Shader cache warm · slice PBR / shadow / water v1",
            Color32::from_rgb(90, 200, 120),
        );

        app.seed_demo_hierarchy();

        if let Some(path) = app.scene_path.clone() {
            if path.exists() {
                app.load_scene_from(&path);
            } else {
                app.status = format!("New project — seed scene (no file at {})", path.display());
                app.log(
                    "WARNING",
                    &format!("No scene file yet at {}", path.display()),
                    Color32::from_rgb(230, 160, 60),
                );
            }
        }
        app
    }

    fn log(&mut self, level: &'static str, message: &str, color: Color32) {
        self.console.push(ConsoleLine {
            level,
            message: message.to_string(),
            color,
        });
        if self.console.len() > 200 {
            self.console.drain(0..self.console.len() - 200);
        }
    }

    fn seed_demo_hierarchy(&mut self) {
        if self.scene.world.entity_count() > 0 {
            return;
        }
        let seeds = [
            ("DirectionalLight", [0.0, 12.0, 0.0]),
            ("SkyAtmosphere", [0.0, 0.0, 0.0]),
            ("FogVolume", [0.0, 2.0, 0.0]),
            ("Terrain_Heightmap", [0.0, 0.0, 0.0]),
            ("WaterBody", [0.0, 0.2, 0.0]),
            ("Pine_Tall", [-4.0, 0.0, 3.0]),
            ("Birch", [2.5, 0.0, -1.5]),
            ("Rock_Large", [0.5, 0.0, 1.0]),
            ("Cliff_Face", [-8.0, 0.0, -4.0]),
        ];
        for (name, pos) in seeds {
            let e = self.scene.spawn_transform(Transform {
                translation: glam::Vec3::from_array(pos),
                ..Transform::default()
            });
            self.entity_names.insert(e, name.into());
        }
        propagate_transforms(&mut self.scene.world);
        self.log(
            "INFO",
            "Seeded Forest_Valley outliner with basic world items",
            Color32::from_rgb(220, 225, 235),
        );
    }

    fn hierarchy_label(&self, index: usize, entity: Entity) -> String {
        self.entity_names
            .get(&entity)
            .cloned()
            .unwrap_or_else(|| format!("Entity {index}"))
    }

    fn new_entity_named(&mut self, name: &str, translation: glam::Vec3) {
        let entity = self.scene.spawn_transform(Transform {
            translation,
            ..Transform::default()
        });
        propagate_transforms(&mut self.scene.world);
        self.entity_names.insert(entity, name.to_string());
        self.selection.clear();
        self.selection.select(entity);
        self.status = format!("Spawned {name}");
        self.log(
            "INFO",
            &format!("Spawned {name}"),
            Color32::from_rgb(220, 225, 235),
        );
    }

    fn new_entity(&mut self) {
        self.new_entity_named("Entity", glam::Vec3::ZERO);
    }

    fn spawn_world_item(&mut self, item: &WorldItem) {
        let offset = (self.scene.world.entity_count() as f32) * 0.35;
        self.new_entity_named(
            item.spawn_name,
            glam::Vec3::new(offset % 5.0, 0.0, offset * 0.2),
        );
    }

    fn default_save_path(&self) -> Option<PathBuf> {
        self.scene_path.clone().or_else(|| {
            self.project
                .as_ref()
                .map(|p| p.root.join(&p.manifest.default_scene))
        })
    }

    fn save_scene(&mut self) {
        let Some(path) = self.default_save_path() else {
            self.status = "Cannot save: no project/scene path".into();
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match save_scene(&path, &self.scene, self.camera.as_ref()) {
            Ok(()) => {
                self.scene_path = Some(path.clone());
                self.status = format!("Saved {}", path.display());
                self.log(
                    "INFO",
                    &format!("Scene saved · {}", path.display()),
                    Color32::from_rgb(220, 225, 235),
                );
            }
            Err(err) => {
                let msg = format!("Save failed: {err}");
                self.status = msg.clone();
                self.log("WARNING", &msg, Color32::from_rgb(230, 160, 60));
            }
        }
    }

    fn load_scene_from(&mut self, path: &std::path::Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(err) => {
                self.status = format!("Load failed: {err}");
                return;
            }
        };
        let file = match SceneFile::from_json(&text) {
            Ok(f) => f,
            Err(err) => {
                self.status = format!("Load failed: {err}");
                return;
            }
        };

        let mut scene = Scene::new(file.name.clone());
        let spawned: Vec<Entity> = file
            .entities
            .iter()
            .map(|record| {
                scene.spawn_transform(Transform {
                    translation: glam::Vec3::from_array(record.translation),
                    rotation: glam::Quat::from_array(record.rotation),
                    scale: glam::Vec3::from_array(record.scale),
                    dirty: true,
                })
            })
            .collect();
        for (record, &child) in file.entities.iter().zip(&spawned) {
            if let Some(parent_index) = record.parent
                && let Some(&parent) = spawned.get(parent_index)
            {
                set_parent(&mut scene.world, child, parent);
            }
        }
        propagate_transforms(&mut scene.world);

        self.entity_names = file
            .entities
            .iter()
            .zip(&spawned)
            .map(|(record, &e)| (e, record.name.clone()))
            .collect();
        self.camera = file.camera.as_ref().map(|c| c.to_camera()).or(self.camera);
        self.scene = scene;
        self.scene_path = Some(path.to_path_buf());
        self.selection.clear();
        self.status = format!("Loaded {}", path.display());
        self.log(
            "INFO",
            &format!("Loaded scene {}", path.display()),
            Color32::from_rgb(220, 225, 235),
        );
    }

    fn toggle_play(&mut self) {
        if self.play.is_playing() {
            let restored_camera = self.play.exit_play(&mut self.scene);
            if restored_camera.is_some() {
                self.camera = restored_camera;
            }
            propagate_transforms(&mut self.scene.world);
            self.status = "Stopped — edit snapshot restored".into();
            self.log(
                "INFO",
                "Play mode exited · snapshot restored",
                Color32::from_rgb(220, 225, 235),
            );
        } else {
            self.play.enter_play(&self.scene, self.camera.as_ref());
            self.status = "Playing (Stop restores edit snapshot)".into();
            self.log(
                "INFO",
                "Play mode entered · live simulation",
                Color32::from_rgb(90, 200, 120),
            );
        }
    }

    fn do_url_import(&mut self) {
        let Some(project) = self.project.as_ref() else {
            self.status = "Open a project before importing".into();
            return;
        };
        let url = self.import_url.clone();
        if url.trim().is_empty() {
            self.status = "Paste an http(s) URL to import".into();
            return;
        }
        match import_from_url(&project.root, &url) {
            Ok(path) => {
                self.status = format!("Imported {}", path.display());
                self.log(
                    "INFO",
                    &format!("Imported from URL → {}", path.display()),
                    Color32::from_rgb(90, 200, 120),
                );
                self.import_url.clear();
                self.bottom_tab = BottomTab::Assets;
            }
            Err(err) => {
                let msg = format!("Import failed: {err}");
                self.status = msg.clone();
                self.log("WARNING", &msg, Color32::from_rgb(230, 160, 60));
            }
        }
    }

    fn apply_theme(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = Color32::from_rgb(22, 24, 30);
        visuals.panel_fill = Color32::from_rgb(26, 28, 36);
        visuals.extreme_bg_color = Color32::from_rgb(16, 18, 24);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 34, 44);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 40, 52);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(44, 52, 70);
        visuals.widgets.active.bg_fill = Color32::from_rgb(50, 90, 160);
        visuals.selection.bg_fill = Color32::from_rgb(50, 110, 200);
        visuals.widgets.noninteractive.fg_stroke =
            Stroke::new(1.0_f32, Color32::from_rgb(190, 196, 210));
        visuals.widgets.inactive.fg_stroke =
            Stroke::new(1.0_f32, Color32::from_rgb(210, 214, 225));
        visuals.hyperlink_color = Color32::from_rgb(90, 160, 255);
        ctx.set_visuals(visuals);
    }

    // ── Panels ──────────────────────────────────────────────────────────

    fn ui_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Save Scene").clicked() {
                    self.save_scene();
                    ui.close_menu();
                }
                if ui.button("Load Scene").clicked() {
                    if let Some(path) = self.default_save_path() {
                        self.load_scene_from(&path);
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Import from URL…").clicked() {
                    self.bottom_tab = BottomTab::Assets;
                    ui.close_menu();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("New Entity").clicked() {
                    self.new_entity();
                    ui.close_menu();
                }
            });
            ui.menu_button("Build", |ui| {
                ui.label("Cook / package — coming soon");
            });
            ui.menu_button("Window", |ui| {
                if ui
                    .selectable_label(self.right_tab == RightTab::Inspector, "Inspector")
                    .clicked()
                {
                    self.right_tab = RightTab::Inspector;
                    ui.close_menu();
                }
                if ui
                    .selectable_label(self.right_tab == RightTab::Node, "Node Graph")
                    .clicked()
                {
                    self.right_tab = RightTab::Node;
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                ui.label("Shiloh3D Editor — premium clear shell");
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let name = self
                    .project
                    .as_ref()
                    .map(|p| p.manifest.name.as_str())
                    .unwrap_or("Untitled");
                ui.label(RichText::new(name).weak());
                ui.label(RichText::new("Shiloh3D Studio").strong().color(Color32::from_rgb(
                    90, 160, 255,
                )));
            });
        });
    }

    fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let editing = !self.play.is_playing();
            ui.add_enabled_ui(editing, |ui| {
                if ui.button("＋ Entity").clicked() {
                    self.new_entity();
                }
                if ui.button("Save").clicked() {
                    self.save_scene();
                }
            });

            ui.separator();

            let play_label = if self.play.is_playing() {
                "⏹ Stop"
            } else {
                "▶ Play"
            };
            let play_btn = egui::Button::new(RichText::new(play_label).strong()).fill(
                if self.play.is_playing() {
                    Color32::from_rgb(160, 60, 60)
                } else {
                    Color32::from_rgb(40, 110, 70)
                },
            );
            if ui.add(play_btn).clicked() {
                self.toggle_play();
            }

            ui.separator();
            ui.label(RichText::new(format!("{:.0} FPS", self.fps_smooth)).monospace());
            ui.separator();
            ui.label(if self.play.is_playing() {
                RichText::new("PLAY").color(Color32::from_rgb(90, 220, 120)).strong()
            } else {
                RichText::new("EDIT").color(Color32::from_rgb(160, 170, 190))
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("Forest_Valley.scene").weak());
                let _ = ui.selectable_label(true, "Viewport");
                let _ = ui.selectable_label(false, "Lighting");
            });
        });
    }

    fn ui_outliner(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Scene Outliner");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("Forest_Valley").weak().small());
            });
        });
        ui.separator();

        egui::ScrollArea::vertical().id_salt("outliner").show(ui, |ui| {
            let mut entities = Vec::new();
            self.scene.world.for_each::<Transform>(|e, _| entities.push(e));

            let groups: [(&str, &[&str]); 3] = [
                ("Environment", &["DirectionalLight", "SkyAtmosphere", "FogVolume", "WaterBody"]),
                ("Terrain", &["Terrain_Heightmap", "Cliff_Face"]),
                (
                    "WorldObjects",
                    &[
                        "Pine_Tall",
                        "Pine_Cluster",
                        "Birch",
                        "Dead_Tree",
                        "Grass_Patch",
                        "Rock_Large",
                        "Rock_Scatter",
                    ],
                ),
            ];

            let mut clicked: Option<Entity> = None;
            let mut toggle_vis: Option<Entity> = None;
            let mut toggle_lock: Option<Entity> = None;

            for (group, prefixes) in groups {
                egui::CollapsingHeader::new(RichText::new(group).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for (index, entity) in entities.iter().enumerate() {
                            let label = self.hierarchy_label(index, *entity);
                            let matches = prefixes.iter().any(|p| label.starts_with(p))
                                || (group == "WorldObjects"
                                    && !groups[0].1.iter().any(|p| label.starts_with(p))
                                    && !groups[1].1.iter().any(|p| label.starts_with(p)));
                            if !matches {
                                continue;
                            }
                            let selected = self.selection.entities.contains(entity);
                            let hidden = *self.hidden.get(entity).unwrap_or(&false);
                            let locked = *self.locked.get(entity).unwrap_or(&false);

                            ui.horizontal(|ui| {
                                let eye = if hidden { "◌" } else { "◉" };
                                if ui
                                    .add(egui::Button::new(eye).frame(false))
                                    .on_hover_text("Visibility")
                                    .clicked()
                                {
                                    toggle_vis = Some(*entity);
                                }
                                let lock = if locked { "🔒" } else { "🔓" };
                                if ui
                                    .add(egui::Button::new(lock).frame(false))
                                    .on_hover_text("Lock")
                                    .clicked()
                                {
                                    toggle_lock = Some(*entity);
                                }
                                let text = if hidden {
                                    RichText::new(&label).weak()
                                } else {
                                    RichText::new(&label)
                                };
                                if ui.selectable_label(selected, text).clicked() {
                                    clicked = Some(*entity);
                                }
                            });
                        }
                    });
            }

            // Orphans not matching prefixes
            egui::CollapsingHeader::new("Other")
                .default_open(true)
                .show(ui, |ui| {
                    for (index, entity) in entities.iter().enumerate() {
                        let label = self.hierarchy_label(index, *entity);
                        let known = groups.iter().any(|(_, prefs)| {
                            prefs.iter().any(|p| label.starts_with(p))
                        });
                        // WorldObjects catch-all already absorbs unknowns in groups loop —
                        // show only truly unlabeled Entity N here if they don't match foliage prefixes
                        let foliageish = [
                            "Pine", "Birch", "Dead", "Grass", "Rock", "Entity",
                        ]
                        .iter()
                        .any(|p| label.starts_with(p));
                        if known || foliageish {
                            continue;
                        }
                        let selected = self.selection.entities.contains(entity);
                        if ui.selectable_label(selected, &label).clicked() {
                            clicked = Some(*entity);
                        }
                    }
                });

            if let Some(entity) = clicked {
                self.selection.clear();
                self.selection.select(entity);
                self.right_tab = RightTab::Inspector;
            }
            if let Some(entity) = toggle_vis {
                let v = self.hidden.entry(entity).or_insert(false);
                *v = !*v;
            }
            if let Some(entity) = toggle_lock {
                let v = self.locked.entry(entity).or_insert(false);
                *v = !*v;
            }
        });
    }

    fn ui_filesystem(&mut self, ui: &mut egui::Ui) {
        ui.strong("File System");
        ui.separator();
        egui::ScrollArea::vertical().id_salt("fs").show(ui, |ui| {
            let root = self.project.as_ref().map(|p| p.root.clone());
            if let Some(root) = root {
                ui.label(RichText::new(root.display().to_string()).small().weak());
                for (folder, children) in [
                    ("Assets/", &["Environment/", "Foliage/", "Materials/", "Meshes/", "Textures/", "Imported/"][..]),
                    ("Scenes/", &["main.json"][..]),
                    ("Scripts/", &["graphs/"][..]),
                ] {
                    egui::CollapsingHeader::new(folder)
                        .default_open(folder.starts_with("Assets"))
                        .show(ui, |ui| {
                            for child in children {
                                ui.label(format!("  {child}"));
                            }
                        });
                }
            } else {
                ui.label("No project open");
            }
        });
    }

    fn ui_viewport(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (rect, _resp) = ui.allocate_exact_size(available, egui::Sense::hover());
        let painter = ui.painter_at(rect);

        // Stylized valley placeholder until wgpu embed ships.
        painter.rect_filled(rect, 0.0, Color32::from_rgb(28, 36, 48));
        let mid = rect.center();
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.left(), mid.y + 20.0),
                egui::pos2(rect.right(), rect.bottom()),
            ),
            0.0,
            Color32::from_rgb(34, 52, 42),
        );
        // Water band
        painter.rect_filled(
            egui::Rect::from_center_size(egui::pos2(mid.x, mid.y + 40.0), Vec2::new(rect.width() * 0.55, 28.0)),
            4.0,
            Color32::from_rgb(40, 90, 110),
        );
        // Mountains
        let peaks = [
            (0.15, 0.55),
            (0.35, 0.35),
            (0.55, 0.48),
            (0.75, 0.30),
            (0.90, 0.50),
        ];
        for (i, (fx, fy)) in peaks.iter().enumerate() {
            let base_y = mid.y + 30.0;
            let tip = egui::pos2(rect.left() + rect.width() * fx, rect.top() + rect.height() * fy);
            let left = egui::pos2(tip.x - 80.0 - i as f32 * 4.0, base_y);
            let right = egui::pos2(tip.x + 90.0, base_y);
            painter.add(egui::Shape::convex_polygon(
                vec![left, tip, right],
                Color32::from_rgb(48 + i as u8 * 8, 58, 70),
                Stroke::NONE,
            ));
        }
        // Gizmo at selection
        let gizmo = egui::pos2(mid.x + 20.0, mid.y + 10.0);
        painter.arrow(
            gizmo,
            Vec2::new(40.0, 0.0),
            Stroke::new(3.0_f32, Color32::from_rgb(220, 70, 70)),
        );
        painter.arrow(
            gizmo,
            Vec2::new(0.0, -40.0),
            Stroke::new(3.0_f32, Color32::from_rgb(70, 200, 90)),
        );
        painter.arrow(
            gizmo,
            Vec2::new(24.0, 24.0),
            Stroke::new(3.0_f32, Color32::from_rgb(70, 120, 230)),
        );
        painter.circle_filled(gizmo, 4.0, Color32::WHITE);

        // HUD overlay
        painter.text(
            rect.left_top() + Vec2::new(12.0, 10.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{:.0} FPS  ·  Perspective  ·  Lit  ·  Entities {}",
                self.fps_smooth,
                self.scene.world.entity_count()
            ),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(210, 220, 235),
        );
        painter.text(
            rect.left_bottom() + Vec2::new(12.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            "Viewport placeholder — SliceRenderer embeds next (wgpu)",
            egui::FontId::proportional(11.0),
            Color32::from_rgb(140, 150, 170),
        );
    }

    fn ui_assets(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Asset Browser");
            ui.separator();
            if ui
                .selectable_label(self.category_filter.is_none(), "All")
                .clicked()
            {
                self.category_filter = None;
            }
            for cat in [
                WorldItemCategory::Foliage,
                WorldItemCategory::Props,
                WorldItemCategory::Terrain,
                WorldItemCategory::Environment,
                WorldItemCategory::Lighting,
            ] {
                if ui
                    .selectable_label(self.category_filter == Some(cat), cat.label())
                    .clicked()
                {
                    self.category_filter = Some(cat);
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Import URL");
            ui.add(
                egui::TextEdit::singleline(&mut self.import_url)
                    .desired_width(320.0)
                    .hint_text("https://…/model.glb or texture.png"),
            );
            if ui.button("Download").clicked() {
                self.do_url_import();
            }
            if ui.button("Refresh").clicked() {
                self.status = "Asset list refreshed".into();
            }
        });

        ui.separator();
        ui.label(RichText::new("Basic world items — click to spawn").weak().small());

        egui::ScrollArea::horizontal()
            .id_salt("world_items")
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let items: Vec<_> = self
                        .world_items
                        .iter()
                        .filter(|i| {
                            self.category_filter
                                .map(|c| c == i.category)
                                .unwrap_or(true)
                        })
                        .cloned()
                        .collect();
                    for item in items {
                        let size = Vec2::new(96.0, 88.0);
                        let (rect, resp) =
                            ui.allocate_exact_size(size, egui::Sense::click());
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 6.0, Color32::from_rgb(34, 38, 50));
                        painter.rect_stroke(
                            rect,
                            6.0,
                            Stroke::new(1.0_f32, Color32::from_rgb(55, 62, 80)),
                            egui::StrokeKind::Outside,
                        );
                        // Thumbnail swatch by category
                        let swatch = match item.category {
                            WorldItemCategory::Foliage => Color32::from_rgb(50, 110, 60),
                            WorldItemCategory::Props => Color32::from_rgb(90, 85, 75),
                            WorldItemCategory::Terrain => Color32::from_rgb(100, 80, 55),
                            WorldItemCategory::Environment => Color32::from_rgb(50, 100, 130),
                            WorldItemCategory::Lighting => Color32::from_rgb(200, 180, 80),
                        };
                        let thumb = egui::Rect::from_center_size(
                            rect.center() - Vec2::new(0.0, 12.0),
                            Vec2::new(52.0, 40.0),
                        );
                        painter.rect_filled(thumb, 4.0, swatch);
                        painter.text(
                            rect.center_bottom() - Vec2::new(0.0, 8.0),
                            egui::Align2::CENTER_BOTTOM,
                            item.name,
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(210, 215, 225),
                        );
                        if resp.clicked() {
                            self.spawn_world_item(&item);
                        }
                        resp.on_hover_text(item.description);
                    }
                });
            });

        ui.add_space(6.0);
        ui.label(RichText::new("Project files").weak().small());
        if let Some(project) = self.project.as_ref() {
            let assets = scan_project_assets(&project.root);
            if assets.is_empty() {
                ui.label("No imported files yet — use Import URL above.");
            } else {
                egui::ScrollArea::vertical()
                    .max_height(80.0)
                    .id_salt("proj_assets")
                    .show(ui, |ui| {
                        for a in assets {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(a.kind.label()).weak().small());
                                ui.label(&a.name);
                            });
                        }
                    });
            }
        }
    }

    fn ui_console(&mut self, ui: &mut egui::Ui) {
        ui.strong("Console");
        ui.separator();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .id_salt("console")
            .show(ui, |ui| {
                for line in &self.console {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("[{}]", line.level))
                                .color(line.color)
                                .monospace()
                                .small(),
                        );
                        ui.label(RichText::new(&line.message).monospace().small());
                    });
                }
            });
    }

    fn ui_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(&selected) = self.selection.entities.first() else {
            ui.label("No entity selected.");
            ui.label(
                RichText::new("Select from the Scene Outliner or spawn a world item.")
                    .weak()
                    .small(),
            );
            return;
        };

        let name = self
            .entity_names
            .get(&selected)
            .cloned()
            .unwrap_or_else(|| format!("Entity {}", selected.index()));
        ui.heading(&name);
        ui.label(RichText::new(format!("Entity {}", selected.index())).weak().small());
        ui.separator();

        let is_light = name.contains("Light");
        let is_sky = name.contains("Sky") || name.contains("Fog");

        egui::CollapsingHeader::new("Transform")
            .default_open(true)
            .show(ui, |ui| {
                let Some(transform) = self.scene.world.get_mut::<Transform>(selected) else {
                    ui.label("No Transform");
                    return;
                };
                let mut changed = false;
                ui.label("Position");
                ui.horizontal(|ui| {
                    for (axis, v) in [
                        ("X", &mut transform.translation.x),
                        ("Y", &mut transform.translation.y),
                        ("Z", &mut transform.translation.z),
                    ] {
                        changed |= ui
                            .add(
                                egui::DragValue::new(v)
                                    .speed(0.05)
                                    .prefix(format!("{axis} ")),
                            )
                            .changed();
                    }
                });
                ui.label("Scale");
                ui.horizontal(|ui| {
                    for (axis, v) in [
                        ("X", &mut transform.scale.x),
                        ("Y", &mut transform.scale.y),
                        ("Z", &mut transform.scale.z),
                    ] {
                        changed |= ui
                            .add(
                                egui::DragValue::new(v)
                                    .speed(0.01)
                                    .prefix(format!("{axis} ")),
                            )
                            .changed();
                    }
                });
                if changed {
                    transform.mark_dirty();
                    propagate_transforms(&mut self.scene.world);
                }
            });

        if is_light {
            egui::CollapsingHeader::new("Light")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Type");
                        egui::ComboBox::from_id_salt("light_type")
                            .selected_text("Directional")
                            .show_ui(ui, |ui| {
                                let _ = ui.selectable_label(true, "Directional");
                                let _ = ui.selectable_label(false, "Point");
                                let _ = ui.selectable_label(false, "Spot");
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Color");
                        ui.color_edit_button_rgb(&mut self.light_color);
                    });
                    ui.add(
                        egui::Slider::new(&mut self.light_intensity, 0.0..=20.0).text("Intensity"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.light_temperature, 1000.0..=12000.0)
                            .text("Temperature"),
                    );
                });
        }

        if is_sky || is_light {
            egui::CollapsingHeader::new("Sky & Atmosphere")
                .default_open(is_sky)
                .show(ui, |ui| {
                    ui.checkbox(&mut self.fog_enabled, "Enable fog");
                    ui.label(RichText::new("Atmosphere hooks into slice fog/water track").weak().small());
                });
        }
    }

    fn ui_right_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.right_tab == RightTab::Inspector, "Inspector")
                .clicked()
            {
                self.right_tab = RightTab::Inspector;
            }
            if ui
                .selectable_label(self.right_tab == RightTab::Node, "Node")
                .clicked()
            {
                self.right_tab = RightTab::Node;
            }
        });
        ui.separator();
        match self.right_tab {
            RightTab::Inspector => self.ui_inspector(ui),
            RightTab::Node => {
                ui.horizontal(|ui| {
                    if ui.button("Export Visual Graph").clicked() {
                        if let Some(project) = self.project.as_ref() {
                            let path = project.root.join("scripts/graphs/main.vgraph.json");
                            let graph = self.node_graph.to_visual_graph("Main");
                            match graph.save(&path) {
                                Ok(()) => {
                                    self.status = format!("Exported {}", path.display());
                                    self.log(
                                        "INFO",
                                        &format!("Visual graph → {}", path.display()),
                                        Color32::from_rgb(90, 200, 120),
                                    );
                                }
                                Err(err) => {
                                    let msg = format!("Export failed: {err}");
                                    self.status = msg.clone();
                                    self.log("WARNING", &msg, Color32::from_rgb(230, 160, 60));
                                }
                            }
                        }
                    }
                });
                self.node_graph.ui(ui);
            }
        }
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        Self::apply_theme(ctx);

        let dt = self.frame_start.elapsed().as_secs_f32().max(1e-4);
        self.frame_start = Instant::now();
        let fps = 1.0 / dt;
        self.fps_smooth = self.fps_smooth * 0.9 + fps * 0.1;

        egui::TopBottomPanel::top("menu")
            .exact_height(28.0)
            .show(ctx, |ui| {
                self.ui_menu_bar(ui);
            });

        egui::TopBottomPanel::top("toolbar")
            .exact_height(32.0)
            .show(ctx, |ui| {
                self.ui_toolbar(ui);
            });

        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Git · local").weak().small());
                        ui.label(RichText::new("PBR · Shadows · Water v1").weak().small());
                    });
                });
            });

        egui::SidePanel::left("left_dock")
            .default_width(260.0)
            .width_range(200.0..=400.0)
            .show(ctx, |ui| {
                let total = ui.available_height();
                egui::TopBottomPanel::top("outliner_dock")
                    .exact_height(total * 0.58)
                    .resizable(true)
                    .show_inside(ui, |ui| {
                        self.ui_outliner(ui);
                    });
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.ui_filesystem(ui);
                });
            });

        egui::SidePanel::right("right_dock")
            .default_width(320.0)
            .width_range(260.0..=520.0)
            .show(ctx, |ui| {
                self.ui_right_panel(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let total = ui.available_height();
            egui::TopBottomPanel::top("viewport_dock")
                .exact_height((total * 0.62).max(220.0))
                .resizable(true)
                .show_inside(ui, |ui| {
                    self.ui_viewport(ui);
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.bottom_tab == BottomTab::Assets, "Asset Browser")
                        .clicked()
                    {
                        self.bottom_tab = BottomTab::Assets;
                    }
                    if ui
                        .selectable_label(self.bottom_tab == BottomTab::Console, "Console")
                        .clicked()
                    {
                        self.bottom_tab = BottomTab::Console;
                    }
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Assets => self.ui_assets(ui),
                    BottomTab::Console => self.ui_console(ui),
                }
            });
        });
    }
}
