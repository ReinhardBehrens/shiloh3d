//! Shiloh3D cross-platform showcase demo — believable 3D slice.
//!
//! Iso camera · textured PBR · multi-light + shadows · skinned anim · fog ·
//! water · HUD · tonemap · ECS hierarchy · scene JSON.

mod gltf_gpu;
mod showcase;
mod winit_map;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use glam::{Mat4, Quat, Vec3};
use shiloh_animation::SkinPalette;
use shiloh_core::{EngineConfig, JobSystem, Time, logging};
use shiloh_input::{Action, ActionMap, InputState, KeyCode};
use shiloh_render::{HudVertex, SliceDrawParams, SliceRenderer, orthographic_light_matrix};
use shiloh_scene::{Camera, ProjectionKind, propagate_transforms, save_scene};
use tracing::{info, warn};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton as WinitMouse, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode as WinitKey, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::showcase::ShowcaseState;
use crate::winit_map::{map_key, map_mouse};

#[derive(Parser, Debug)]
#[command(name = "shiloh-demo", about = "Shiloh3D engine showcase (Windows / macOS / Linux)")]
struct Args {
    /// Run N frames without presenting (smoke / CI).
    #[arg(long)]
    headless_frames: Option<u64>,
    /// Instance count for the cube field (GPU instancing stress).
    #[arg(long, default_value_t = 48)]
    cubes: u32,
    /// Capture version screenshots into this directory, then exit.
    #[arg(long)]
    screenshot_dir: Option<PathBuf>,
}

struct DemoApp {
    args: Args,
    window: Option<Arc<Window>>,
    renderer: Option<SliceRenderer>,
    time: Time,
    input: InputState,
    actions: ActionMap,
    jobs: JobSystem,
    showcase: ShowcaseState,
    camera: Camera,
    pan: Vec3,
    distance: f32,
    dragging: bool,
    last_mouse: Option<(f64, f64)>,
    cube_mats: Vec<Mat4>,
    sphere_mats: Vec<Mat4>,
    /// Instances for the optional imported glTF mesh (see `assets/sample.gltf`).
    extra_mats: Vec<Mat4>,
    exit_requested: bool,
    scene_saved: bool,
    screenshot_plan: Vec<(String, f32, Vec3, f32)>,
    screenshot_index: usize,
    screenshot_warmup: u32,
}

impl DemoApp {
    fn new(args: Args) -> anyhow::Result<Self> {
        logging::init();
        let config = EngineConfig {
            app_name: "Shiloh3D Showcase".into(),
            fixed_update_hz: 60.0,
            job_workers: Some(4),
            assets_root: "assets".into(),
        };

        let jobs = JobSystem::builder()
            .worker_count(config.job_workers.unwrap_or(4))
            .build();

        let mut actions = ActionMap::default();
        actions.bind_key(Action("forward"), KeyCode::W);
        actions.bind_key(Action("back"), KeyCode::S);
        actions.bind_key(Action("left"), KeyCode::A);
        actions.bind_key(Action("right"), KeyCode::D);
        actions.bind_key(Action("boost"), KeyCode::LeftShift);
        actions.bind_key(Action("reset"), KeyCode::R);
        actions.bind_key(Action("quit"), KeyCode::Escape);

        let showcase = ShowcaseState::boot(&config, args.cubes, &jobs)?;
        let mut camera = Camera::isometric(Vec3::new(0.0, 0.5, 0.0), 18.0);
        camera.projection = ProjectionKind::Isometric;

        let screenshot_plan = if args.screenshot_dir.is_some() {
            vec![
                (
                    "v1-overview.png".into(),
                    18.0,
                    Vec3::ZERO,
                    1.5,
                ),
                (
                    "v1-character.png".into(),
                    10.0,
                    Vec3::new(2.0, 0.0, 1.5),
                    3.2,
                ),
                (
                    "v1-water-fog.png".into(),
                    22.0,
                    Vec3::new(-2.0, 0.0, 3.0),
                    5.0,
                ),
            ]
        } else {
            Vec::new()
        };

        Ok(Self {
            args,
            window: None,
            renderer: None,
            time: Time::new(config.fixed_update_hz),
            input: InputState::new(),
            actions,
            jobs,
            showcase,
            camera,
            pan: Vec3::ZERO,
            distance: 18.0,
            dragging: false,
            last_mouse: None,
            cube_mats: Vec::new(),
            sphere_mats: Vec::new(),
            extra_mats: Vec::new(),
            exit_requested: false,
            scene_saved: false,
            screenshot_plan,
            screenshot_index: 0,
            screenshot_warmup: 0,
        })
    }

