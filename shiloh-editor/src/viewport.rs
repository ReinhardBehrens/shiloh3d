//! Live 3D viewport — offscreen `SliceRenderer` → egui texture.
//!
//! Uses CPU readback so the editor can keep its glow egui stack while the
//! slice path stays on wgpu 25 (egui-wgpu 0.31 still pins wgpu 24).
//!
//! # Godot borrowings
//! Interaction and gizmo behaviour below deliberately mirrors Godot 4's 3D
//! editor (`EditorPlugin::_forward_3d_gui_input`, `Node3D` transform gizmo,
//! `Camera3D.project_ray_*` + plane/AABB pick). Comments call out each borrow
//! so we can swap in engine-native physics/mesh picking later without losing
//! the UX contract. Blackmarsh ARPG shot = complexity bar; FirstGoal mockup =
//! editor/world presentation bar for this viewport.

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions, Vec2};
use glam::{Mat4, Quat, Vec3, Vec4};
use shiloh_assets::load_gltf;
use shiloh_ecs::Entity;
use shiloh_render::{SliceDrawParams, SliceRenderer, orthographic_light_matrix};
use shiloh_scene::{Camera, GlobalTransform, ProjectionKind, Scene, Transform};

use crate::gltf_mesh::to_slice_mesh;
use crate::selection::SelectMode;

/// Events produced by viewport interaction — handled by [`EditorApp`](crate::ui::EditorApp).
#[derive(Debug, Clone)]
pub enum ViewportEvent {
    Select {
        entity: Entity,
        mode: SelectMode,
    },
    ClearSelection,
    Translate {
        delta: Vec3,
    },
    /// Asset-palette place mode: primary click hit the ground plane.
    PlaceAt {
        world: Vec3,
    },
    /// Place brush cancelled because the user picked an existing node.
    ExitPlaceMode,
}

/// Viewport transform tool — Godot 3D toolbar Select / Move (Rotate/Scale later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportTool {
    #[default]
    Select,
    Move,
}

/// One pickable scene node — Godot-style AABB hit (not origin-only).
#[derive(Clone, Copy)]
struct Pickable {
    entity: Entity,
    /// World AABB center (origin lifted by half-height so boxes sit on ground).
    center: Vec3,
    half: Vec3,
}

/// Orbit / pan / move state for the scene viewport.
pub struct ViewportCamera {
    pub focus: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    dragging_orbit: bool,
    dragging_pan: bool,
    dragging_move: bool,
    last_pointer: Option<egui::Pos2>,
    drag_start: Option<egui::Pos2>,
}

impl Default for ViewportCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::new(0.0, 0.8, 0.0),
            distance: 22.0,
            yaw: 35f32.to_radians(),
            pitch: 32f32.to_radians(),
            dragging_orbit: false,
            dragging_pan: false,
            dragging_move: false,
            last_pointer: None,
            drag_start: None,
        }
    }
}

impl ViewportCamera {
    pub fn to_camera(&self, aspect: f32) -> Camera {
        let eye = self.focus
            + Vec3::new(
                self.distance * self.yaw.cos() * self.pitch.cos(),
                self.distance * self.pitch.sin(),
                self.distance * self.yaw.sin() * self.pitch.cos(),
            );
        let mut cam = Camera::isometric(self.focus, self.distance);
        cam.eye = eye;
        cam.target = self.focus;
        cam.aspect = aspect.max(0.01);
        cam.projection = ProjectionKind::Perspective;
        cam.fov_y_radians = 52f32.to_radians();
        cam
    }

