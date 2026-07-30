//! Premium docked editor UI — mockup-faithful shell.
//!
//! Layout: menu · outliner + filesystem · viewport + assets/console ·
//! inspector / node graph · status. Live features: play snapshot, world-item
//! spawn, URL import, interactive node graph.

use std::path::PathBuf;
use std::time::Instant;

use ahash::AHashMap;
use eframe::egui::{self, Color32, ColorImage, RichText, Stroke, TextureHandle, TextureOptions, Vec2};
use shiloh_ecs::Entity;
use shiloh_scene::{
    Camera, Scene, SceneFile, Transform, propagate_transforms, save_scene, set_parent,
};

use crate::import::import_from_url;
use crate::node_graph::NodeGraph;
use crate::play_mode::PlaySession;
use crate::project::Project;
use crate::selection::{SelectMode, Selection};
use crate::viewport::{SceneViewport, ViewportEvent, ViewportTool};
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
    light_cast_shadows: bool,
    sky_atmosphere: bool,
    sky_sun_disk: bool,
    sky_bloom: bool,
    sky_lens_flare: bool,
    fog_enabled: bool,
    viewport: SceneViewport,
    /// Selected asset browser item — click viewport ground to place (Esc clears).
    place_brush: Option<WorldItem>,
    /// Godot-style Select / Move tool for the 3D viewport.
    viewport_tool: ViewportTool,
    /// Brand mark shown in the menu bar (replaces generic gear / settings chrome).
    logo_texture: Option<TextureHandle>,
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
            light_intensity: 5.0,
            light_temperature: 6500.0,
            light_color: [1.0, 1.0, 1.0],
            light_cast_shadows: true,
            sky_atmosphere: true,
            sky_sun_disk: true,
            sky_bloom: true,
            sky_lens_flare: false,
            fog_enabled: true,
            viewport: SceneViewport::new(),
            place_brush: None,
            viewport_tool: ViewportTool::Select,
            logo_texture: None,
        };

        app.log("INFO", "Shiloh3D Editor started", Color32::from_rgb(220, 225, 235));
        app.log(
            "INFO",
            "Viewport: Godot-like LMB place/select · MMB orbit · RMB pan · Q/W tools · G grab",
            Color32::from_rgb(220, 225, 235),
        );
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

        app.log(
            "INFO",
            "Using Renderer: Shiloh Slice Renderer (Vulkan preferred)",
            Color32::from_rgb(220, 225, 235),
        );

        let scene_path = app.scene_path.clone();
        let path_exists = scene_path.as_ref().is_some_and(|p| p.exists());
        if let Some(ref path) = scene_path {
            if path_exists {
                app.load_scene_from(path);
            } else {
                app.log(
                    "WARNING",
                    &format!("No scene file yet at {}", path.display()),
                    Color32::from_rgb(230, 160, 60),
                );
            }
        }

        if app.should_reseed_forest_valley() {
            app.clear_and_seed_forest_valley();
            if let Some(path) = app.default_save_path() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if save_scene(&path, &app.scene, app.camera.as_ref()).is_ok() {
                    app.scene_path = Some(path.clone());
                    app.log(
                        "INFO",
                        &format!("Scene saved · {}", path.display()),
                        Color32::from_rgb(220, 225, 235),
                    );
                }
            }
        }

        app.select_directional_light();
        app.status = "Ready".into();
        app
    }

    fn should_reseed_forest_valley(&self) -> bool {
        let count = self.scene.world.entity_count();
        if count < 3 {
            return true;
        }
        if self.scene.name != "Forest_Valley" {
            return false;
        }
        let named = self
            .entity_names
            .values()
            .filter(|n| !n.starts_with("Entity") && !n.starts_with("entity_"))
            .count();
        named < 8
    }

    fn select_directional_light(&mut self) {
        for (&entity, name) in &self.entity_names {
            if name.contains("DirectionalLight") {
                self.selection.clear();
                self.selection.select(entity);
                self.right_tab = RightTab::Inspector;
                return;
            }
        }
    }

    fn clear_and_seed_forest_valley(&mut self) {
        self.scene = Scene::new("Forest_Valley");
        self.entity_names.clear();
        self.hidden.clear();
        self.locked.clear();
        self.selection.clear();
        self.seed_forest_valley();
    }

    fn spawn_named(&mut self, name: &str, translation: glam::Vec3, scale: glam::Vec3) -> Entity {
        let entity = self.scene.spawn_transform(Transform {
            translation,
            scale,
            ..Transform::default()
        });
        self.entity_names.insert(entity, name.into());
        entity
    }

    fn seed_forest_valley(&mut self) {
        self.spawn_named("DirectionalLight", glam::Vec3::new(0.0, 12.0, 0.0), glam::Vec3::ONE);
        self.spawn_named("SkyAtmosphere", glam::Vec3::ZERO, glam::Vec3::ONE);
        self.spawn_named("FogVolume", glam::Vec3::new(0.0, 2.0, 0.0), glam::Vec3::ONE);
        self.spawn_named(
            "Terrain_Heightmap",
            glam::Vec3::ZERO,
            glam::Vec3::new(1.0, 0.15, 1.0),
        );
        self.spawn_named("WaterBody", glam::Vec3::new(0.0, 0.15, 0.0), glam::Vec3::ONE);

        // Pines — left bank
        let pine_left: [(f32, f32); 12] = [
            (-14.0, 4.0),
            (-12.5, 6.5),
            (-11.0, 2.0),
            (-10.0, 8.0),
            (-9.0, 5.0),
            (-8.0, 1.5),
            (-7.0, 7.0),
            (-6.5, 3.5),
            (-5.5, 9.0),
            (-4.5, 2.5),
            (-3.5, 6.0),
            (-2.5, 4.5),
        ];
        for (i, (x, z)) in pine_left.iter().enumerate() {
            self.spawn_named(
                &format!("Pine_Tall_{:02}", i + 1),
                glam::Vec3::new(*x, 0.0, *z),
                glam::Vec3::new(1.0, 1.2, 1.0),
            );
        }

        // Pines — right bank
        let pine_right: [(f32, f32); 10] = [
            (3.0, -3.0),
            (4.5, -5.5),
            (6.0, -2.0),
            (7.5, -7.0),
            (9.0, -4.0),
            (10.5, -8.5),
            (12.0, -3.5),
            (13.5, -6.0),
            (15.0, -2.5),
            (16.5, -5.0),
        ];
        for (i, (x, z)) in pine_right.iter().enumerate() {
            self.spawn_named(
                &format!("Pine_Tall_{:02}", i + 13),
                glam::Vec3::new(*x, 0.0, *z),
                glam::Vec3::new(0.9, 1.1, 0.9),
            );
        }

        // Pine clusters
        for (i, (x, z)) in [(-8.0, 5.5), (5.0, -6.0), (11.0, -7.5), (-3.0, 7.5)]
            .iter()
            .enumerate()
        {
            self.spawn_named(
                &format!("Pine_Cluster_{:02}", i + 1),
                glam::Vec3::new(*x, 0.0, *z),
                glam::Vec3::new(1.4, 1.0, 1.4),
            );
        }

        // Birches scattered
        for (i, (x, z)) in [
            (-5.0, -2.0),
            (-2.0, 5.0),
            (2.0, 4.0),
            (8.0, -1.0),
            (14.0, -4.0),
            (-11.0, -1.0),
        ]
        .iter()
        .enumerate()
        {
            self.spawn_named(
                &format!("Birch_{:02}", i + 1),
                glam::Vec3::new(*x, 0.0, *z),
                glam::Vec3::new(0.85, 1.0, 0.85),
            );
        }

        // Dead trees
        for (i, (x, z)) in [(-6.0, -4.0), (1.0, -5.0), (10.0, 2.0)].iter().enumerate() {
            self.spawn_named(
                &format!("Dead_Tree_{:02}", i + 1),
                glam::Vec3::new(*x, 0.0, *z),
                glam::Vec3::new(0.7, 0.9, 0.7),
            );
        }

        // Rocks near water
        for (i, (x, z)) in [
            (0.5, 1.0),
            (-1.0, -0.5),
            (1.5, -1.0),
            (-0.5, 1.5),
            (2.0, 0.5),
            (-2.0, 0.0),
        ]
        .iter()
        .enumerate()
        {
            let name = if i == 0 {
                "Rock_09".to_string()
            } else {
                format!("Rock_06_{:02}", i)
            };
            self.spawn_named(
                &name,
                glam::Vec3::new(*x, 0.0, *z),
                glam::Vec3::new(1.2, 0.8, 1.0),
            );
        }

        // CC0 undergrowth (Poly Haven) — FirstGoal valley density between pines.
        for (i, (x, z)) in [
            (-6.0, 3.0),
            (-4.0, 5.5),
            (4.0, -4.0),
            (8.0, -6.5),
            (-1.0, 2.0),
            (2.5, -2.5),
        ]
        .iter()
        .enumerate()
        {
            let name = if i % 2 == 0 {
                format!("Shrub_03_{:02}", i / 2 + 1)
            } else {
                format!("Fern_02_{:02}", i / 2 + 1)
            };
            self.spawn_named(&name, glam::Vec3::new(*x, 0.0, *z), glam::Vec3::ONE);
        }

        // Cliff faces on valley sides
        for (i, (x, z)) in [(-18.0, 0.0), (-17.0, 6.0), (18.0, -2.0), (17.0, 5.0)]
            .iter()
            .enumerate()
        {
            self.spawn_named(
                &format!("Cliff_Face_{:02}", i + 1),
                glam::Vec3::new(*x, 0.0, *z),
                glam::Vec3::new(1.0, 2.5, 0.4),
            );
        }

        // Distant mountains
        for (i, (x, z)) in [
            (-28.0, -12.0),
            (-22.0, 14.0),
            (0.0, -22.0),
            (24.0, -10.0),
            (30.0, 8.0),
        ]
        .iter()
        .enumerate()
        {
            self.spawn_named(
                &format!("Mountain_{:02}", i + 1),
                glam::Vec3::new(*x, 0.0, *z),
                glam::Vec3::new(2.0, 1.8, 1.5),
            );
        }

        propagate_transforms(&mut self.scene.world);
        self.log(
            "INFO",
            "Scene loaded · Forest_Valley",
            Color32::from_rgb(90, 200, 120),
        );
        self.log(
            "INFO",
            &format!(
                "Seeded {} entities · valley trees, rocks, cliffs, mountains",
                self.scene.world.entity_count()
            ),
            Color32::from_rgb(220, 225, 235),
        );
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

    #[allow(dead_code)]
    fn spawn_world_item(&mut self, item: &WorldItem) {
        let offset = (self.scene.world.entity_count() as f32) * 0.35;
        self.spawn_world_item_at(
            item,
            glam::Vec3::new(offset % 5.0, 0.0, offset * 0.2),
        );
    }

    fn spawn_world_item_at(&mut self, item: &WorldItem, mut world: glam::Vec3) {
        let lower = item.spawn_name.to_ascii_lowercase();
        if lower.contains("light") {
            world.y = if lower.contains("directional") {
                12.0
            } else if lower.contains("spot") {
                5.0
            } else {
                3.0
            };
        } else if lower.contains("fog") || lower.contains("sky") {
            world.y = if lower.contains("fog") { 2.0 } else { 0.0 };
        } else if lower.contains("water") {
            world.y = 0.15;
        } else {
            world.y = 0.0;
        }
        self.new_entity_named(item.spawn_name, world);
        self.status = format!("Placed {} at ({:.1}, {:.1})", item.name, world.x, world.z);
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
        self.status = "Ready".into();
        self.log(
            "INFO",
            &format!("Scene loaded · {}", self.scene.name),
            Color32::from_rgb(90, 200, 120),
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

    fn ensure_logo(&mut self, ctx: &egui::Context) {
        if self.logo_texture.is_some() {
            return;
        }
        const LOGO_PNG: &[u8] = include_bytes!("../../logo_shiloh3d.png");
        let Ok(dyn_img) = image::load_from_memory(LOGO_PNG) else {
            return;
        };
        let rgba = dyn_img.into_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_raw();
        let color = ColorImage::from_rgba_unmultiplied(size, &pixels);
        self.logo_texture = Some(ctx.load_texture("shiloh3d_logo", color, TextureOptions::LINEAR));
    }

    // ── Panels ──────────────────────────────────────────────────────────

    fn ui_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            if let Some(logo) = &self.logo_texture {
                ui.add(
                    egui::Image::new(logo)
                        .fit_to_exact_size(Vec2::splat(20.0))
                        .sense(egui::Sense::hover()),
                )
                .on_hover_text("Shiloh3D — Christian-owned engine you can bundle into your game");
                ui.add_space(4.0);
            }
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
                ui.label("Shiloh3D Studio — Christian-owned Rust engine");
                ui.label(
                    RichText::new(
                        "Built to ship inside your game (like Unreal / Godot), not lock you in.",
                    )
                    .weak()
                    .small(),
                );
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
                if ui.button("＋ Sun").clicked() {
                    self.new_entity_named(
                        "DirectionalLight",
                        glam::Vec3::new(0.0, 12.0, 0.0),
                    );
                }
                if ui.button("＋ Point").clicked() {
                    self.new_entity_named("PointLight", glam::Vec3::new(0.0, 3.0, 0.0));
                }
                if ui.button("＋ Spot").clicked() {
                    self.new_entity_named("SpotLight", glam::Vec3::new(0.0, 5.0, 0.0));
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

            let mut clicked: Option<(Entity, SelectMode)> = None;
            let mut toggle_vis: Option<Entity> = None;
            let mut toggle_lock: Option<Entity> = None;

            let entity_rows: Vec<(Entity, String)> = entities
                .iter()
                .enumerate()
                .map(|(i, &e)| (e, self.hierarchy_label(i, e)))
                .collect();

            let draw_entity = |ui: &mut egui::Ui,
                               entity: Entity,
                               label: &str,
                               selected: bool,
                               hidden: bool,
                               locked: bool|
             -> (Option<(Entity, SelectMode)>, Option<Entity>, Option<Entity>) {
                let mut click = None;
                let mut vis = None;
                let mut lock = None;
                ui.horizontal(|ui| {
                    let eye = if hidden { "◌" } else { "◉" };
                    if ui
                        .add(egui::Button::new(eye).frame(false))
                        .on_hover_text("Visibility")
                        .clicked()
                    {
                        vis = Some(entity);
                    }
                    let lock_icon = if locked { "🔒" } else { "🔓" };
                    if ui
                        .add(egui::Button::new(lock_icon).frame(false))
                        .on_hover_text("Lock")
                        .clicked()
                    {
                        lock = Some(entity);
                    }
                    let text = if hidden {
                        RichText::new(label).weak()
                    } else {
                        RichText::new(label)
                    };
                    if ui.selectable_label(selected, text).clicked() {
                        let mode = if ui.input(|i| i.modifiers.shift) {
                            SelectMode::Add
                        } else if ui.input(|i| i.modifiers.command || i.modifiers.ctrl) {
                            SelectMode::Toggle
                        } else {
                            SelectMode::Replace
                        };
                        click = Some((entity, mode));
                    }
                });
                (click, vis, lock)
            };

            egui::CollapsingHeader::new(RichText::new("World").strong())
                .default_open(true)
                .show(ui, |ui| {
                    egui::CollapsingHeader::new("Environment")
                        .default_open(true)
                        .show(ui, |ui| {
                            for (entity, label) in &entity_rows {
                                if !matches!(
                                    label.as_str(),
                                    "DirectionalLight"
                                        | "PointLight"
                                        | "SpotLight"
                                        | "SkyAtmosphere"
                                        | "FogVolume"
                                ) {
                                    continue;
                                }
                                let selected = self.selection.entities.contains(entity);
                                let hidden = *self.hidden.get(entity).unwrap_or(&false);
                                let locked = *self.locked.get(entity).unwrap_or(&false);
                                let (c, v, l) =
                                    draw_entity(ui, *entity, label, selected, hidden, locked);
                                clicked = clicked.or(c);
                                toggle_vis = toggle_vis.or(v);
                                toggle_lock = toggle_lock.or(l);
                            }
                        });

                    egui::CollapsingHeader::new("Terrain")
                        .default_open(true)
                        .show(ui, |ui| {
                            for (entity, label) in &entity_rows {
                                if !label.starts_with("Terrain_")
                                    && !label.starts_with("WaterBody")
                                {
                                    continue;
                                }
                                let selected = self.selection.entities.contains(entity);
                                let hidden = *self.hidden.get(entity).unwrap_or(&false);
                                let locked = *self.locked.get(entity).unwrap_or(&false);
                                let (c, v, l) =
                                    draw_entity(ui, *entity, label, selected, hidden, locked);
                                clicked = clicked.or(c);
                                toggle_vis = toggle_vis.or(v);
                                toggle_lock = toggle_lock.or(l);
                            }
                        });

                    egui::CollapsingHeader::new("WorldObjects")
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::CollapsingHeader::new("Trees")
                                .default_open(true)
                                .show(ui, |ui| {
                                    for (entity, label) in &entity_rows {
                                        if !label.starts_with("Pine_")
                                            && !label.starts_with("Birch")
                                            && !label.starts_with("Dead_Tree")
                                            && !label.starts_with("Shrub_")
                                            && !label.starts_with("Fern_")
                                            && !label.starts_with("Grass_")
                                        {
                                            continue;
                                        }
                                        let selected = self.selection.entities.contains(entity);
                                        let hidden = *self.hidden.get(entity).unwrap_or(&false);
                                        let locked = *self.locked.get(entity).unwrap_or(&false);
                                        let (c, v, l) = draw_entity(
                                            ui, *entity, label, selected, hidden, locked,
                                        );
                                        clicked = clicked.or(c);
                                        toggle_vis = toggle_vis.or(v);
                                        toggle_lock = toggle_lock.or(l);
                                    }
                                });

                            egui::CollapsingHeader::new("Rocks")
                                .default_open(true)
                                .show(ui, |ui| {
                                    for (entity, label) in &entity_rows {
                                        if !label.starts_with("Rock_") {
                                            continue;
                                        }
                                        let selected = self.selection.entities.contains(entity);
                                        let hidden = *self.hidden.get(entity).unwrap_or(&false);
                                        let locked = *self.locked.get(entity).unwrap_or(&false);
                                        let (c, v, l) = draw_entity(
                                            ui, *entity, label, selected, hidden, locked,
                                        );
                                        clicked = clicked.or(c);
                                        toggle_vis = toggle_vis.or(v);
                                        toggle_lock = toggle_lock.or(l);
                                    }
                                });

                            egui::CollapsingHeader::new("Cliffs")
                                .default_open(true)
                                .show(ui, |ui| {
                                    for (entity, label) in &entity_rows {
                                        if !label.starts_with("Cliff_") {
                                            continue;
                                        }
                                        let selected = self.selection.entities.contains(entity);
                                        let hidden = *self.hidden.get(entity).unwrap_or(&false);
                                        let locked = *self.locked.get(entity).unwrap_or(&false);
                                        let (c, v, l) = draw_entity(
                                            ui, *entity, label, selected, hidden, locked,
                                        );
                                        clicked = clicked.or(c);
                                        toggle_vis = toggle_vis.or(v);
                                        toggle_lock = toggle_lock.or(l);
                                    }
                                });
                        });

                    egui::CollapsingHeader::new("Backdrop")
                        .default_open(false)
                        .show(ui, |ui| {
                            for (entity, label) in &entity_rows {
                                if !label.starts_with("Mountain_") {
                                    continue;
                                }
                                let selected = self.selection.entities.contains(entity);
                                let hidden = *self.hidden.get(entity).unwrap_or(&false);
                                let locked = *self.locked.get(entity).unwrap_or(&false);
                                let (c, v, l) =
                                    draw_entity(ui, *entity, label, selected, hidden, locked);
                                clicked = clicked.or(c);
                                toggle_vis = toggle_vis.or(v);
                                toggle_lock = toggle_lock.or(l);
                            }
                        });
                });

            if let Some((entity, mode)) = clicked {
                self.selection.apply(entity, mode);
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
        let selection = self.selection.entities.clone();
        let fps = self.fps_smooth;
        let fog = self.fog_enabled;
        let brush_name = self.place_brush.as_ref().map(|i| i.name);
        let events = self.viewport.ui(
            ui,
            &self.scene,
            &self.entity_names,
            &selection,
            fps,
            fog,
            brush_name,
            &mut self.viewport_tool,
        );
        for event in events {
            match event {
                ViewportEvent::Select { entity, mode } => {
                    self.selection.apply(entity, mode);
                    self.right_tab = RightTab::Inspector;
                    // Godot: picking an existing node while painting drops the brush.
                    if self.place_brush.is_some() {
                        self.place_brush = None;
                        self.status = "Selected node — place mode off".into();
                    }
                }
                ViewportEvent::ClearSelection => {
                    self.selection.clear();
                }
                ViewportEvent::Translate { delta } => {
                    for &entity in &self.selection.entities {
                        if *self.locked.get(&entity).unwrap_or(&false) {
                            continue;
                        }
                        if let Some(transform) = self.scene.world.get_mut::<Transform>(entity) {
                            transform.translation += delta;
                            transform.mark_dirty();
                        }
                    }
                    propagate_transforms(&mut self.scene.world);
                }
                ViewportEvent::PlaceAt { world } => {
                    if let Some(item) = self.place_brush.clone() {
                        self.spawn_world_item_at(&item, world);
                    }
                }
                ViewportEvent::ExitPlaceMode => {
                    self.place_brush = None;
                }
            }
        }
        let aspect = self.viewport.cam.to_camera(16.0 / 9.0);
        self.camera = Some(aspect);
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
        ui.label(
            RichText::new("Basic world items — select, then click ground in the viewport to place")
                .weak()
                .small(),
        );
        if let Some(brush_name) = self.place_brush.as_ref().map(|b| b.name) {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Placing: {brush_name}"))
                        .color(Color32::from_rgb(90, 200, 120)),
                );
                if ui.small_button("Cancel").clicked() {
                    self.place_brush = None;
                }
            });
        }

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
                    let brush_id = self.place_brush.as_ref().map(|b| b.id);
                    for item in items {
                        let size = Vec2::new(96.0, 88.0);
                        let (rect, resp) =
                            ui.allocate_exact_size(size, egui::Sense::click());
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 6.0, Color32::from_rgb(34, 38, 50));
                        let selected = brush_id == Some(item.id);
                        painter.rect_stroke(
                            rect,
                            6.0,
                            Stroke::new(
                                if selected { 2.0_f32 } else { 1.0_f32 },
                                if selected {
                                    Color32::from_rgb(90, 200, 120)
                                } else {
                                    Color32::from_rgb(55, 62, 80)
                                },
                            ),
                            egui::StrokeKind::Outside,
                        );
                        let thumb = egui::Rect::from_center_size(
                            rect.center() - Vec2::new(0.0, 12.0),
                            Vec2::new(52.0, 40.0),
                        );
                        draw_asset_thumbnail(&painter, thumb, &item);
                        painter.text(
                            rect.center_bottom() - Vec2::new(0.0, 8.0),
                            egui::Align2::CENTER_BOTTOM,
                            item.name,
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(210, 215, 225),
                        );
                        if resp.clicked() {
                            if brush_id == Some(item.id) {
                                self.place_brush = None;
                                self.status = "Place mode cancelled".into();
                            } else {
                                self.status = format!(
                                    "Place mode: {} — click ground in viewport (Esc cancels)",
                                    item.name
                                );
                                self.place_brush = Some(item.clone());
                            }
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
                ui.label("Rotation");
                let (rx, ry, rz) = transform.rotation.to_euler(glam::EulerRot::XYZ);
                let mut euler_x = rx.to_degrees();
                let mut euler_y = ry.to_degrees();
                let mut euler_z = rz.to_degrees();
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut euler_x)
                                .speed(0.5)
                                .suffix("°")
                                .prefix("X "),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut euler_y)
                                .speed(0.5)
                                .suffix("°")
                                .prefix("Y "),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut euler_z)
                                .speed(0.5)
                                .suffix("°")
                                .prefix("Z "),
                        )
                        .changed();
                });
                if (euler_x - rx.to_degrees()).abs() > 1e-4
                    || (euler_y - ry.to_degrees()).abs() > 1e-4
                    || (euler_z - rz.to_degrees()).abs() > 1e-4
                {
                    transform.rotation = glam::Quat::from_euler(
                        glam::EulerRot::XYZ,
                        euler_x.to_radians(),
                        euler_y.to_radians(),
                        euler_z.to_radians(),
                    );
                    changed = true;
                }
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
                            .text("Temperature (K)"),
                    );
                    ui.checkbox(&mut self.light_cast_shadows, "Cast Shadows");
                });
        }

        if is_sky || is_light {
            egui::CollapsingHeader::new("Sky & Atmosphere")
                .default_open(is_sky || is_light)
                .show(ui, |ui| {
                    ui.checkbox(&mut self.sky_atmosphere, "Atmosphere");
                    ui.checkbox(&mut self.sky_sun_disk, "Sun Disk");
                    ui.checkbox(&mut self.sky_bloom, "Bloom");
                    ui.checkbox(&mut self.sky_lens_flare, "Lens Flare");
                    ui.checkbox(&mut self.fog_enabled, "Enable fog");
                    ui.label(
                        RichText::new("Hooks into slice fog / water atmosphere track")
                            .weak()
                            .small(),
                    );
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

/// Simple shape icons for asset browser cards (tree, rock, water, sun…).
fn draw_asset_thumbnail(painter: &egui::Painter, rect: egui::Rect, item: &WorldItem) {
    painter.rect_filled(rect, 4.0, Color32::from_rgb(24, 28, 36));
    let cx = rect.center().x;
    let ground_y = rect.bottom() - 4.0;

    match item.id {
        "pine_tall" | "pine_cluster" => {
            let trunk = egui::Rect::from_min_max(
                egui::pos2(cx - 3.0, ground_y - 14.0),
                egui::pos2(cx + 3.0, ground_y),
            );
            painter.rect_filled(trunk, 1.0, Color32::from_rgb(80, 55, 35));
            let foliage = egui::Rect::from_center_size(
                egui::pos2(cx, ground_y - 18.0),
                Vec2::new(22.0, 16.0),
            );
            painter.rect_filled(foliage, 3.0, Color32::from_rgb(40, 100, 50));
            if item.id == "pine_cluster" {
                painter.rect_filled(
                    egui::Rect::from_center_size(
                        egui::pos2(cx - 8.0, ground_y - 12.0),
                        Vec2::new(12.0, 10.0),
                    ),
                    2.0,
                    Color32::from_rgb(35, 90, 45),
                );
            }
        }
        "birch" | "dead_tree" => {
            let col = if item.id == "birch" {
                Color32::from_rgb(200, 195, 180)
            } else {
                Color32::from_rgb(90, 80, 70)
            };
            painter.line_segment(
                [
                    egui::pos2(cx, ground_y),
                    egui::pos2(cx - 4.0, ground_y - 20.0),
                ],
                Stroke::new(2.5_f32, col),
            );
            if item.id == "birch" {
                painter.circle_filled(
                    egui::pos2(cx - 4.0, ground_y - 22.0),
                    8.0,
                    Color32::from_rgb(60, 120, 55),
                );
            }
        }
        "grass_patch" => {
            for dx in [-8.0_f32, -3.0, 2.0, 7.0] {
                painter.line_segment(
                    [
                        egui::pos2(cx + dx, ground_y),
                        egui::pos2(cx + dx - 2.0, ground_y - 10.0),
                    ],
                    Stroke::new(1.5_f32, Color32::from_rgb(50, 130, 55)),
                );
            }
        }
        "rock_large" | "rock_scatter" => {
            let w = if item.id == "rock_large" { 28.0 } else { 18.0 };
            let h = if item.id == "rock_large" { 16.0 } else { 12.0 };
            let rock = egui::Rect::from_center_size(
                egui::pos2(cx, ground_y - h * 0.5),
                Vec2::new(w, h),
            );
            painter.rect_filled(rock, 4.0, Color32::from_rgb(110, 95, 80));
            painter.line_segment(
                [rock.left_top() + Vec2::new(4.0, 4.0), rock.right_bottom() - Vec2::new(6.0, 3.0)],
                Stroke::new(1.0_f32, Color32::from_rgb(80, 70, 60)),
            );
        }
        "cliff" | "heightmap" => {
            let pts = vec![
                egui::pos2(rect.left() + 4.0, ground_y),
                egui::pos2(rect.left() + 10.0, ground_y - 18.0),
                egui::pos2(rect.right() - 8.0, ground_y - 12.0),
                egui::pos2(rect.right() - 4.0, ground_y),
            ];
            painter.add(egui::Shape::convex_polygon(
                pts,
                Color32::from_rgb(95, 80, 65),
                Stroke::NONE,
            ));
        }
        "water_body" => {
            let water = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 4.0, ground_y - 10.0),
                egui::pos2(rect.right() - 4.0, ground_y),
            );
            painter.rect_filled(water, 2.0, Color32::from_rgb(40, 90, 140));
            painter.line_segment(
                [
                    egui::pos2(rect.left() + 8.0, ground_y - 6.0),
                    egui::pos2(rect.right() - 10.0, ground_y - 4.0),
                ],
                Stroke::new(1.0_f32, Color32::from_rgb(120, 180, 220)),
            );
        }
        "dir_light" | "sky_atmosphere" | "fog_volume" | "point_light" | "spot_light" => {
            painter.circle_filled(
                egui::pos2(cx, rect.center().y - 2.0),
                12.0,
                Color32::from_rgb(240, 210, 90),
            );
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let inner = egui::pos2(cx + a.cos() * 14.0, rect.center().y - 2.0 + a.sin() * 14.0);
                let outer = egui::pos2(cx + a.cos() * 20.0, rect.center().y - 2.0 + a.sin() * 20.0);
                painter.line_segment([inner, outer], Stroke::new(2.0_f32, Color32::from_rgb(240, 210, 90)));
            }
        }
        _ => {
            painter.rect_filled(
                rect.shrink(8.0),
                3.0,
                Color32::from_rgb(60, 70, 90),
            );
        }
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        Self::apply_theme(ctx);
        self.ensure_logo(ctx);

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.place_brush.is_some() {
            self.place_brush = None;
            self.status = "Place mode cancelled".into();
        }
        // Godot 3D editor shortcuts: Q = Select, W = Move (E/R later).
        if self.place_brush.is_none() {
            if ctx.input(|i| i.key_pressed(egui::Key::Q)) {
                self.viewport_tool = ViewportTool::Select;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::W)) {
                self.viewport_tool = ViewportTool::Move;
            }
        }

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
                    ui.label(RichText::new("Ready").strong());
                    if !self.status.is_empty() && self.status != "Ready" {
                        ui.label(RichText::new("|").weak());
                        ui.label(&self.status);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{:.0} FPS", self.fps_smooth))
                                .monospace()
                                .color(Color32::from_rgb(90, 200, 120)),
                        );
                        ui.label(RichText::new("|").weak());
                        ui.label(RichText::new("Git: main").weak().small());
                        ui.label(RichText::new("|").weak());
                        ui.label(RichText::new("Water v1").weak().small());
                        ui.label(RichText::new("|").weak());
                        ui.label(RichText::new("Lit").weak().small());
                        ui.label(RichText::new("|").weak());
                        ui.label(RichText::new("Vulkan").weak().small());
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
