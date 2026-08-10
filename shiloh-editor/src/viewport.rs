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

#[cfg(feature = "ui")]
use shiloh_ray::{RayScene, camera_ray_from_ndc};

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
    /// Y-axis rotate (radians) while Rotate tool dragging.
    RotateY {
        radians: f32,
    },
    /// Uniform scale factor delta while Scale tool dragging.
    ScaleUniform {
        factor: f32,
    },
    /// Axis-constrained translate (Godot/UE gizmo handle).
    TranslateAxis {
        axis: GizmoAxis,
        delta: f32,
    },
    /// Axis-constrained rotate (radians about axis).
    RotateAxis {
        axis: GizmoAxis,
        radians: f32,
    },
    /// Axis-constrained non-uniform scale factor.
    ScaleAxis {
        axis: GizmoAxis,
        factor: f32,
    },
    /// Alt+drag duplicate-and-move (Unreal).
    DuplicateTranslate {
        delta: Vec3,
    },
    /// Landscape sculpt at world XZ (positive = raise).
    TerrainSculpt {
        world: Vec3,
        strength: f32,
        radius: f32,
    },
    /// Landscape paint layer 0..3.
    TerrainPaint {
        world: Vec3,
        layer: u8,
        strength: f32,
        radius: f32,
    },
    /// Foliage paint / erase at XZ.
    FoliagePaint {
        world: Vec3,
        erase: bool,
    },
    /// Asset-palette place mode: primary click hit the ground plane.
    PlaceAt {
        world: Vec3,
    },
    /// Place brush cancelled because the user picked an existing node.
    ExitPlaceMode,
}

/// RGB transform gizmo axis — Borrowed from Godot 4 TranslationGizmo / Unreal WER.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
}

/// Viewport transform / world tool — Godot QWER + Unreal Modes + Blender-accurate ray.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportTool {
    #[default]
    Select,
    Move,
    Rotate,
    Scale,
    /// Borrowed from Unreal Engine: Landscape Mode (Shift+2).
    Landscape,
    /// Borrowed from Unreal Engine: Foliage Mode (Shift+3).
    Foliage,
    /// Borrowed from Blender: mesh ray pick via `shiloh-ray` / Parry.
    RayAccurate,
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
    /// Active gizmo axis while dragging (None = free XZ / screen).
    gizmo_axis: Option<GizmoAxis>,
    /// Alt was held when drag started → duplicate-move.
    alt_duplicate: bool,
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
            gizmo_axis: None,
            alt_duplicate: false,
            last_pointer: None,
            drag_start: None,
        }
    }
}