    fn xz_delta_from_screen(&self, delta: Vec2) -> Vec3 {
        let right = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());
        let forward = Vec3::new(-self.yaw.cos(), 0.0, -self.yaw.sin());
        let scale = self.distance * 0.003;
        right * (delta.x * scale) + forward * (-delta.y * scale)
    }

    /// Godot `EditorPlugin::_forward_3d_gui_input` analogue.
    ///
    /// Contract (Godot 4 3D editor defaults):
    /// - LMB click empty → place (brush) or clear selection
    /// - LMB click node → select (Shift add / Ctrl toggle)
    /// - LMB drag on selection / Move tool / `G` → translate on ground plane
    /// - MMB drag → orbit · RMB drag → pan · scroll → zoom
    /// - Never orbit on LMB (that made place/select feel broken)
    fn handle_input(
        &mut self,
        resp: &egui::Response,
        ui: &egui::Ui,
        selection: &[Entity],
        pickables: &[Pickable],
        camera: &Camera,
        rect: egui::Rect,
        place_mode: bool,
        tool: ViewportTool,
    ) -> Option<ViewportEvent> {
        let g_key = ui.input(|i| i.key_down(egui::Key::G));
        let mut event = None;

        // --- Camera navigation (Godot: MMB orbit, RMB pan) ---
        if resp.drag_started_by(egui::PointerButton::Secondary) {
            self.dragging_pan = true;
            self.last_pointer = resp.interact_pointer_pos();
            self.drag_start = self.last_pointer;
        }
        if resp.drag_started_by(egui::PointerButton::Middle) {
            self.dragging_orbit = true;
            self.last_pointer = resp.interact_pointer_pos();
            self.drag_start = self.last_pointer;
        }

        // --- Translate drag (Godot: G grab / Move tool / drag near selection) ---
        if resp.drag_started_by(egui::PointerButton::Primary) {
            self.last_pointer = resp.interact_pointer_pos();
            self.drag_start = self.last_pointer;
            let near_selected = !place_mode
                && self.last_pointer.is_some_and(|p| {
                    // Godot Move tool: wider grab around selection / gizmo.
                    if tool == ViewportTool::Move {
                        pointer_near_selection(p, selection, pickables, camera, rect)
                            || pick_entity_aabb(p, pickables, camera, rect)
                                .is_some_and(|e| selection.iter().any(|&s| s == e))
                    } else {
                        pointer_near_selection(p, selection, pickables, camera, rect)
                    }
                });
            if !place_mode && (g_key || near_selected) {
                self.dragging_move = true;
            }
        }

        // --- Click select / place (Godot short-click; must NOT wait for drag_stopped) ---
        // egui: Sense::click_and_drag sets `clicked` on short clicks and never
        // starts a drag — the old drag_stopped path never fired PlaceAt/Select.
        if resp.clicked_by(egui::PointerButton::Primary) && !self.dragging_move {
            if let Some(pos) = resp.interact_pointer_pos() {
                if place_mode {
                    // Prefer pick → exit brush (Godot: click existing node while placing).
                    if let Some(entity) = pick_entity_aabb(pos, pickables, camera, rect) {
                        event = Some(ViewportEvent::Select {
                            entity,
                            mode: select_mode_from_input(ui),
                        });
                        // Also signal brush clear — handled as ExitPlaceMode after Select
                        // by emitting ExitPlaceMode when we want both; ui clears on Select.
                    } else if let Some(world) = ray_hit_ground(pos, camera, rect) {
                        event = Some(ViewportEvent::PlaceAt { world });
                    }
                } else if let Some(entity) = pick_entity_aabb(pos, pickables, camera, rect) {
                    event = Some(ViewportEvent::Select {
                        entity,
                        mode: select_mode_from_input(ui),
                    });
                } else if !ui.input(|i| i.modifiers.shift || i.modifiers.ctrl || i.modifiers.command)
                {
                    event = Some(ViewportEvent::ClearSelection);
                }
            }
        }

        if resp.drag_stopped() {
            self.dragging_orbit = false;
            self.dragging_pan = false;
            self.dragging_move = false;
            self.last_pointer = None;
            self.drag_start = None;
        }

        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(prev) = self.last_pointer {
                let delta = pos - prev;
                if self.dragging_move {
                    let world_delta = self.xz_delta_from_screen(delta);
                    if world_delta.length_squared() > 1e-8 {
                        event = Some(ViewportEvent::Translate { delta: world_delta });
                    }
                } else if self.dragging_orbit {
                    self.yaw += delta.x * 0.005;
                    self.pitch = (self.pitch + delta.y * 0.005).clamp(0.08, 1.45);
                } else if self.dragging_pan {
                    let right = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());
                    let up = Vec3::Y;
                    let scale = self.distance * 0.0025;
                    self.focus += right * (-delta.x * scale) + up * (delta.y * scale);
                }
            }
            if self.dragging_orbit || self.dragging_pan || self.dragging_move {
                self.last_pointer = Some(pos);
            }
        }

        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if resp.hovered() && scroll.abs() > 0.0 {
            self.distance = (self.distance * (1.0 - scroll * 0.0015)).clamp(4.0, 90.0);
        }

        event
    }
}

const PICK_RADIUS_PX: f32 = 14.0;

fn select_mode_from_input(ui: &egui::Ui) -> SelectMode {
    ui.input(|i| {
        if i.modifiers.shift {
            SelectMode::Add
        } else if i.modifiers.command || i.modifiers.ctrl {
            SelectMode::Toggle
        } else {
            SelectMode::Replace
        }
    })
}

fn pointer_near_selection(
    pointer: egui::Pos2,
    selection: &[Entity],
    pickables: &[Pickable],
    camera: &Camera,
    rect: egui::Rect,
) -> bool {
    selection.iter().any(|&sel| {
        pickables.iter().any(|p| {
            p.entity == sel && screen_dist_to_aabb(pointer, *p, camera, rect) <= PICK_RADIUS_PX * 2.5
        })
    })
}

/// Godot-ish pick: closest projected AABB (screen distance), not origin radius alone.
fn pick_entity_aabb(
    pointer: egui::Pos2,
    pickables: &[Pickable],
    camera: &Camera,
    rect: egui::Rect,
) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;
    for &p in pickables {
        let dist = screen_dist_to_aabb(pointer, p, camera, rect);
        let threshold = PICK_RADIUS_PX.max(projected_aabb_radius(p, camera, rect) * 0.55);
        if dist <= threshold && best.map_or(true, |(_, d)| dist < d) {
            best = Some((p.entity, dist));
        }
    }
    best.map(|(e, _)| e)
}