    fn update_camera(&mut self) {
        let focus = Vec3::new(0.0, 0.5, 0.0) + self.pan;
        let mut cam = Camera::isometric(focus, self.distance);
        if let Some(r) = &self.renderer {
            cam.set_aspect(r.size.0, r.size.1);
        } else {
            cam.aspect = self.camera.aspect;
        }
        self.camera = cam;
    }

    fn build_hud(hp: f32, mp: f32) -> Vec<HudVertex> {
        // Bottom-left health / mana bars + hotbar slots (NDC).
        let mut v = Vec::new();
        let push_quad = |out: &mut Vec<HudVertex>, x0, y0, x1, y1, rgba: [f32; 4]| {
            out.extend_from_slice(&[
                HudVertex {
                    pos: [x0, y0],
                    color: rgba,
                },
                HudVertex {
                    pos: [x1, y0],
                    color: rgba,
                },
                HudVertex {
                    pos: [x1, y1],
                    color: rgba,
                },
                HudVertex {
                    pos: [x0, y0],
                    color: rgba,
                },
                HudVertex {
                    pos: [x1, y1],
                    color: rgba,
                },
                HudVertex {
                    pos: [x0, y1],
                    color: rgba,
                },
            ]);
        };
        // Backplates
        push_quad(&mut v, -0.95, -0.92, -0.55, -0.86, [0.05, 0.05, 0.06, 0.85]);
        push_quad(&mut v, -0.95, -0.84, -0.55, -0.78, [0.05, 0.05, 0.06, 0.85]);
        // Fills
        let hx = -0.95 + 0.40 * hp.clamp(0.0, 1.0);
        let mx = -0.95 + 0.40 * mp.clamp(0.0, 1.0);
        push_quad(&mut v, -0.95, -0.92, hx, -0.86, [0.55, 0.12, 0.14, 0.95]);
        push_quad(&mut v, -0.95, -0.84, mx, -0.78, [0.12, 0.28, 0.65, 0.95]);
        // Hotbar
        for i in 0..5 {
            let x0 = -0.22 + i as f32 * 0.12;
            let x1 = x0 + 0.10;
            push_quad(&mut v, x0, -0.95, x1, -0.82, [0.08, 0.09, 0.10, 0.9]);
            push_quad(
                &mut v,
                x0 + 0.01,
                -0.94,
                x1 - 0.01,
                -0.83,
                [0.18 + i as f32 * 0.05, 0.22, 0.16, 0.85],
            );
        }
        v
    }