impl ViewportCamera {
    /// Borrowed from Unreal / Godot: F focuses the camera on selection AABB center.
    pub fn focus_selection(&mut self, center: Vec3, radius: f32) {
        self.focus = center;
        self.distance = (radius * 3.5).clamp(6.0, 80.0);
    }

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
        terrain: Option<&shiloh_scene::TerrainChunk>,
        grid_snap: bool,
        snap_size: f32,
        paint_layer: u8,
        landscape_paint: bool,
    ) -> Option<ViewportEvent> {
        // Borrowed from Godot 4: W Move tool drag (G grab removed — G is Unreal Game view).
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

        // --- Translate / rotate / scale drag ---
        if resp.drag_started_by(egui::PointerButton::Primary) {
            self.last_pointer = resp.interact_pointer_pos();
            self.drag_start = self.last_pointer;
            self.alt_duplicate = ui.input(|i| i.modifiers.alt);
            self.gizmo_axis = self.last_pointer.and_then(|p| {
                selection_gizmo_axis(p, selection, pickables, camera, rect, tool)
            });
            let transform_tool = matches!(
                tool,
                ViewportTool::Move | ViewportTool::Rotate | ViewportTool::Scale
            );
            let near_selected = !place_mode
                && transform_tool
                && self.last_pointer.is_some_and(|p| {
                    self.gizmo_axis.is_some()
                        || pointer_near_selection(p, selection, pickables, camera, rect)
                        || pick_entity_aabb(p, pickables, camera, rect)
                            .is_some_and(|e| selection.iter().any(|&s| s == e))
                });
            if !place_mode && near_selected {
                self.dragging_move = true;
            }
        }

        // --- Click select / place / landscape / foliage ---
        if resp.clicked_by(egui::PointerButton::Primary) && !self.dragging_move {
            if let Some(pos) = resp.interact_pointer_pos() {
                let shift = ui.input(|i| i.modifiers.shift);
                let pick = |p: egui::Pos2| -> Option<Entity> {
                    if tool == ViewportTool::RayAccurate {
                        pick_entity_ray(p, pickables, camera, rect)
                    } else {
                        pick_entity_aabb(p, pickables, camera, rect)
                    }
                };
                if place_mode {
                    if let Some(entity) = pick(pos) {
                        event = Some(ViewportEvent::Select {
                            entity,
                            mode: select_mode_from_input(ui),
                        });
                    } else if let Some(world) = ray_hit_ground(pos, camera, rect, terrain) {
                        event = Some(ViewportEvent::PlaceAt { world });
                    }
                } else if tool == ViewportTool::Landscape {
                    if let Some(world) = ray_hit_ground(pos, camera, rect, terrain) {
                        if landscape_paint {
                            event = Some(ViewportEvent::TerrainPaint {
                                world,
                                layer: paint_layer,
                                strength: 0.55,
                                radius: 3.5,
                            });
                        } else {
                            let strength = if shift { -0.35 } else { 0.35 };
                            event = Some(ViewportEvent::TerrainSculpt {
                                world,
                                strength,
                                radius: 3.5,
                            });
                        }
                    }
                } else if tool == ViewportTool::Foliage {
                    if let Some(world) = ray_hit_ground(pos, camera, rect, terrain) {
                        event = Some(ViewportEvent::FoliagePaint {
                            world,
                            erase: shift,
                        });
                    }
                } else if let Some(entity) = pick(pos) {
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
            self.gizmo_axis = None;
            self.alt_duplicate = false;
            self.last_pointer = None;
            self.drag_start = None;
        }

        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(prev) = self.last_pointer {
                let delta = pos - prev;
                if self.dragging_move {
                    // Borrowed from Unreal: Ctrl temporarily disables snap.
                    let free = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    let snap = grid_snap && !free;
                    match tool {
                        ViewportTool::Rotate => {
                            let radians = delta.x * 0.01;
                            event = Some(match self.gizmo_axis {
                                Some(axis) => ViewportEvent::RotateAxis { axis, radians },
                                None => ViewportEvent::RotateY { radians },
                            });
                        }
                        ViewportTool::Scale => {
                            let f = 1.0 + (-delta.y) * 0.005;
                            if f > 0.01 {
                                event = Some(match self.gizmo_axis {
                                    Some(axis) => ViewportEvent::ScaleAxis { axis, factor: f },
                                    None => ViewportEvent::ScaleUniform { factor: f },
                                });
                            }
                        }
                        _ => {
                            let mut world_delta = match self.gizmo_axis {
                                Some(GizmoAxis::X) => {
                                    Vec3::X * self.xz_delta_from_screen(delta).x
                                }
                                Some(GizmoAxis::Y) => Vec3::Y * (-delta.y) * self.distance * 0.0025,
                                Some(GizmoAxis::Z) => {
                                    Vec3::Z * self.xz_delta_from_screen(delta).z
                                }
                                None => self.xz_delta_from_screen(delta),
                            };
                            if snap && snap_size > 1e-6 {
                                world_delta = snap_vec(world_delta, snap_size);
                            }
                            if world_delta.length_squared() > 1e-8 {
                                // Borrowed from Unreal Engine: Alt+drag duplicate-and-move.
                                event = Some(if self.alt_duplicate {
                                    ViewportEvent::DuplicateTranslate { delta: world_delta }
                                } else {
                                    ViewportEvent::Translate { delta: world_delta }
                                });
                            }
                        }
                    }
                } else if self.dragging_orbit {
                    self.yaw += delta.x * 0.005;
                    self.pitch = (self.pitch + delta.y * 0.005).clamp(0.08, 1.45);
                } else if self.dragging_pan {
                    let right = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());
                    let up = Vec3::Y;
                    let scale = self.distance * 0.0025;
                    self.focus += right * (-delta.x * scale) + up * (delta.y * scale);
                } else if matches!(tool, ViewportTool::Landscape | ViewportTool::Foliage)
                    && resp.dragged_by(egui::PointerButton::Primary)
                {
                    if let Some(world) = ray_hit_ground(pos, camera, rect, terrain) {
                        let shift = ui.input(|i| i.modifiers.shift);
                        event = if tool == ViewportTool::Landscape {
                            if landscape_paint {
                                Some(ViewportEvent::TerrainPaint {
                                    world,
                                    layer: paint_layer,
                                    strength: 0.35,
                                    radius: 3.5,
                                })
                            } else {
                                Some(ViewportEvent::TerrainSculpt {
                                    world,
                                    strength: if shift { -0.2 } else { 0.2 },
                                    radius: 3.5,
                                })
                            }
                        } else {
                            Some(ViewportEvent::FoliagePaint {
                                world,
                                erase: shift,
                            })
                        };
                    }
                }
            }
            if self.dragging_orbit
                || self.dragging_pan
                || self.dragging_move
                || matches!(tool, ViewportTool::Landscape | ViewportTool::Foliage)
                    && resp.dragged_by(egui::PointerButton::Primary)
            {
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

/// Borrowed from Blender mesh pick — Parry TriMesh/BVH via `shiloh-ray`.
fn pick_entity_ray(
    pointer: egui::Pos2,
    pickables: &[Pickable],
    camera: &Camera,
    rect: egui::Rect,
) -> Option<Entity> {
    if rect.width() < 1.0 || rect.height() < 1.0 {
        return pick_entity_aabb(pointer, pickables, camera, rect);
    }
    let ndc_x = ((pointer.x - rect.left()) / rect.width()) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((pointer.y - rect.top()) / rect.height()) * 2.0;
    let (origin, dir) = camera_ray_from_ndc(camera.view_proj(), ndc_x, ndc_y);
    let mut scene = RayScene::default();
    for (i, p) in pickables.iter().enumerate() {
        let world = Mat4::from_translation(p.center - Vec3::new(0.0, p.half.y, 0.0));
        scene.insert_box(i as u64, world, p.half);
    }
    scene
        .cast(origin, dir, 500.0)
        .and_then(|hit| pickables.get(hit.id as usize).map(|p| p.entity))
        .or_else(|| pick_entity_aabb(pointer, pickables, camera, rect))
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
        terrain: Option<&shiloh_scene::TerrainChunk>,
        foliage: Option<&shiloh_scene::FoliageLayer>,
        grid_snap: bool,
        snap_size: f32,
        paint_layer: u8,
        landscape_paint: bool,
        game_view: bool,
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

        // Landscape Mode: displace a coarse grid of ground tiles from TerrainChunk.
        if let Some(terrain) = terrain {
            push_terrain_visual(terrain, &mut ground_mats, &mut rock_mats);
        }
        // Foliage Mode: draw painted instances into the live viewport.
        if let Some(layer) = foliage {
            for inst in &layer.instances {
                let yaw = Quat::from_rotation_y(inst.yaw);
                let pos = Vec3::from_array(inst.translation);
                let s = inst.scale;
                let typ = inst.typ.to_ascii_lowercase();
                if typ.contains("rock") {
                    rock_mats.push(Mat4::from_scale_rotation_translation(
                        Vec3::new(0.9, 0.7, 0.85) * s,
                        yaw,
                        pos,
                    ));
                } else {
                    let h = if typ.contains("pine") {
                        4.0
                    } else if typ.contains("birch") {
                        3.2
                    } else {
                        1.8
                    };
                    foliage_mats.push(Mat4::from_scale_rotation_translation(
                        Vec3::new(1.15, h, 1.15) * s,
                        yaw,
                        pos,
                    ));
                }
            }
        }

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
            terrain,
            grid_snap,
            snap_size,
            paint_layer,
            landscape_paint,
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
                sun_color: Vec3::new(1.25, 0.88, 0.58) * 1.55,
                ambient: Vec3::new(0.10, 0.12, 0.16),
                fog_color: Vec3::new(0.72, 0.58, 0.45),
                fog_density,
                light_view_proj,
                point0_pos: Vec3::new(6.0, 4.0, 3.0),
                point0_range: 22.0,
                point0_color: Vec3::new(0.45, 0.62, 1.0) * 0.75,
                point1_pos: Vec3::new(-7.0, 3.0, -2.0),
                point1_range: 18.0,
                point1_color: Vec3::new(1.1, 0.5, 0.28) * 0.75,
                spot_pos: Vec3::new(0.0, 10.0, 2.0),
                spot_range: 28.0,
                spot_dir: Vec3::new(0.1, -1.0, 0.15).normalize(),
                spot_inner_cos: 0.92,
                spot_outer_cos: 0.72,
                spot_color: Vec3::new(1.12, 0.92, 0.7) * 1.5,
                exposure: 1.12,
                contrast: 1.22,
                saturation: 1.28,
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
        // Borrowed from Unreal Engine: Game view (G) hides gizmos/helpers.
        if !game_view {
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
                // Godot/UE transform gizmos — RGB axes at selection origin.
                match *tool {
                    ViewportTool::Move => draw_move_gizmo(&painter, pos, &camera, rect),
                    ViewportTool::Rotate => draw_rotate_gizmo(&painter, pos, &camera, rect),
                    ViewportTool::Scale => draw_scale_gizmo(&painter, pos, &camera, rect),
                    _ if selection.len() == 1 => draw_move_gizmo(&painter, pos, &camera, rect),
                    _ => {}
                }
            }

            // Godot place-preview: ghost ring on ground under cursor while brushing.
            if place_brush_name.is_some() {
                if let Some(pointer) = resp.hover_pos() {
                    if let Some(hit) = ray_hit_ground(pointer, &camera, rect, terrain) {
                        draw_place_ghost(&painter, hit, &camera, rect);
                    }
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
        Vec2::new(520.0, 26.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Borrowed from Godot 4: QWER · Borrowed from Unreal: Landscape/Foliage · Blender: Ray.
            for (label, value) in [
                ("Select", ViewportTool::Select),
                ("Move", ViewportTool::Move),
                ("Rotate", ViewportTool::Rotate),
                ("Scale", ViewportTool::Scale),
                ("Land", ViewportTool::Landscape),
                ("Foliage", ViewportTool::Foliage),
                ("Ray", ViewportTool::RayAccurate),
            ] {
                let selected = *tool == value;
                if ui
                    .selectable_label(selected, rich_text_tool(label, selected))
                    .clicked()
                {
                    *tool = value;
                }
            }
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

/// Borrowed from Godot 4: rotation gizmo rings (screen-projected axis tips).
fn draw_rotate_gizmo(painter: &egui::Painter, origin: Vec3, camera: &Camera, rect: egui::Rect) {
    let Some(o) = project_to_viewport(origin, camera, rect) else {
        return;
    };
    let r = 1.35_f32;
    let axes = [
        (GizmoAxis::X, Color32::from_rgb(230, 70, 70)),
        (GizmoAxis::Y, Color32::from_rgb(80, 210, 90)),
        (GizmoAxis::Z, Color32::from_rgb(70, 130, 240)),
    ];
    for (axis, color) in axes {
        let dir = match axis {
            GizmoAxis::X => Vec3::X,
            GizmoAxis::Y => Vec3::Y,
            GizmoAxis::Z => Vec3::Z,
        };
        if let Some(tip) = project_to_viewport(origin + dir * r, camera, rect) {
            painter.circle_stroke(tip, 6.0, egui::Stroke::new(2.0_f32, color));
            painter.line_segment([o, tip], egui::Stroke::new(1.5_f32, color));
        }
    }
    painter.circle_stroke(o, 10.0, egui::Stroke::new(1.5_f32, Color32::from_rgb(220, 220, 230)));
}

/// Borrowed from Godot 4: scale gizmo cubes on axes.
fn draw_scale_gizmo(painter: &egui::Painter, origin: Vec3, camera: &Camera, rect: egui::Rect) {
    let Some(o) = project_to_viewport(origin, camera, rect) else {
        return;
    };
    let axis_len = 1.5_f32;
    let axes = [
        (Vec3::X * axis_len, Color32::from_rgb(230, 70, 70)),
        (Vec3::Y * axis_len, Color32::from_rgb(80, 210, 90)),
        (Vec3::Z * axis_len, Color32::from_rgb(70, 130, 240)),
    ];
    for (delta, color) in axes {
        if let Some(tip) = project_to_viewport(origin + delta, camera, rect) {
            painter.line_segment([o, tip], egui::Stroke::new(2.5_f32, color));
            painter.rect_filled(
                egui::Rect::from_center_size(tip, Vec2::splat(8.0)),
                1.0,
                color,
            );
        }
    }
    painter.rect_filled(
        egui::Rect::from_center_size(o, Vec2::splat(9.0)),
        1.0,
        Color32::from_rgb(240, 240, 245),
    );
}

fn snap_vec(v: Vec3, size: f32) -> Vec3 {
    let s = size.max(1e-6);
    Vec3::new(
        (v.x / s).round() * s,
        (v.y / s).round() * s,
        (v.z / s).round() * s,
    )
}

fn selection_gizmo_axis(
    pointer: egui::Pos2,
    selection: &[Entity],
    pickables: &[Pickable],
    camera: &Camera,
    rect: egui::Rect,
    tool: ViewportTool,
) -> Option<GizmoAxis> {
    if !matches!(
        tool,
        ViewportTool::Move | ViewportTool::Rotate | ViewportTool::Scale
    ) {
        return None;
    }
    let origin = selection.first().and_then(|&sel| {
        pickables
            .iter()
            .find(|p| p.entity == sel)
            .map(|p| p.center - Vec3::new(0.0, p.half.y, 0.0))
    })?;
    let tip_dist = 1.55_f32;
    let candidates = [
        (GizmoAxis::X, origin + Vec3::X * tip_dist),
        (GizmoAxis::Y, origin + Vec3::Y * tip_dist),
        (GizmoAxis::Z, origin + Vec3::Z * tip_dist),
    ];
    let mut best: Option<(GizmoAxis, f32)> = None;
    for (axis, tip) in candidates {
        if let Some(screen) = project_to_viewport(tip, camera, rect) {
            let d = (screen - pointer).length();
            if d <= 12.0 && best.map_or(true, |(_, bd)| d < bd) {
                best = Some((axis, d));
            }
        }
    }
    best.map(|(a, _)| a)
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
        format!("Place: {name} · LMB ground · Esc cancel · MMB orbit")
    } else {
        match tool {
            ViewportTool::Move => {
                "Move · drag near gizmo · G grab · MMB orbit · RMB pan".into()
            }
            ViewportTool::Rotate => {
                "Rotate · drag selection (Y-axis) · E tool · MMB orbit".into()
            }
            ViewportTool::Scale => {
                "Scale · drag selection · R tool · MMB orbit".into()
            }
            ViewportTool::Landscape => {
                "Landscape · LMB sculpt raise · Shift lower · [ ] brush · Shift+2".into()
            }
            ViewportTool::Foliage => {
                "Foliage · LMB paint · Shift erase · Shift+3".into()
            }
            ViewportTool::RayAccurate => {
                "RayAccurate · Parry mesh pick · Blender-like · Shift+4".into()
            }
            ViewportTool::Select => {
                "Select · LMB pick · Q · Shift+1 · MMB orbit · RMB pan".into()
            }
        }
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

/// Unproject a viewport pointer to the terrain surface (y=0 fallback).
fn ray_hit_ground(
    pointer: egui::Pos2,
    camera: &Camera,
    rect: egui::Rect,
    terrain: Option<&shiloh_scene::TerrainChunk>,
) -> Option<Vec3> {
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
    let y = terrain
        .map(|chunk| {
            let half = chunk.world_size * 0.5;
            chunk.height_at_world(hit.x + half, hit.z + half)
        })
        .unwrap_or(0.0);
    Some(Vec3::new(hit.x, y, hit.z))
}

/// Coarse heightfield tiles so Landscape sculpt is visible in the slice viewport.
fn push_terrain_visual(
    terrain: &shiloh_scene::TerrainChunk,
    ground_mats: &mut Vec<Mat4>,
    rock_mats: &mut Vec<Mat4>,
) {
    let half = terrain.world_size * 0.5;
    let step = ((terrain.width.max(2) - 1) / 16).max(1);
    let cell = terrain.world_size / terrain.width.saturating_sub(1).max(1) as f32;
    let tile = (cell * step as f32 * 0.95).max(0.4);
    let mut iz = 0u32;
    while iz < terrain.height {
        let mut ix = 0u32;
        while ix < terrain.width {
            let (wx0, wz0) = terrain.grid_to_world(ix, iz);
            let wx = wx0 - half;
            let wz = wz0 - half;
            let y = terrain.height_at_world(wx0, wz0);
            if y.abs() > 0.02 {
                let i = (iz as usize) * (terrain.width as usize) + (ix as usize);
                let rockish = terrain
                    .weights
                    .get(i)
                    .map(|w| w[2] + w[1] > 0.45)
                    .unwrap_or(false);
                let mat = Mat4::from_scale_rotation_translation(
                    Vec3::new(tile, (y.abs() * 0.5 + 0.08).min(4.0), tile),
                    Quat::IDENTITY,
                    Vec3::new(wx, y * 0.5, wz),
                );
                if rockish {
                    rock_mats.push(mat);
                } else {
                    ground_mats.push(mat);
                }
            }
            ix = ix.saturating_add(step);
            if ix == 0 {
                break;
            }
        }
        iz = iz.saturating_add(step);
        if iz == 0 {
            break;
        }
    }
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