fn screen_dist_to_aabb(pointer: egui::Pos2, p: Pickable, camera: &Camera, rect: egui::Rect) -> f32 {
    if let Some(screen) = project_to_viewport(p.center, camera, rect) {
        let d = (screen - pointer).length();
        // Also accept hits near any projected corner (tall trees / cliffs).
        let mut best = d;
        for c in aabb_corners(p.center, p.half) {
            if let Some(sc) = project_to_viewport(c, camera, rect) {
                best = best.min((sc - pointer).length());
            }
        }
        best
    } else {
        f32::MAX
    }
}

fn projected_aabb_radius(p: Pickable, camera: &Camera, rect: egui::Rect) -> f32 {
    let Some(c) = project_to_viewport(p.center, camera, rect) else {
        return PICK_RADIUS_PX;
    };
    let mut r = 0.0_f32;
    for corner in aabb_corners(p.center, p.half) {
        if let Some(sc) = project_to_viewport(corner, camera, rect) {
            r = r.max((sc - c).length());
        }
    }
    r.max(PICK_RADIUS_PX)
}

/// CC0 prop slot availability (shrub, fern, rock_09, rock_06).
#[derive(Clone, Copy, Default)]
struct PropSlots {
    loaded: [bool; 4],
}

impl PropSlots {
    /// Only map names that intentionally use CC0 photogrammetry slots.
    /// Pines/birches stay on stylized foliage mesh (FirstGoal valley read) —
    /// do not remesh them as shrub_03 (that made the forest look like rocks).
    fn foliage_slot(&self, name: &str) -> Option<usize> {
        let lower = name.to_ascii_lowercase();
        if lower.contains("fern_02") || lower.contains("grass_patch") || lower.contains("grass") {
            return self.loaded[1].then_some(1);
        }
        if lower.contains("shrub_03") || lower.contains("shrub") {
            return self.loaded[0].then_some(0);
        }
        None
    }

    fn rock_slot(&self, name: &str) -> Option<usize> {
        let lower = name.to_ascii_lowercase();
        if lower.contains("rock_09") || lower.contains("rock_large") {
            return self.loaded[2].then_some(2);
        }
        if lower.contains("rock_06") || lower.contains("rock_scatter") {
            return self.loaded[3].then_some(3);
        }
        if lower.contains("rock") || lower.contains("cliff") {
            if lower.contains("large") && self.loaded[2] {
                Some(2)
            } else if self.loaded[3] {
                Some(3)
            } else if self.loaded[2] {
                Some(2)
            } else {
                None
            }
        } else {
            None
        }
    }
}

