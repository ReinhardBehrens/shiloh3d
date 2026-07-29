//! egui-based editor UI: hierarchy, inspector, toolbar, play mode.
//!
//! This is the *only* place in the workspace allowed to depend on `egui` /
//! `eframe` (see `docs/TECH_STACK.md`). Everything it touches — `Project`,
//! `Scene`, `Selection`, `PlaySession` — is plain Shiloh3D API, so a future
//! custom UI shell can replace this module without touching the rest of the
//! engine.

use std::path::PathBuf;

use ahash::AHashMap;
use eframe::egui;
use shiloh_ecs::Entity;
use shiloh_scene::{
    Camera, Scene, SceneFile, Transform, propagate_transforms, save_scene, set_parent,
};

use crate::play_mode::PlaySession;
use crate::project::Project;
use crate::selection::Selection;

/// Top-level egui application: owns the in-memory scene being edited.
pub struct EditorApp {
    pub project: Option<Project>,
    pub scene: Scene,
    pub selection: Selection,
    pub play: PlaySession,
    pub camera: Option<Camera>,
    pub status: String,
    /// Names captured from `EntityRecord` on load; entities created fresh in
    /// the editor fall back to `"Entity {index}"` in the hierarchy list.
    entity_names: AHashMap<Entity, String>,
    scene_path: Option<PathBuf>,
}

impl EditorApp {
    pub fn new(project: Option<Project>) -> Self {
        let scene_path = project
            .as_ref()
            .map(|p| p.root.join(&p.manifest.default_scene));

        let mut app = Self {
            project,
            scene: Scene::new("untitled"),
            selection: Selection::default(),
            play: PlaySession::default(),
            camera: Some(Camera::default()),
            status: "Ready".into(),
            entity_names: AHashMap::default(),
            scene_path,
        };

        if let Some(path) = app.scene_path.clone() {
            if path.exists() {
                app.load_scene_from(&path);
            } else {
                app.status = format!("New project — no scene at {}", path.display());
            }
        }
        app
    }

    fn hierarchy_label(&self, index: usize, entity: Entity) -> String {
        self.entity_names
            .get(&entity)
            .cloned()
            .unwrap_or_else(|| format!("Entity {index}"))
    }

    fn new_entity(&mut self) {
        let entity = self.scene.spawn_transform(Transform::default());
        propagate_transforms(&mut self.scene.world);
        self.selection.clear();
        self.selection.select(entity);
        self.status = "Created entity".into();
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
            }
            Err(err) => self.status = format!("Save failed: {err}"),
        }
    }

    /// Load a scene JSON file, tracking `EntityRecord` names for the
    /// hierarchy panel (spawns + parents manually so entity identities are
    /// known, unlike `SceneFile::apply_to_scene`).
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
    }

    fn toggle_play(&mut self) {
        if self.play.is_playing() {
            let restored_camera = self.play.exit_play(&mut self.scene);
            if restored_camera.is_some() {
                self.camera = restored_camera;
            }
            propagate_transforms(&mut self.scene.world);
            self.status = "Stopped — edit snapshot restored".into();
        } else {
            self.play.enter_play(&self.scene, self.camera.as_ref());
            self.status = "Playing (edits are simulated, Stop restores them)".into();
        }
    }

    fn ui_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let editing = !self.play.is_playing();
            ui.add_enabled_ui(editing, |ui| {
                if ui.button("New Entity").clicked() {
                    self.new_entity();
                }
                if ui.button("Save Scene").clicked() {
                    self.save_scene();
                }
                if ui.button("Load Scene").clicked() {
                    if let Some(path) = self.default_save_path() {
                        self.load_scene_from(&path);
                    } else {
                        self.status = "Cannot load: no project/scene path".into();
                    }
                }
            });

            ui.separator();

            let play_label = if self.play.is_playing() { "\u{25a0} Stop" } else { "\u{25b6} Play" };
            if ui.button(play_label).clicked() {
                self.toggle_play();
            }

            ui.separator();
            ui.label(if self.play.is_playing() { "PLAY MODE" } else { "Edit mode" });
        });
    }

    fn ui_hierarchy(&mut self, ui: &mut egui::Ui) {
        ui.heading("Hierarchy");
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut entities = Vec::new();
            self.scene.world.for_each::<Transform>(|e, _| entities.push(e));

            let mut clicked: Option<Entity> = None;
            for (index, entity) in entities.iter().enumerate() {
                let label = self.hierarchy_label(index, *entity);
                let is_selected = self.selection.entities.contains(entity);
                if ui.selectable_label(is_selected, label).clicked() {
                    clicked = Some(*entity);
                }
            }
            if let Some(entity) = clicked {
                self.selection.clear();
                self.selection.select(entity);
            }
        });
    }

    fn ui_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspector");
        ui.separator();

        let Some(&selected) = self.selection.entities.first() else {
            ui.label("No entity selected.");
            return;
        };

        let Some(transform) = self.scene.world.get_mut::<Transform>(selected) else {
            ui.label("Selected entity has no Transform.");
            return;
        };

        let mut changed = false;
        ui.label("Translation");
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::DragValue::new(&mut transform.translation.x).speed(0.05).prefix("x: "))
                .changed();
            changed |= ui
                .add(egui::DragValue::new(&mut transform.translation.y).speed(0.05).prefix("y: "))
                .changed();
            changed |= ui
                .add(egui::DragValue::new(&mut transform.translation.z).speed(0.05).prefix("z: "))
                .changed();
        });

        if changed {
            transform.mark_dirty();
            propagate_transforms(&mut self.scene.world);
        }
    }

    fn ui_viewport(&mut self, ui: &mut egui::Ui) {
        ui.heading("Viewport");
        ui.separator();
        ui.label(
            "Live 3D preview is not embedded yet — the SliceRenderer (wgpu) owns its own \
             window in shiloh-demo. Play mode still snapshots/restores scene state below.",
        );
        ui.add_space(8.0);
        ui.label(format!("Entities: {}", self.scene.world.entity_count()));
        if let Some(camera) = &self.camera {
            ui.label(format!(
                "Camera eye: ({:.2}, {:.2}, {:.2})",
                camera.eye.x, camera.eye.y, camera.eye.z
            ));
        }
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.ui_toolbar(ui);
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.label(&self.status);
        });

        egui::SidePanel::left("hierarchy_panel")
            .default_width(220.0)
            .show(ctx, |ui| {
                self.ui_hierarchy(ui);
            });

        egui::SidePanel::right("inspector_panel")
            .default_width(260.0)
            .show(ctx, |ui| {
                self.ui_inspector(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.ui_viewport(ui);
        });
    }
}