    fn tick_frame(&mut self) -> anyhow::Result<()> {
        self.input.begin_frame();

        let fixed = self.time.tick();
        let frame = self.time.frame();
        let dt = frame.delta_seconds;
        let t = frame.elapsed.as_secs_f32();

        if self.actions.pressed(&self.input, Action("quit")) {
            self.exit_requested = true;
            return Ok(());
        }
        if self.actions.pressed(&self.input, Action("reset")) {
            self.pan = Vec3::ZERO;
            self.distance = 18.0;
        }

        let boost = if self.actions.down(&self.input, Action("boost")) {
            2.5
        } else {
            1.0
        };
        let speed = 8.0 * dt * boost;
        // Iso-aligned pan on XZ.
        if self.actions.down(&self.input, Action("left")) {
            self.pan.x -= speed;
            self.pan.z += speed;
        }
        if self.actions.down(&self.input, Action("right")) {
            self.pan.x += speed;
            self.pan.z -= speed;
        }
        if self.actions.down(&self.input, Action("forward")) {
            self.pan.x -= speed;
            self.pan.z -= speed;
        }
        if self.actions.down(&self.input, Action("back")) {
            self.pan.x += speed;
            self.pan.z += speed;
        }

        self.update_camera();

        // Automated version-1 screenshot camera positions.
        let mut shot_path: Option<PathBuf> = None;
        if let Some(dir) = self.args.screenshot_dir.clone() {
            if self.screenshot_index < self.screenshot_plan.len() {
                let (name, dist, pan, warmup_secs) =
                    self.screenshot_plan[self.screenshot_index].clone();
                self.distance = dist;
                self.pan = pan;
                self.update_camera();
                self.screenshot_warmup += 1;
                let need = (warmup_secs * 60.0).ceil() as u32;
                if self.screenshot_warmup >= need.max(8) {
                    std::fs::create_dir_all(&dir)?;
                    shot_path = Some(dir.join(name));
                }
            } else {
                self.exit_requested = true;
            }
        }

        self.showcase
            .tick(dt, fixed, t, &self.jobs, &self.input);
        propagate_transforms(&mut self.showcase.scene.world);

        if !self.scene_saved {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join("showcase_scene.json");
            if let Err(err) = save_scene(&path, &self.showcase.scene, Some(&self.camera)) {
                warn!(?err, "scene save failed");
            } else {
                info!(?path, "wrote scene JSON");
            }
            self.scene_saved = true;
        }

        let cube_count = self.showcase.cube_count();
        self.cube_mats.resize(cube_count, Mat4::IDENTITY);
        {
            use rayon::prelude::*;
            self.cube_mats
                .par_iter_mut()
                .enumerate()
                .for_each(|(i, slot)| {
                    let angle = t * 0.35 + i as f32 * 0.4;
                    let radius = 4.0 + (i % 6) as f32 * 0.45;
                    let y = 0.55 + ((t * 1.6 + i as f32).sin() * 0.25);
                    let pos = Vec3::new(angle.cos() * radius, y, angle.sin() * radius);
                    let rot = Quat::from_euler(
                        glam::EulerRot::YXZ,
                        angle,
                        t * 0.5 + i as f32 * 0.08,
                        0.15,
                    );
                    let scale = Vec3::splat(0.32 + (i % 3) as f32 * 0.06);
                    *slot = Mat4::from_scale_rotation_translation(scale, rot, pos);
                });
        }

        let bob = (t * 1.2).sin() * 0.35;
        let ball = self.showcase.physics_ball_position();
        self.sphere_mats = vec![
            Mat4::from_scale_rotation_translation(
                Vec3::splat(1.1),
                Quat::from_rotation_y(t * 0.4),
                Vec3::new(0.0, 1.4 + bob, 0.0),
            ),
            // Physics-driven ball (falls / slides from stub integrator).
            Mat4::from_scale_rotation_translation(Vec3::splat(0.55), Quat::IDENTITY, ball),
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.45),
                Quat::IDENTITY,
                Vec3::new(6.0, 1.0, -1.5),
            ),
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.45),
                Quat::IDENTITY,
                Vec3::new(-5.5, 0.9, 2.5),
            ),
        ];

        // Slowly rotating instance of the imported glTF mesh (assets/sample.gltf),
        // if one was loaded and uploaded in `resumed()`.
        self.extra_mats = vec![Mat4::from_scale_rotation_translation(
            Vec3::splat(1.0),
            Quat::from_rotation_y(t * 0.6),
            Vec3::new(-3.0, 0.5, 0.0),
        )];

        let sun_dir = Vec3::new(-0.45, -1.0, -0.35).normalize();
        let light_view_proj =
            orthographic_light_matrix(sun_dir, Vec3::new(0.0, 0.5, 0.0), 28.0, 1.0, 70.0);
        let skin = SkinPalette::demo_sway(t, 3);
        let char_xform = Mat4::from_scale_rotation_translation(
            Vec3::splat(1.2),
            Quat::from_rotation_y(t * 0.6),
            Vec3::new(2.5, 0.0, 2.0),
        );
        let hud = Self::build_hud(0.72 + (t * 0.5).sin() * 0.05, 0.55);

        if let Some(renderer) = self.renderer.as_mut() {
            renderer.render(SliceDrawParams {
                view_proj: self.camera.view_proj(),
                camera_pos: self.camera.eye,
                time: t,
                sun_dir,
                sun_color: Vec3::new(1.0, 0.92, 0.78),
                ambient: Vec3::new(0.08, 0.10, 0.09),
                fog_color: Vec3::new(0.18, 0.22, 0.20),
                fog_density: 0.035,
                light_view_proj,
                point0_pos: Vec3::new(3.0, 2.5, 1.0),
                point0_range: 12.0,
                point0_color: Vec3::new(0.55, 0.75, 1.0),
                point1_pos: Vec3::new(-4.0, 1.8, -2.0),
                point1_range: 10.0,
                point1_color: Vec3::new(1.0, 0.45, 0.25),
                exposure: 1.05,
                contrast: 1.08,
                saturation: 1.12,
                cube_instances: &self.cube_mats,
                sphere_instances: &self.sphere_mats,
                extra_instances: &self.extra_mats,
                skinned_model: Some(char_xform),
                skin_joints: &skin.joints,
                hud_verts: &hud,
                draw_water: true,
                screenshot_path: shot_path.as_deref(),
            })?;
            if shot_path.is_some() {
                self.screenshot_index += 1;
                self.screenshot_warmup = 0;
                if self.screenshot_index >= self.screenshot_plan.len() {
                    info!("version 1 screenshots complete");
                    self.exit_requested = true;
                }
            }
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        if let Some(max) = self.args.headless_frames
            && frame.frame_index >= max
        {
            self.exit_requested = true;
        }

        Ok(())
    }
}