fn prop_gltf_paths() -> [(usize, &'static str); 4] {
    [
        (0, "shrub_03/shrub_03_1k.gltf"),
        (1, "fern_02/fern_02_1k.gltf"),
        (2, "rock_09/rock_09_1k.gltf"),
        (3, "rock_06/rock_06_1k.gltf"),
    ]
}

fn load_prop_meshes(renderer: &mut SliceRenderer) -> PropSlots {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/props");
    let mut slots = PropSlots::default();
    for (slot, rel) in prop_gltf_paths() {
        let path = base.join(rel);
        if !path.exists() {
            continue;
        }
        match load_gltf(&path) {
            Ok(gltf) => {
                if let Some(prim) = gltf.primitives.first() {
                    renderer.set_prop_mesh(slot, &to_slice_mesh(prim));
                    slots.loaded[slot] = true;
                }
            }
            Err(err) => {
                tracing::warn!("prop load failed {}: {err}", path.display());
            }
        }
    }
    slots
}

fn entity_half_extents(name: &str, scale: Vec3) -> Vec3 {
    let lower = name.to_ascii_lowercase();
    let base = if lower.contains("light") {
        Vec3::splat(0.25)
    } else if lower.contains("pine") || lower.contains("birch") || lower.contains("tree") {
        Vec3::new(0.85, 2.4, 0.85)
    } else if lower.contains("grass") || lower.contains("fern") {
        Vec3::new(0.45, 0.4, 0.45)
    } else if lower.contains("shrub") {
        Vec3::new(0.7, 0.9, 0.7)
    } else if lower.contains("rock") || lower.contains("cliff") {
        Vec3::new(1.2, 0.75, 1.2)
    } else if lower.contains("mountain") {
        Vec3::new(8.0, 6.0, 5.0)
    } else if lower.contains("terrain") || lower.contains("heightmap") {
        Vec3::new(16.0, 0.2, 12.0)
    } else {
        Vec3::splat(0.5)
    };
    base * scale
}

/// Lift AABB so planted props sit on the ground plane (Godot AABB for Node3D).
fn aabb_center_on_ground(origin: Vec3, half: Vec3) -> Vec3 {
    origin + Vec3::new(0.0, half.y, 0.0)
}

fn aabb_corners(center: Vec3, half: Vec3) -> [Vec3; 8] {
    [
        center + Vec3::new(-half.x, -half.y, -half.z),
        center + Vec3::new(half.x, -half.y, -half.z),
        center + Vec3::new(half.x, -half.y, half.z),
        center + Vec3::new(-half.x, -half.y, half.z),
        center + Vec3::new(-half.x, half.y, -half.z),
        center + Vec3::new(half.x, half.y, -half.z),
        center + Vec3::new(half.x, half.y, half.z),
        center + Vec3::new(-half.x, half.y, half.z),
    ]
}

const AABB_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

fn draw_aabb_wireframe(
    painter: &egui::Painter,
    center: Vec3,
    half: Vec3,
    camera: &Camera,
    rect: egui::Rect,
    color: Color32,
) {
    let corners = aabb_corners(center, half);
    let stroke = egui::Stroke::new(1.5_f32, color);
    for (a, b) in AABB_EDGES {
        if let (Some(sa), Some(sb)) = (
            project_to_viewport(corners[a], camera, rect),
            project_to_viewport(corners[b], camera, rect),
        ) {
            painter.line_segment([sa, sb], stroke);
        }
    }
}

/// Owns the GPU slice renderer and the egui texture that displays it.
pub struct SceneViewport {
    renderer: Option<SliceRenderer>,
    texture: Option<TextureHandle>,
    pub cam: ViewportCamera,
    last_size: (u32, u32),
    boot_error: Option<String>,
    time: Instant,
    max_dim: u32,
    props: PropSlots,
}

impl SceneViewport {
    pub fn new() -> Self {
        let mut vp = Self {
            renderer: None,
            texture: None,
            cam: ViewportCamera::default(),
            last_size: (0, 0),
            boot_error: None,
            time: Instant::now(),
            max_dim: 1280,
            props: PropSlots::default(),
        };
        match pollster::block_on(SliceRenderer::new_offscreen(640, 360)) {
            Ok(mut r) => {
                vp.props = load_prop_meshes(&mut r);
                vp.renderer = Some(r);
                vp.last_size = (640, 360);
            }
            Err(err) => {
                vp.boot_error = Some(format!("3D viewport GPU init failed: {err}"));
            }
        }
        vp
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        scene: &Scene,
        entity_names: &ahash::AHashMap<Entity, String>,
        selection: &[Entity],
        fps: f32,
        fog_enabled: bool,
        place_brush_name: Option<&str>,
        tool: &mut ViewportTool,
    ) -> Vec<ViewportEvent> {
        let mut events = Vec::new();
        let available = ui.available_size();
        let (rect, resp) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());

        if let Some(err) = &self.boot_error {
            ui.painter_at(rect)
                .rect_filled(rect, 0.0, Color32::from_rgb(28, 20, 20));
            ui.painter_at(rect).text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                err,
                egui::FontId::proportional(14.0),
                Color32::LIGHT_RED,
            );
            return events;
        }

        let pixel = ui.ctx().pixels_per_point();
        let mut w = (rect.width() * pixel).round().max(1.0) as u32;
        let mut h = (rect.height() * pixel).round().max(1.0) as u32;
        let longest = w.max(h);
        if longest > self.max_dim {
            let scale = self.max_dim as f32 / longest as f32;
            w = ((w as f32) * scale).round().max(1.0) as u32;
            h = ((h as f32) * scale).round().max(1.0) as u32;
        }

        let aspect = w as f32 / h as f32;
        let camera = self.cam.to_camera(aspect);
        let t = self.time.elapsed().as_secs_f32();

        let mut cube_mats = Vec::new();
        let mut sphere_mats = Vec::new();
        let mut foliage_mats = Vec::new();
        let mut rock_mats = Vec::new();
        let mut mountain_mats = Vec::new();
        let mut ground_mats = Vec::new();
        let mut prop_mats: [Vec<Mat4>; 4] = Default::default();
        let mut pickables: Vec<Pickable> = Vec::new();
        let mut water = false;

        // Base terrain plate — FirstGoal valley ground read (wider, slightly thicker).
        ground_mats.push(Mat4::from_scale_rotation_translation(
            Vec3::new(56.0, 0.12, 42.0),
            Quat::IDENTITY,
            Vec3::new(0.0, -0.06, 0.0),
        ));

        let mut entities = Vec::new();
        scene.world.for_each::<Transform>(|e, _| entities.push(e));

        for entity in entities {
            let Some(local) = scene.world.get::<Transform>(entity) else {
                continue;
            };
            let world = scene
                .world
                .get::<GlobalTransform>(entity)
                .map(|g| g.0)
                .unwrap_or_else(|| {
                    Mat4::from_scale_rotation_translation(
                        local.scale,
                        local.rotation,
                        local.translation,
                    )
                });
            let (scale, _r, pos) = world.to_scale_rotation_translation();

            let name = entity_names
                .get(&entity)
                .map(|s| s.as_str())
                .unwrap_or("");
            let lower = name.to_ascii_lowercase();

            if !lower.contains("sky") && !lower.contains("fog") && !lower.contains("terrain") {
                let half = entity_half_extents(name, scale);
                pickables.push(Pickable {
                    entity,
                    center: aabb_center_on_ground(pos, half),
                    half,
                });
            }

            if lower.contains("water") {
                water = true;
                continue;
            }
            if lower.contains("light") || lower.contains("fog") || lower.contains("sky") {
                sphere_mats.push(Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.35),
                    Quat::IDENTITY,
                    pos,
                ));
                continue;
            }
            if lower.contains("mountain") {
                mountain_mats.push(Mat4::from_scale_rotation_translation(
                    Vec3::new(18.0, 14.0, 8.0) * scale,
                    local.rotation,
                    pos,
                ));
                continue;
            }
            if lower.contains("terrain") || lower.contains("heightmap") {
                ground_mats.push(Mat4::from_scale_rotation_translation(
                    Vec3::new(40.0, 0.18, 30.0) * scale,
                    local.rotation,
                    pos,
                ));
                continue;
            }
            if lower.contains("cliff") {
                if let Some(slot) = self.props.rock_slot(name) {
                    prop_mats[slot].push(Mat4::from_scale_rotation_translation(
                        scale * 2.5,
                        local.rotation,
                        pos,
                    ));
                } else {
                    rock_mats.push(Mat4::from_scale_rotation_translation(
                        Vec3::new(3.5, 8.0, 1.2) * scale,
                        local.rotation,
                        pos,
                    ));
                }
                continue;
            }
            if lower.contains("rock") {
                if let Some(slot) = self.props.rock_slot(name) {
                    let s = if lower.contains("large") || lower.contains("09") {
                        scale * 1.35
                    } else {
                        scale * 0.85
                    };
                    prop_mats[slot].push(Mat4::from_scale_rotation_translation(
                        s,
                        local.rotation,
                        pos,
                    ));
                } else {
                    let s = if lower.contains("large") {
                        Vec3::new(2.2, 1.6, 2.0)
                    } else {
                        Vec3::new(0.9, 0.7, 0.85)
                    };
                    rock_mats.push(Mat4::from_scale_rotation_translation(
                        s * scale,
                        local.rotation,
                        pos,
                    ));
                }
                continue;
            }
            if lower.contains("pine")
                || lower.contains("birch")
                || lower.contains("tree")
                || lower.contains("grass")
                || lower.contains("dead")
                || lower.contains("shrub")
                || lower.contains("fern")
            {
                if let Some(slot) = self.props.foliage_slot(name) {
                    prop_mats[slot].push(Mat4::from_scale_rotation_translation(
                        scale,
                        local.rotation,
                        pos,
                    ));
                } else {
                    let h = if lower.contains("cluster") {
                        2.8
                    } else if lower.contains("tall") || lower.contains("pine") {
                        4.2
                    } else if lower.contains("birch") {
                        3.2
                    } else if lower.contains("dead") {
                        2.4
                    } else {
                        1.6
                    };
                    foliage_mats.push(Mat4::from_scale_rotation_translation(
                        Vec3::new(1.25, h, 1.25) * scale,
                        local.rotation,
                        pos,
                    ));
                }
                continue;
            }
            cube_mats.push(world);
        }

        // Always draw water when a WaterBody exists — FirstGoal valley river read.

        if let Some(ev) = self.cam.handle_input(
            &resp,
            ui,
            selection,
            &pickables,
            &camera,
            rect,
            place_brush_name.is_some(),
            *tool,
        ) {
            // Selecting while placing exits the brush (Godot paint-tool behaviour).
            if place_brush_name.is_some() {
                if matches!(ev, ViewportEvent::Select { .. }) {
                    events.push(ViewportEvent::ExitPlaceMode);
                }
            }
            events.push(ev);
        }

        // Do not force water when the scene is empty of meshes — WaterBody sets `water`.

        if let Some(renderer) = self.renderer.as_mut() {
            if (w, h) != self.last_size {
                renderer.resize(w, h);
                self.last_size = (w, h);
            }

            let sun_dir = Vec3::new(-0.55, -0.85, -0.25).normalize();
            let light_view_proj =
                orthographic_light_matrix(sun_dir, self.cam.focus, 48.0, 1.0, 110.0);

            // FirstGoal mockup lighting: warm key sun, cool fill, soft fog.
            let fog_density = if fog_enabled { 0.008 } else { 0.0 };

            if let Err(err) = renderer.render(SliceDrawParams {
                view_proj: camera.view_proj(),
                camera_pos: camera.eye,
                time: t,
                sun_dir,
                sun_color: Vec3::new(1.0, 0.92, 0.75) * 1.35,
                ambient: Vec3::new(0.14, 0.16, 0.20),
                fog_color: Vec3::new(0.55, 0.68, 0.78),
                fog_density,
                light_view_proj,
                point0_pos: Vec3::new(6.0, 4.0, 3.0),
                point0_range: 22.0,
                point0_color: Vec3::new(0.55, 0.72, 1.0) * 0.85,
                point1_pos: Vec3::new(-7.0, 3.0, -2.0),
                point1_range: 18.0,
                point1_color: Vec3::new(1.0, 0.55, 0.35) * 0.55,
                spot_pos: Vec3::new(0.0, 10.0, 2.0),
                spot_range: 28.0,
                spot_dir: Vec3::new(0.1, -1.0, 0.15).normalize(),
                spot_inner_cos: 0.92,
                spot_outer_cos: 0.72,
                spot_color: Vec3::new(1.0, 0.95, 0.8) * 1.4,
                exposure: 1.05,
                contrast: 1.15,
                saturation: 1.3,
                cube_instances: &cube_mats,
                sphere_instances: &sphere_mats,
                extra_instances: &[],
                foliage_instances: &foliage_mats,
                rock_instances: &rock_mats,
                mountain_instances: &mountain_mats,
                ground_instances: &ground_mats,
                prop0_instances: &prop_mats[0],
                prop1_instances: &prop_mats[1],
                prop2_instances: &prop_mats[2],
                prop3_instances: &prop_mats[3],
                skinned_model: None,
                skin_joints: &[],
                hud_verts: &[],
                draw_water: water,
                screenshot_path: None,
            }) {
                self.boot_error = Some(format!("viewport render failed: {err}"));
                return events;
            }

            match renderer.read_rgba8() {
                Ok((rw, rh, rgba)) => {
                    let image =
                        ColorImage::from_rgba_unmultiplied([rw as usize, rh as usize], &rgba);
                    if let Some(tex) = &mut self.texture {
                        tex.set(image, TextureOptions::LINEAR);
                    } else {
                        self.texture = Some(ui.ctx().load_texture(
                            "shiloh_viewport",
                            image,
                            TextureOptions::LINEAR,
                        ));
                    }
                }
                Err(err) => {
                    self.boot_error = Some(format!("viewport readback failed: {err}"));
                    return events;
                }
            }
        }

        if let Some(tex) = &self.texture {
            ui.painter_at(rect).image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        let painter = ui.painter_at(rect);
        let multi = selection.len() > 1;
        let wire_color = if multi {
            Color32::from_rgb(80, 220, 230)
        } else {
            Color32::from_rgb(255, 180, 60)
        };
        for &sel in selection {
            let Some(name) = entity_names.get(&sel).map(|s| s.as_str()) else {
                continue;
            };
            let Some(local) = scene.world.get::<Transform>(sel) else {
                continue;
            };
            let pos = scene
                .world
                .get::<GlobalTransform>(sel)
                .map(|g| g.0.w_axis.truncate())
                .unwrap_or(local.translation);
            let half = entity_half_extents(name, local.scale);
            let center = aabb_center_on_ground(pos, half);
            // Godot Node3D selection AABB (wireframe).
            draw_aabb_wireframe(&painter, center, half, &camera, rect, wire_color);
            // Godot TranslationGizmo — RGB axes at selection origin.
            if *tool == ViewportTool::Move || selection.len() == 1 {
                draw_move_gizmo(&painter, pos, &camera, rect);
            }
        }

        // Godot place-preview: ghost ring on ground under cursor while brushing.
        if place_brush_name.is_some() {
            if let Some(pointer) = resp.hover_pos() {
                if let Some(hit) = ray_hit_ground(pointer, &camera, rect) {
                    draw_place_ghost(&painter, hit, &camera, rect);
                }
            }
        }

        draw_tool_strip(ui, rect, tool);
        draw_viewport_chrome(&painter, rect, fps, place_brush_name, *tool);
        ui.ctx().request_repaint();
        events
    }
}

