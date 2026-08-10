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

use crate::asset_cook::{ensure_cook_stub, AssetCookStub};
use crate::import::import_from_url;
use crate::layouts::EditorLayout;
use crate::node_graph::NodeGraph;
use crate::play_mode::PlaySession;
use crate::project::Project;
use crate::script_editor::ScriptEditorState;
use crate::selection::{SelectMode, Selection};
use crate::viewport::{GizmoAxis, SceneViewport, ViewportEvent, ViewportTool};
use crate::world_items::{
    WorldItem, WorldItemCategory, builtin_world_items, ensure_project_layout,
    scan_project_assets,
};
use shiloh_scripting::ScriptComponent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RightTab {
    Inspector,
    Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Assets,
    Console,
    Script,
}

/// Borrowed from Godot 4: main screen strip (3D / Script / Game).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum WorkspaceMode {
    #[default]
    ThreeD,
    Script,
    Game,
}

#[derive(Debug, Clone)]
struct ConsoleLine {
    level: &'static str,
    message: String,
    color: Color32,
}

#[derive(Debug, Clone)]
struct SceneTab {
    name: String,
    #[allow(dead_code)]
    path: Option<PathBuf>,
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
    /// Godot-style Select / Move / Rotate / Scale + Unreal Landscape/Foliage + RayAccurate.
    viewport_tool: ViewportTool,
    /// Phase 5 landscape chunk (Unreal Landscape Mode data).
    terrain: shiloh_scene::TerrainChunk,
    /// Phase 5 foliage instances (Unreal Foliage Mode data).
    foliage_layer: shiloh_scene::FoliageLayer,
    /// Paint layer index 0..3 for Landscape paint (defaults: grass/dirt/rock/sand).
    terrain_paint_layer: u8,
    /// Brand mark shown in the menu bar (replaces generic gear / settings chrome).
    logo_texture: Option<TextureHandle>,
    /// Borrowed from Godot 4: 3D / Script / Game workspace strip.
    workspace: WorkspaceMode,
    /// Borrowed from Godot 4: hide side docks for distraction-free viewport.
    distraction_free: bool,
    /// Borrowed from Unreal Engine: Content Browser drawer (Ctrl+Space).
    content_drawer_open: bool,
    /// Borrowed from Unreal Engine: Game view hides gizmos (G).
    game_view: bool,
    /// Borrowed from Unreal / Godot: grid snap; Ctrl holds free move.
    grid_snap: bool,
    snap_size: f32,
    outliner_filter: String,
    /// Type chip: None = all, or "Light" / "Mesh" / "Water" / …
    outliner_type: Option<&'static str>,
    inspector_filter: String,
    left_dock_width: f32,
    right_dock_width: f32,
    /// Landscape Mode: false = sculpt height, true = paint weight layer.
    landscape_paint: bool,
    scene_tabs: Vec<SceneTab>,
    active_scene_tab: usize,
    /// Per-entity script attachments (Phase 5 ScriptComponent).
    script_components: AHashMap<Entity, ScriptComponent>,
    /// Designer script IDE (autocomplete + IntelliSense).
    script_editor: ScriptEditorState,
    layout_name: String,
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
            scene_path: scene_path.clone(),
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
            terrain: {
                let mut t = shiloh_scene::TerrainChunk::flat(96, 64.0);
                // Soft valley undulation so Landscape Mode has readable relief day-one.
                t.sculpt(32.0, 32.0, 18.0, 0.45);
                t.sculpt(20.0, 40.0, 10.0, 0.25);
                t.sculpt(44.0, 22.0, 8.0, -0.15);
                t
            },
            foliage_layer: shiloh_scene::FoliageLayer::default(),
            terrain_paint_layer: 0,
            logo_texture: None,
            workspace: WorkspaceMode::ThreeD,
            distraction_free: false,
            content_drawer_open: false,
            game_view: false,
            grid_snap: true,
            snap_size: 0.5,
            outliner_filter: String::new(),
            outliner_type: None,
            inspector_filter: String::new(),
            left_dock_width: 260.0,
            right_dock_width: 300.0,
            landscape_paint: false,
            scene_tabs: vec![SceneTab {
                name: "Forest_Valley".into(),
                path: scene_path.clone(),
            }],
            active_scene_tab: 0,
            script_components: AHashMap::default(),
            script_editor: ScriptEditorState::default(),
            layout_name: "Default".into(),
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
            let script = self.load_play_script();
            self.play
                .enter_play(&self.scene, self.camera.as_ref(), script.as_deref());
            self.status = if script.is_some() {
                "Playing · Rhai on_ready/on_update".into()
            } else {
                "Playing (Stop restores edit snapshot)".into()
            };
            self.log(
                "INFO",
                if script.is_some() {
                    "Play mode entered · RhaiHost wired"
                } else {
                    "Play mode entered · live simulation (no Scripts/*.rhai)"
                },
                Color32::from_rgb(90, 200, 120),
            );
        }
    }

    /// Prefer `Scripts/demo_spin.rhai`, else first `.rhai` under Scripts/.
    fn load_play_script(&self) -> Option<String> {
        let project = self.project.as_ref()?;
        let scripts = project.root.join("Scripts");
        let preferred = scripts.join("demo_spin.rhai");
        if preferred.is_file() {
            return std::fs::read_to_string(preferred).ok();
        }
        let rd = std::fs::read_dir(&scripts).ok()?;
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rhai") {
                return std::fs::read_to_string(path).ok();
            }
        }
        None
    }

    fn apply_script_commands(&mut self, commands: Vec<shiloh_scripting::ScriptCommand>) {
        use shiloh_scripting::ScriptCommand;
        for cmd in commands {
            match cmd {
                ScriptCommand::Log(msg) => {
                    self.log("SCRIPT", &msg, Color32::from_rgb(140, 200, 255));
                }
                ScriptCommand::SetTranslation {
                    entity_index,
                    x,
                    y,
                    z,
                } => {
                    let mut entities = Vec::new();
                    self.scene
                        .world
                        .for_each::<Transform>(|e, _| entities.push(e));
                    if let Some(&entity) = entities.get(entity_index as usize) {
                        if let Some(t) = self.scene.world.get_mut::<Transform>(entity) {
                            t.translation = glam::Vec3::new(x as f32, y as f32, z as f32);
                            t.mark_dirty();
                        }
                    }
                    propagate_transforms(&mut self.scene.world);
                }
                ScriptCommand::SpawnNamed { name, x, y, z } => {
                    self.spawn_named(
                        &name,
                        glam::Vec3::new(x as f32, y as f32, z as f32),
                        glam::Vec3::ONE,
                    );
                }
                ScriptCommand::EmitSignal { name } => {
                    self.log(
                        "SIGNAL",
                        &format!("emit `{name}`"),
                        Color32::from_rgb(255, 200, 120),
                    );
                }
                ScriptCommand::PlayAudio { name } => {
                    self.log(
                        "AUDIO",
                        &format!("play `{name}`"),
                        Color32::from_rgb(180, 160, 255),
                    );
                }
            }
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
                if ui.button("New Scene").clicked() {
                    // Borrowed from Godot 4: Scene tabs + New Scene (+ scene).
                    self.new_scene_tab();
                    ui.close_menu();
                }
                if ui
                    .button("＋ Scene")
                    .on_hover_text("Add a new scene tab")
                    .clicked()
                {
                    self.new_scene_tab();
                    ui.close_menu();
                }
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
                if ui.button("Cook selected mesh stub").clicked() {
                    self.cook_selected_stub();
                    ui.close_menu();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("New Entity").clicked() {
                    self.new_entity();
                    ui.close_menu();
                }
                // Borrowed from Godot 4: Ctrl+D duplicate node.
                if ui.button("Duplicate (Ctrl+D)").clicked() {
                    self.duplicate_selection(glam::Vec3::new(1.0, 0.0, 0.0));
                    ui.close_menu();
                }
            });
            ui.menu_button("Build", |ui| {
                ui.label(
                    RichText::new("One-click desktop pack — Windows · macOS · Ubuntu")
                        .weak()
                        .small(),
                );
                if ui
                    .button("Pack Desktop…")
                    .on_hover_text("Runs packaging/one-click-pack.sh (host OS now; CI fills all three)")
                    .clicked()
                {
                    self.status = "Pack: run `./packaging/one-click-pack.sh` or `shiloh-cli pack`".into();
                    self.log(
                        "INFO",
                        "Desktop pack → dist/desktop/ (see docs/PACKAGING.md)",
                        Color32::from_rgb(90, 200, 120),
                    );
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Cook selected mesh stub").clicked() {
                    self.cook_selected_stub();
                    ui.close_menu();
                }
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
                ui.separator();
                // Borrowed from Godot 4: Editor Layouts.
                ui.menu_button("Layouts", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut self.layout_name);
                    });
                    if ui.button("Save Layout").clicked() {
                        self.save_layout();
                        ui.close_menu();
                    }
                    if let Some(project) = self.project.as_ref() {
                        for name in EditorLayout::list(&project.root) {
                            if ui.button(format!("Load “{name}”")).clicked() {
                                self.load_layout(&name);
                                ui.close_menu();
                            }
                        }
                    }
                });
                if ui
                    .checkbox(&mut self.distraction_free, "Distraction-free")
                    .clicked()
                {
                    ui.close_menu();
                }
                if ui.checkbox(&mut self.grid_snap, "Grid snap").changed() {
                    // keep open
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

            // Borrowed from Godot 4: 3D · Script · Game workspace strip.
            for (label, mode) in [
                ("3D", WorkspaceMode::ThreeD),
                ("Script", WorkspaceMode::Script),
                ("Game", WorkspaceMode::Game),
            ] {
                if ui
                    .selectable_label(self.workspace == mode, label)
                    .clicked()
                {
                    self.workspace = mode;
                    if mode == WorkspaceMode::Script {
                        self.bottom_tab = BottomTab::Script;
                        self.right_tab = RightTab::Node;
                    } else if mode == WorkspaceMode::Game {
                        self.game_view = true;
                    } else {
                        self.game_view = false;
                    }
                }
            }

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
                // Borrowed from Godot 4: scene tabs above viewport.
                for (i, tab) in self.scene_tabs.iter().enumerate().rev() {
                    let selected = i == self.active_scene_tab;
                    if ui.selectable_label(selected, &tab.name).clicked() {
                        self.active_scene_tab = i;
                    }
                }
                if ui.small_button("＋").on_hover_text("New Scene").clicked() {
                    self.new_scene_tab();
                }
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
        // Borrowed from Unreal Engine: World Outliner search + type filters.
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut self.outliner_filter)
                    .desired_width(120.0)
                    .hint_text("Filter…"),
            );
        });
        ui.horizontal(|ui| {
            for (label, chip) in [
                ("All", None),
                ("Light", Some("light")),
                ("Mesh", Some("mesh")),
                ("Water", Some("water")),
                ("Terrain", Some("terrain")),
            ] {
                if ui
                    .selectable_label(self.outliner_type == chip, label)
                    .clicked()
                {
                    self.outliner_type = chip;
                }
            }
        });
        ui.separator();

        egui::ScrollArea::vertical().id_salt("outliner").show(ui, |ui| {
            let mut entities = Vec::new();
            self.scene.world.for_each::<Transform>(|e, _| entities.push(e));

            let filter = self.outliner_filter.to_ascii_lowercase();
            let type_chip = self.outliner_type;

            let mut clicked: Option<(Entity, SelectMode)> = None;
            let mut toggle_vis: Option<Entity> = None;
            let mut toggle_lock: Option<Entity> = None;

            let entity_rows: Vec<(Entity, String)> = entities
                .iter()
                .enumerate()
                .map(|(i, &e)| (e, self.hierarchy_label(i, e)))
                .filter(|(_, label)| {
                    let lower = label.to_ascii_lowercase();
                    if !filter.is_empty() && !lower.contains(&filter) {
                        return false;
                    }
                    match type_chip {
                        Some("light") => {
                            lower.contains("light") || lower.contains("sky") || lower.contains("fog")
                        }
                        Some("mesh") => {
                            !(lower.contains("light")
                                || lower.contains("sky")
                                || lower.contains("fog")
                                || lower.contains("water")
                                || lower.contains("terrain")
                                || lower.contains("heightmap"))
                        }
                        Some("water") => lower.contains("water"),
                        Some("terrain") => {
                            lower.contains("terrain") || lower.contains("heightmap")
                        }
                        _ => true,
                    }
                })
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

            // Flat filtered list when searching / type-filtering; folders otherwise.
            let use_flat = !filter.is_empty() || type_chip.is_some();
            if use_flat {
                for (entity, label) in &entity_rows {
                    let selected = self.selection.entities.contains(entity);
                    let hidden = *self.hidden.get(entity).unwrap_or(&false);
                    let locked = *self.locked.get(entity).unwrap_or(&false);
                    let (c, v, l) = draw_entity(ui, *entity, label, selected, hidden, locked);
                    clicked = clicked.or(c);
                    toggle_vis = toggle_vis.or(v);
                    toggle_lock = toggle_lock.or(l);
                }
            } else {
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
                                    let lower = label.to_ascii_lowercase();
                                    if !(lower.contains("terrain")
                                        || lower.contains("heightmap")
                                        || lower.contains("water")
                                        || lower.contains("mountain")
                                        || lower.contains("cliff"))
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
                                for (entity, label) in &entity_rows {
                                    let lower = label.to_ascii_lowercase();
                                    if matches!(
                                        label.as_str(),
                                        "DirectionalLight"
                                            | "PointLight"
                                            | "SpotLight"
                                            | "SkyAtmosphere"
                                            | "FogVolume"
                                    ) || lower.contains("terrain")
                                        || lower.contains("heightmap")
                                        || lower.contains("water")
                                        || lower.contains("mountain")
                                        || lower.contains("cliff")
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
                    });
            }

            if let Some((entity, mode)) = clicked {
                self.selection.apply(entity, mode);
                self.right_tab = RightTab::Inspector;
            }
            if let Some(entity) = toggle_vis {
                let cur = *self.hidden.get(&entity).unwrap_or(&false);
                self.hidden.insert(entity, !cur);
            }
            if let Some(entity) = toggle_lock {
                let cur = *self.locked.get(&entity).unwrap_or(&false);
                self.locked.insert(entity, !cur);
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
        // Mode tool palette under Modes (Landscape / Foliage).
        if self.viewport_tool == ViewportTool::Landscape {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Landscape").strong().small());
                // Borrowed from Unreal: sculpt vs paint without a material graph.
                if ui
                    .selectable_label(!self.landscape_paint, "Sculpt")
                    .clicked()
                {
                    self.landscape_paint = false;
                }
                if ui
                    .selectable_label(self.landscape_paint, "Paint")
                    .clicked()
                {
                    self.landscape_paint = true;
                }
                ui.separator();
                for (i, name) in ["Grass", "Dirt", "Rock", "Sand"].iter().enumerate() {
                    if ui
                        .selectable_label(self.terrain_paint_layer == i as u8, *name)
                        .clicked()
                    {
                        self.terrain_paint_layer = i as u8;
                        self.landscape_paint = true;
                    }
                }
            });
        } else if self.viewport_tool == ViewportTool::Foliage {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Foliage").strong().small());
                ui.label("Density");
                ui.add(egui::Slider::new(&mut self.foliage_layer.density, 0.05..=2.0));
                ui.checkbox(&mut self.foliage_layer.align_to_normal, "Align");
                ui.label(
                    RichText::new(format!("{} inst", self.foliage_layer.instances.len()))
                        .weak()
                        .small(),
                );
            });
        }

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
            Some(&self.terrain),
            Some(&self.foliage_layer),
            self.grid_snap,
            self.snap_size,
            self.terrain_paint_layer,
            self.landscape_paint,
            self.game_view,
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
                    self.translate_selection(delta);
                }
                ViewportEvent::DuplicateTranslate { delta } => {
                    // Borrowed from Unreal Engine: Alt+drag duplicate-and-move.
                    self.duplicate_selection(delta);
                }
                ViewportEvent::TranslateAxis { axis, delta } => {
                    let v = match axis {
                        GizmoAxis::X => glam::Vec3::X * delta,
                        GizmoAxis::Y => glam::Vec3::Y * delta,
                        GizmoAxis::Z => glam::Vec3::Z * delta,
                    };
                    self.translate_selection(v);
                }
                ViewportEvent::RotateY { radians } => {
                    self.rotate_selection_y(radians);
                }
                ViewportEvent::RotateAxis { axis, radians } => {
                    self.rotate_selection_axis(axis, radians);
                }
                ViewportEvent::ScaleUniform { factor } => {
                    self.scale_selection(None, factor);
                }
                ViewportEvent::ScaleAxis { axis, factor } => {
                    self.scale_selection(Some(axis), factor);
                }
                ViewportEvent::TerrainSculpt {
                    world,
                    strength,
                    radius,
                } => {
                    let half = self.terrain.world_size * 0.5;
                    self.terrain.sculpt(
                        world.x + half,
                        world.z + half,
                        radius,
                        strength,
                    );
                    self.status = format!(
                        "Sculpt · h={:.2}",
                        self.terrain.height_at_world(world.x + half, world.z + half)
                    );
                }
                ViewportEvent::TerrainPaint {
                    world,
                    layer,
                    strength,
                    radius,
                } => {
                    let half = self.terrain.world_size * 0.5;
                    let layer = if self.landscape_paint {
                        self.terrain_paint_layer as usize
                    } else {
                        layer as usize
                    };
                    self.terrain.paint_layer(
                        world.x + half,
                        world.z + half,
                        radius,
                        strength,
                        layer,
                    );
                    self.status = format!("Paint layer {layer}");
                }
                ViewportEvent::FoliagePaint { world, erase } => {
                    if erase {
                        self.foliage_layer.erase([world.x, world.z], 2.0);
                        self.status = format!(
                            "Foliage erase · {}",
                            self.foliage_layer.instances.len()
                        );
                    } else {
                        let seed = (self.foliage_layer.instances.len() as u64)
                            .wrapping_mul(0x9E37_79B9)
                            .wrapping_add(1);
                        self.foliage_layer.paint_add(
                            "pine",
                            [world.x, world.z],
                            world.y,
                            2.5,
                            1.0,
                            0.25,
                            seed,
                        );
                        self.status = format!(
                            "Foliage paint · {}",
                            self.foliage_layer.instances.len()
                        );
                    }
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

    fn translate_selection(&mut self, delta: glam::Vec3) {
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

    fn rotate_selection_y(&mut self, radians: f32) {
        self.rotate_selection_axis(GizmoAxis::Y, radians);
    }

    fn rotate_selection_axis(&mut self, axis: GizmoAxis, radians: f32) {
        let q = match axis {
            GizmoAxis::X => glam::Quat::from_rotation_x(radians),
            GizmoAxis::Y => glam::Quat::from_rotation_y(radians),
            GizmoAxis::Z => glam::Quat::from_rotation_z(radians),
        };
        for &entity in &self.selection.entities {
            if *self.locked.get(&entity).unwrap_or(&false) {
                continue;
            }
            if let Some(transform) = self.scene.world.get_mut::<Transform>(entity) {
                transform.rotation = q * transform.rotation;
                transform.mark_dirty();
            }
        }
        propagate_transforms(&mut self.scene.world);
    }

    fn scale_selection(&mut self, axis: Option<GizmoAxis>, factor: f32) {
        for &entity in &self.selection.entities {
            if *self.locked.get(&entity).unwrap_or(&false) {
                continue;
            }
            if let Some(transform) = self.scene.world.get_mut::<Transform>(entity) {
                match axis {
                    Some(GizmoAxis::X) => transform.scale.x *= factor,
                    Some(GizmoAxis::Y) => transform.scale.y *= factor,
                    Some(GizmoAxis::Z) => transform.scale.z *= factor,
                    None => transform.scale *= factor,
                }
                transform.mark_dirty();
            }
        }
        propagate_transforms(&mut self.scene.world);
    }

    /// Borrowed from Godot 4: Ctrl+D duplicate with optional offset.
    fn duplicate_selection(&mut self, offset: glam::Vec3) {
        let selected = self.selection.entities.clone();
        let mut new_sel = Vec::new();
        for entity in selected {
            let Some(t) = self.scene.world.get::<Transform>(entity).cloned() else {
                continue;
            };
            let name = self
                .entity_names
                .get(&entity)
                .cloned()
                .unwrap_or_else(|| "Entity".into());
            let mut nt = t;
            nt.translation += offset;
            let e = self.scene.spawn_transform(nt);
            self.entity_names.insert(e, format!("{name}_dup"));
            if let Some(script) = self.script_components.get(&entity).cloned() {
                self.script_components.insert(e, script);
            }
            new_sel.push(e);
        }
        propagate_transforms(&mut self.scene.world);
        self.selection.entities = new_sel;
        self.status = "Duplicated selection".into();
    }

    fn new_scene_tab(&mut self) {
        let n = self.scene_tabs.len() + 1;
        self.scene_tabs.push(SceneTab {
            name: format!("Scene_{n}"),
            path: None,
        });
        self.active_scene_tab = self.scene_tabs.len() - 1;
        self.status = "New scene tab (stub — same world for now)".into();
    }

    fn save_layout(&mut self) {
        let Some(project) = self.project.as_ref() else {
            self.status = "Open a project to save layouts".into();
            return;
        };
        let layout = EditorLayout {
            name: self.layout_name.clone(),
            left_width: self.left_dock_width,
            right_width: self.right_dock_width,
            bottom_height: 180.0,
            distraction_free: self.distraction_free,
            grid_snap: self.grid_snap,
            snap_size: self.snap_size,
        };
        match layout.save(&project.root) {
            Ok(()) => {
                self.status = format!("Saved layout “{}”", layout.name);
                self.log("INFO", &self.status.clone(), Color32::from_rgb(90, 200, 120));
            }
            Err(err) => self.status = format!("Layout save failed: {err}"),
        }
    }

    fn load_layout(&mut self, name: &str) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        match EditorLayout::load(&project.root, name) {
            Ok(layout) => {
                self.layout_name = layout.name;
                self.left_dock_width = layout.left_width;
                self.right_dock_width = layout.right_width;
                self.distraction_free = layout.distraction_free;
                self.grid_snap = layout.grid_snap;
                self.snap_size = layout.snap_size;
                self.status = format!("Loaded layout “{}”", self.layout_name);
            }
            Err(err) => self.status = format!("Layout load failed: {err}"),
        }
    }

    fn cook_selected_stub(&mut self) {
        let Some(project) = self.project.as_ref() else {
            self.status = "Open a project to cook stubs".into();
            return;
        };
        let mesh = project.root.join("Assets/Meshes/pine_hero.glb");
        if let Some(parent) = mesh.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if !mesh.exists() {
            let _ = std::fs::write(&mesh, b"stub");
        }
        match ensure_cook_stub(&mesh, [0.8, 2.4, 0.8]) {
            Ok(stub) => {
                self.status = format!(
                    "Cooked {} · collision+LOD",
                    AssetCookStub::meta_path_for(&mesh).display()
                );
                self.log(
                    "INFO",
                    &format!("Cook stub source={}", stub.source),
                    Color32::from_rgb(90, 200, 120),
                );
            }
            Err(err) => self.status = format!("Cook failed: {err}"),
        }
    }

    fn focus_selected(&mut self) {
        // Borrowed from Unreal / Godot: F frames selection.
        let mut center = glam::Vec3::ZERO;
        let mut count = 0usize;
        let mut radius = 2.0_f32;
        for &entity in &self.selection.entities {
            if let Some(t) = self.scene.world.get::<Transform>(entity) {
                center += t.translation;
                count += 1;
                radius = radius.max(t.scale.max_element() * 2.0);
            }
        }
        if count > 0 {
            center /= count as f32;
            self.viewport.cam.focus_selection(center, radius);
            self.status = "Focused selection".into();
        }
    }

    fn ui_content_drawer(&mut self, ctx: &egui::Context) {
        // Borrowed from Unreal Engine: Content Browser drawer (Ctrl+Space).
        if !self.content_drawer_open {
            return;
        }
        egui::Window::new("Content Browser")
            .collapsible(false)
            .resizable(true)
            .default_size([520.0, 280.0])
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -40.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Esc closes · click asset to place brush")
                        .weak()
                        .small(),
                );
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for item in self.world_items.clone() {
                            let resp = ui
                                .add_sized([88.0, 72.0], egui::Button::new(item.name))
                                .on_hover_text(item.description);
                            if resp.clicked() {
                                self.status =
                                    format!("Place: {} — click ground", item.name);
                                self.place_brush = Some(item);
                                self.content_drawer_open = false;
                            }
                        }
                    });
                });
                if ui.button("Close").clicked() {
                    self.content_drawer_open = false;
                }
            });
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

    fn ui_script_dock(&mut self, ui: &mut egui::Ui) {
        // Borrowed from Godot: scripts dock while Scene tree stays visible (never hide Outliner).
        // IntelliSense adapted from egui_code_editor Completer UX (MIT) — see script_editor.rs.
        let scripts_dir = self.project.as_ref().map(|p| p.root.join("Scripts"));
        self.script_editor
            .ui(ui, scripts_dir.as_deref());

        ui.add_space(6.0);
        ui.separator();
        ui.label(RichText::new("Attach to selected entity").strong().small());
        if let Some(entity) = self.selection.primary() {
            let name = self
                .entity_names
                .get(&entity)
                .cloned()
                .unwrap_or_else(|| format!("Entity {}", entity.index()));
            ui.horizontal(|ui| {
                ui.label(format!("• {name}"));
                if ui
                    .add_sized(
                        [160.0, 28.0],
                        egui::Button::new("Attach open script"),
                    )
                    .on_hover_text("Bind the open .rhai to this entity for Play")
                    .clicked()
                {
                    let rel = if self.script_editor.open_rel.is_empty() {
                        "Scripts/untitled.rhai".to_string()
                    } else {
                        self.script_editor.open_rel.clone()
                    };
                    if let Some(dir) = scripts_dir.as_ref() {
                        let _ = self.script_editor.save(dir);
                    }
                    self.script_components
                        .insert(entity, ScriptComponent::rhai(&rel));
                    self.status = format!("Attached {rel}");
                    self.log("INFO", &format!("ScriptComponent → {rel}"), Color32::from_rgb(90, 200, 120));
                }
            });
            if let Some(script) = self.script_components.get(&entity) {
                ui.label(
                    RichText::new(format!("Bound: {}", script.path))
                        .small()
                        .color(Color32::from_rgb(140, 200, 255)),
                );
            }
        } else {
            ui.label(RichText::new("Select an entity in the Scene tree to attach.").weak().small());
        }
        ui.label(
            RichText::new("Play runs attached Scripts/*.rhai via RhaiHost (on_ready / on_update). Tab = autocomplete.")
                .weak()
                .small(),
        );
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
        // Borrowed from Unreal Engine: Details property search.
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut self.inspector_filter)
                    .desired_width(160.0)
                    .hint_text("Filter properties…"),
            );
        });
        ui.separator();

        let filt = self.inspector_filter.to_ascii_lowercase();
        let show = |key: &str| filt.is_empty() || key.contains(&filt);

        let is_light = name.contains("Light");
        let is_sky = name.contains("Sky") || name.contains("Fog");

        if show("transform") {
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
        }

        if is_light && show("light") {
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

        if (is_sky || is_light) && show("sky") {
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

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.content_drawer_open {
                self.content_drawer_open = false;
            } else if self.place_brush.is_some() {
                self.place_brush = None;
                self.status = "Place mode cancelled".into();
            }
        }

        // Borrowed from Unreal Engine: Ctrl+Space Content Browser drawer.
        if ctx.input(|i| {
            i.key_pressed(egui::Key::Space) && (i.modifiers.ctrl || i.modifiers.command)
        }) {
            self.content_drawer_open = !self.content_drawer_open;
        }

        // Borrowed from Unreal Engine: G toggles Game view (hide gizmos).
        if ctx.input(|i| i.key_pressed(egui::Key::G) && !i.modifiers.any()) {
            self.game_view = !self.game_view;
            self.status = if self.game_view {
                "Game view — gizmos hidden".into()
            } else {
                "Edit view".into()
            };
        }

        // Borrowed from Unreal / Godot: F focuses selection.
        if ctx.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.any()) {
            self.focus_selected();
        }

        // Borrowed from Godot 4: Ctrl+D duplicate.
        if ctx.input(|i| {
            i.key_pressed(egui::Key::D) && (i.modifiers.ctrl || i.modifiers.command)
        }) {
            self.duplicate_selection(glam::Vec3::new(1.0, 0.0, 0.0));
        }

        // Godot 3D + Unreal Modes shortcuts (exclusive bindings).
        if self.place_brush.is_none() {
            if ctx.input(|i| i.key_pressed(egui::Key::Q)) {
                self.viewport_tool = ViewportTool::Select;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::W)) {
                self.viewport_tool = ViewportTool::Move;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::E)) {
                self.viewport_tool = ViewportTool::Rotate;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.viewport_tool = ViewportTool::Scale;
            }
            // Borrowed from Unreal Engine: Shift+1..3 Modes (+4 RayAccurate).
            let shift = ctx.input(|i| i.modifiers.shift);
            if shift && ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
                self.viewport_tool = ViewportTool::Select;
            }
            if shift && ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
                self.viewport_tool = ViewportTool::Landscape;
            }
            if shift && ctx.input(|i| i.key_pressed(egui::Key::Num3)) {
                self.viewport_tool = ViewportTool::Foliage;
            }
            if shift && ctx.input(|i| i.key_pressed(egui::Key::Num4)) {
                self.viewport_tool = ViewportTool::RayAccurate;
            }
        }

        let dt = self.frame_start.elapsed().as_secs_f32().max(1e-4);
        self.frame_start = Instant::now();
        let fps = 1.0 / dt;
        self.fps_smooth = self.fps_smooth * 0.9 + fps * 0.1;

        if self.play.is_playing() {
            let cmds = self.play.tick(dt);
            self.apply_script_commands(cmds);
        }

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

        let tool_name = match self.viewport_tool {
            ViewportTool::Select => "Select",
            ViewportTool::Move => "Move",
            ViewportTool::Rotate => "Rotate",
            ViewportTool::Scale => "Scale",
            ViewportTool::Landscape => "Landscape",
            ViewportTool::Foliage => "Foliage",
            ViewportTool::RayAccurate => "RayAccurate",
        };
        let workspace = match self.workspace {
            WorkspaceMode::ThreeD => "3D",
            WorkspaceMode::Script => "Script",
            WorkspaceMode::Game => "Game",
        };
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Borrowed from EDITOR_UX: mode · tool · backend · FPS · hint.
                    ui.label(RichText::new(workspace).strong());
                    ui.label(RichText::new("|").weak());
                    ui.label(RichText::new(tool_name).color(Color32::from_rgb(120, 180, 255)));
                    if !self.status.is_empty() {
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
                        ui.label(
                            RichText::new(if self.grid_snap {
                                format!("Snap {:.1}", self.snap_size)
                            } else {
                                "Snap off".into()
                            })
                            .weak()
                            .small(),
                        );
                        ui.label(RichText::new("|").weak());
                        ui.label(RichText::new("Water v1").weak().small());
                        ui.label(RichText::new("|").weak());
                        ui.label(RichText::new("Lit").weak().small());
                        ui.label(RichText::new("|").weak());
                        ui.label(RichText::new("Vulkan").weak().small());
                    });
                });
            });

        // Borrowed from Godot 4: distraction-free hides side docks.
        if !self.distraction_free {
            egui::SidePanel::left("left_dock")
                .default_width(self.left_dock_width)
                .width_range(200.0..=400.0)
                .show(ctx, |ui| {
                    self.left_dock_width = ui.available_width();
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
                .default_width(self.right_dock_width)
                .width_range(240.0..=480.0)
                .show(ctx, |ui| {
                    self.right_dock_width = ui.available_width();
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.right_tab == RightTab::Inspector, "Inspector")
                            .clicked()
                        {
                            self.right_tab = RightTab::Inspector;
                        }
                        if ui
                            .selectable_label(self.right_tab == RightTab::Node, "Node Graph")
                            .clicked()
                        {
                            self.right_tab = RightTab::Node;
                        }
                    });
                    ui.separator();
                    match self.right_tab {
                        RightTab::Inspector => self.ui_inspector(ui),
                        RightTab::Node => self.ui_right_panel(ui),
                    }
                });
        }

        egui::TopBottomPanel::bottom("bottom_dock")
            .default_height(180.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (label, tab) in [
                        ("Assets", BottomTab::Assets),
                        ("Console", BottomTab::Console),
                        ("Script", BottomTab::Script),
                    ] {
                        if ui
                            .selectable_label(self.bottom_tab == tab, label)
                            .clicked()
                        {
                            self.bottom_tab = tab;
                        }
                    }
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Assets => self.ui_assets(ui),
                    BottomTab::Console => self.ui_console(ui),
                    BottomTab::Script => self.ui_script_dock(ui),
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui_viewport(ui);
        });

        self.ui_content_drawer(ctx);
        ctx.request_repaint();
    }
}