impl ApplicationHandler for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Shiloh3D — Believable Slice")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                match pollster::block_on(SliceRenderer::new(Arc::clone(&window))) {
                    Ok(mut renderer) => {
                        info!("slice renderer ready");
                        self.camera.set_aspect(renderer.size.0, renderer.size.1);

                        let gltf_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("assets")
                            .join("sample.gltf");
                        match shiloh_assets::load_gltf(&gltf_path) {
                            Ok(doc) => {
                                if let Some(prim) = doc.primitives.first() {
                                    let mesh = crate::gltf_gpu::to_slice_mesh(prim);
                                    renderer.set_extra_mesh(&mesh);
                                    info!(
                                        path = %gltf_path.display(),
                                        vertices = mesh.vertices.len(),
                                        indices = mesh.indices.len(),
                                        "uploaded imported glTF mesh to GPU"
                                    );
                                } else {
                                    warn!(path = %gltf_path.display(), "glTF has no primitives");
                                }
                            }
                            Err(err) => {
                                warn!(?err, path = %gltf_path.display(), "failed to load sample glTF mesh");
                            }
                        }

                        self.renderer = Some(renderer);
                        self.window = Some(window);
                        self.update_camera();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                    Err(err) => {
                        warn!(?err, "failed to create slice renderer");
                        self.exit_requested = true;
                    }
                }
            }
            Err(err) => {
                warn!(?err, "failed to create window");
                self.exit_requested = true;
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => self.exit_requested = true,
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                    self.camera.set_aspect(size.width.max(1), size.height.max(1));
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.tick_frame() {
                    warn!(?err, "frame error");
                    self.exit_requested = true;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    let key = map_key(code);
                    match event.state {
                        ElementState::Pressed => self.input.key_down(key),
                        ElementState::Released => self.input.key_up(key),
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let btn = map_mouse(button);
                match state {
                    ElementState::Pressed => {
                        self.input.mouse_down(btn);
                        if button == WinitMouse::Left {
                            self.dragging = true;
                        }
                    }
                    ElementState::Released => {
                        self.input.mouse_up(btn);
                        if button == WinitMouse::Left {
                            self.dragging = false;
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = (position.x, position.y);
                if let Some((lx, ly)) = self.last_mouse
                    && self.dragging
                {
                    let dx = (x - lx) as f32 * 0.01;
                    let dy = (y - ly) as f32 * 0.01;
                    self.pan.x += dx + dy;
                    self.pan.z += dx - dy;
                }
                self.last_mouse = Some((x, y));
                self.input
                    .set_mouse_position(glam::Vec2::new(x as f32, y as f32));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.05,
                };
                self.distance = (self.distance - dy * 1.5).clamp(8.0, 40.0);
            }
            _ => {}
        }

        if self.exit_requested {
            event_loop.exit();
        }
        let _ = WinitKey::Escape;
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    info_banner(&args);

    if let Some(frames) = args.headless_frames {
        return run_headless(args, frames);
    }

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = DemoApp::new(args)?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn run_headless(args: Args, frames: u64) -> anyhow::Result<()> {
    logging::init();
    let config = EngineConfig {
        app_name: "Shiloh3D Showcase (headless)".into(),
        ..EngineConfig::default()
    };
    let jobs = JobSystem::builder().worker_count(2).build();
    let mut showcase = ShowcaseState::boot(&config, args.cubes, &jobs)?;
    let mut time = Time::new(60.0);
    let input = InputState::new();
    for _ in 0..frames {
        let fixed = time.tick();
        let frame = time.frame();
        showcase.tick(
            frame.delta_seconds,
            fixed,
            frame.elapsed.as_secs_f32(),
            &jobs,
            &input,
        );
        propagate_transforms(&mut showcase.scene.world);
    }
    info!(frames, "headless showcase complete");
    Ok(())
}

fn info_banner(args: &Args) {
    eprintln!(
        "\n  Shiloh3D Believable Slice  |  cubes={}  |  platform={}  |  WASD pan, drag LMB, scroll zoom, Esc quit\n",
        args.cubes,
        std::env::consts::OS
    );
}