/// Godot 3D toolbar Select / Move (Rotate · Scale · Snap later).
fn draw_tool_strip(ui: &mut egui::Ui, rect: egui::Rect, tool: &mut ViewportTool) {
    let strip = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + 20.0),
        Vec2::new(280.0, 26.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (label, value) in [
                ("Select", ViewportTool::Select),
                ("Move", ViewportTool::Move),
            ] {
                let selected = *tool == value;
                if ui
                    .selectable_label(selected, rich_text_tool(label, selected))
                    .clicked()
                {
                    *tool = value;
                }
            }
            ui.add_enabled(false, egui::Label::new(
                egui::RichText::new("Rotate  Scale  Snap").weak().small(),
            ));
        });
    });
}

fn rich_text_tool(label: &str, selected: bool) -> egui::RichText {
    let t = egui::RichText::new(label).small();
    if selected {
        t.strong().color(Color32::from_rgb(120, 180, 255))
    } else {
        t.color(Color32::from_rgb(170, 178, 195))
    }
}

/// Godot `EditorNode3DGizmo` translation axes (screen-projected).
fn draw_move_gizmo(painter: &egui::Painter, origin: Vec3, camera: &Camera, rect: egui::Rect) {
    let Some(o) = project_to_viewport(origin, camera, rect) else {
        return;
    };
    let axis_len = 1.6_f32;
    let axes = [
        (Vec3::X * axis_len, Color32::from_rgb(230, 70, 70)),
        (Vec3::Y * axis_len, Color32::from_rgb(80, 210, 90)),
        (Vec3::Z * axis_len, Color32::from_rgb(70, 130, 240)),
    ];
    for (delta, color) in axes {
        if let Some(tip) = project_to_viewport(origin + delta, camera, rect) {
            painter.line_segment([o, tip], egui::Stroke::new(2.5_f32, color));
            painter.circle_filled(tip, 4.0, color);
        }
    }
    painter.circle_filled(o, 5.0, Color32::from_rgb(240, 240, 245));
}

fn draw_place_ghost(painter: &egui::Painter, hit: Vec3, camera: &Camera, rect: egui::Rect) {
    let Some(c) = project_to_viewport(hit, camera, rect) else {
        return;
    };
    let r = 10.0;
    painter.circle_stroke(
        c,
        r,
        egui::Stroke::new(2.0_f32, Color32::from_rgb(90, 220, 130)),
    );
    painter.circle_filled(c, 3.0, Color32::from_rgb(90, 220, 130));
    // Crosshair on XZ.
    for d in [
        Vec3::new(0.6, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.6),
    ] {
        if let (Some(a), Some(b)) = (
            project_to_viewport(hit - d, camera, rect),
            project_to_viewport(hit + d, camera, rect),
        ) {
            painter.line_segment(
                [a, b],
                egui::Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(90, 220, 130, 160)),
            );
        }
    }
}

fn draw_viewport_chrome(
    painter: &egui::Painter,
    rect: egui::Rect,
    fps: f32,
    place_brush_name: Option<&str>,
    tool: ViewportTool,
) {
    let bar_bg = Color32::from_rgba_unmultiplied(16, 18, 24, 190);
    let text = Color32::from_rgb(220, 225, 235);
    let weak = Color32::from_rgb(170, 178, 195);

    let tl = rect.left_top() + Vec2::new(8.0, 8.0);
    let mode_rect = egui::Rect::from_min_size(tl, Vec2::new(220.0, 26.0));
    painter.rect_filled(mode_rect, 4.0, bar_bg);
    painter.text(
        tl + Vec2::new(10.0, 5.0),
        egui::Align2::LEFT_TOP,
        "Perspective ▾   Lit ▾",
        egui::FontId::proportional(12.0),
        text,
    );

    painter.text(
        rect.right_bottom() + Vec2::new(-12.0, -10.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.0} FPS", fps),
        egui::FontId::monospace(12.0),
        text,
    );

    let hint = if let Some(name) = place_brush_name {
        format!("Place: {name} · LMB ground · click object to select · Esc cancel · MMB orbit")
    } else if tool == ViewportTool::Move {
        "Move · drag near gizmo/selection · G grab · MMB orbit · RMB pan".into()
    } else {
        "Select · LMB pick · drag selection to move · MMB orbit · RMB pan · scroll zoom".into()
    };
    painter.text(
        rect.left_bottom() + Vec2::new(12.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        hint,
        egui::FontId::proportional(11.0),
        if place_brush_name.is_some() {
            Color32::from_rgb(90, 200, 120)
        } else {
            weak
        },
    );
}

/// Unproject a viewport pointer to the y=0 ground plane (terrain proxy).
fn ray_hit_ground(pointer: egui::Pos2, camera: &Camera, rect: egui::Rect) -> Option<Vec3> {
    if rect.width() < 1.0 || rect.height() < 1.0 {
        return None;
    }
    let ndc_x = ((pointer.x - rect.left()) / rect.width()) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((pointer.y - rect.top()) / rect.height()) * 2.0;
    let inv = camera.view_proj().inverse();
    // glam `perspective_rh` uses Vulkan-style depth in [0, 1].
    let near_h = inv * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_h = inv * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    if near_h.w.abs() < 1e-8 || far_h.w.abs() < 1e-8 {
        return None;
    }
    let near = near_h.truncate() / near_h.w;
    let far = far_h.truncate() / far_h.w;
    let dir = far - near;
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let t = -near.y / dir.y;
    if t < 0.0 {
        return None;
    }
    let hit = near + dir * t;
    if hit.x.abs() > 200.0 || hit.z.abs() > 200.0 {
        return None;
    }
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

fn project_to_viewport(world: Vec3, camera: &Camera, rect: egui::Rect) -> Option<egui::Pos2> {
    let clip = camera.view_proj() * Vec4::new(world.x, world.y, world.z, 1.0);
    if clip.w.abs() < 1e-5 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.z < -1.0 || ndc.z > 1.0 {
        return None;
    }
    let x = rect.left() + (ndc.x * 0.5 + 0.5) * rect.width();
    let y = rect.top() + (1.0 - (ndc.y * 0.5 + 0.5)) * rect.height();
    Some(egui::pos2(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn offscreen_slice_renders_rgba() {
        let mut renderer = pollster::block_on(SliceRenderer::new_offscreen(64, 48))
            .expect("offscreen GPU");
        let cam = Camera::isometric(Vec3::ZERO, 12.0);
        let cube = Mat4::IDENTITY;
        renderer
            .render(SliceDrawParams {
                view_proj: cam.view_proj(),
                camera_pos: cam.eye,
                time: 0.0,
                sun_dir: Vec3::new(-0.4, -1.0, -0.3).normalize(),
                sun_color: Vec3::ONE,
                ambient: Vec3::splat(0.2),
                fog_color: Vec3::ZERO,
                fog_density: 0.0,
                light_view_proj: orthographic_light_matrix(
                    Vec3::new(-0.4, -1.0, -0.3).normalize(),
                    Vec3::ZERO,
                    20.0,
                    1.0,
                    50.0,
                ),
                point0_pos: Vec3::ZERO,
                point0_range: 1.0,
                point0_color: Vec3::ZERO,
                point1_pos: Vec3::ZERO,
                point1_range: 1.0,
                point1_color: Vec3::ZERO,
                spot_pos: Vec3::ZERO,
                spot_range: 1.0,
                spot_dir: -Vec3::Y,
                spot_inner_cos: 1.0,
                spot_outer_cos: 0.9,
                spot_color: Vec3::ZERO,
                exposure: 1.0,
                contrast: 1.0,
                saturation: 1.0,
                cube_instances: &[cube],
                sphere_instances: &[],
                extra_instances: &[],
                foliage_instances: &[],
                rock_instances: &[],
                mountain_instances: &[],
                ground_instances: &[],
                prop0_instances: &[],
                prop1_instances: &[],
                prop2_instances: &[],
                prop3_instances: &[],
                skinned_model: None,
                skin_joints: &[],
                hud_verts: &[],
                draw_water: true,
                screenshot_path: None,
            })
            .expect("render");
        let (w, h, rgba) = renderer.read_rgba8().expect("readback");
        assert_eq!((w, h), (64, 48));
        assert_eq!(rgba.len(), (64 * 48 * 4) as usize);
        assert!(rgba.iter().any(|&b| b > 8));
    }

    #[test]
    fn foliage_instances_render_non_empty() {
        let mut renderer = pollster::block_on(SliceRenderer::new_offscreen(64, 48))
            .expect("offscreen GPU");
        let cam = Camera::isometric(Vec3::new(0.0, 1.0, 0.0), 14.0);
        let foliage = Mat4::from_scale_rotation_translation(
            Vec3::new(0.5, 2.0, 0.5),
            Quat::IDENTITY,
            Vec3::new(0.0, 1.0, 0.0),
        );
        renderer
            .render(SliceDrawParams {
                view_proj: cam.view_proj(),
                camera_pos: cam.eye,
                time: 0.0,
                sun_dir: Vec3::new(-0.4, -1.0, -0.3).normalize(),
                sun_color: Vec3::ONE,
                ambient: Vec3::splat(0.25),
                fog_color: Vec3::new(0.1, 0.12, 0.11),
                fog_density: 0.01,
                light_view_proj: orthographic_light_matrix(
                    Vec3::new(-0.4, -1.0, -0.3).normalize(),
                    Vec3::ZERO,
                    20.0,
                    1.0,
                    50.0,
                ),
                point0_pos: Vec3::ZERO,
                point0_range: 1.0,
                point0_color: Vec3::ZERO,
                point1_pos: Vec3::ZERO,
                point1_range: 1.0,
                point1_color: Vec3::ZERO,
                spot_pos: Vec3::ZERO,
                spot_range: 1.0,
                spot_dir: -Vec3::Y,
                spot_inner_cos: 1.0,
                spot_outer_cos: 0.9,
                spot_color: Vec3::ZERO,
                exposure: 1.0,
                contrast: 1.0,
                saturation: 1.0,
                cube_instances: &[],
                sphere_instances: &[],
                extra_instances: &[],
                foliage_instances: &[foliage],
                rock_instances: &[],
                mountain_instances: &[],
                ground_instances: &[Mat4::from_scale_rotation_translation(
                    Vec3::new(10.0, 0.1, 10.0),
                    Quat::IDENTITY,
                    Vec3::ZERO,
                )],
                prop0_instances: &[],
                prop1_instances: &[],
                prop2_instances: &[],
                prop3_instances: &[],
                skinned_model: None,
                skin_joints: &[],
                hud_verts: &[],
                draw_water: false,
                screenshot_path: None,
            })
            .expect("render foliage");
        let (_, _, rgba) = renderer.read_rgba8().expect("readback");
        assert!(rgba.iter().any(|&b| b > 20));
    }
}
