//! Shiloh3D cross-platform showcase demo.
//!
//! Exercises: ECS, scene/camera, forward GPU (sky/grid/lit WGSL), input, jobs,
//! physics stub, animation, audio mixer, assets, networking, scripting, editor project.

mod showcase;
mod winit_map;

use std::sync::Arc;

use clap::Parser;
use glam::{Mat4, Quat, Vec3};
use shiloh_core::{EngineConfig, JobSystem, Time, logging};
use shiloh_input::{Action, ActionMap, InputState, KeyCode};
use shiloh_render::ForwardRenderer;
use shiloh_scene::Camera;
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
    #[arg(long, default_value_t = 64)]
    cubes: u32,
}

struct DemoApp {
    args: Args,
    window: Option<Arc<Window>>,
    renderer: Option<ForwardRenderer>,
    time: Time,
    input: InputState,
    actions: ActionMap,
    jobs: JobSystem,
    showcase: ShowcaseState,
    camera: Camera,
    yaw: f32,
    pitch: f32,
    distance: f32,
    dragging: bool,
    last_mouse: Option<(f64, f64)>,
    cube_mats: Vec<Mat4>,
    sphere_mats: Vec<Mat4>,
    exit_requested: bool,
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

        Ok(Self {
            args,
            window: None,
            renderer: None,
            time: Time::new(config.fixed_update_hz),
            input: InputState::new(),
            actions,
            jobs,
            showcase,
            camera: Camera::default(),
            yaw: 0.6,
            pitch: 0.35,
            distance: 14.0,
            dragging: false,
            last_mouse: None,
            cube_mats: Vec::new(),
            sphere_mats: Vec::new(),
            exit_requested: false,
        })
    }

    fn update_camera_from_orbit(&mut self) {
        let cp = self.pitch.cos();
        let eye = Vec3::new(
            self.distance * self.yaw.cos() * cp,
            self.distance * self.pitch.sin(),
            self.distance * self.yaw.sin() * cp,
        );
        self.camera.eye = eye + Vec3::new(0.0, 1.5, 0.0);
        self.camera.target = Vec3::new(0.0, 1.0, 0.0);
    }

    fn tick_frame(&mut self) -> anyhow::Result<()> {
        self.input.begin_frame();
        // Mouse deltas already applied via events before redraw.

        let fixed = self.time.tick();
        let frame = self.time.frame();
        let dt = frame.delta_seconds;

        if self.actions.pressed(&self.input, Action("quit")) {
            self.exit_requested = true;
            return Ok(());
        }
        if self.actions.pressed(&self.input, Action("reset")) {
            self.yaw = 0.6;
            self.pitch = 0.35;
            self.distance = 14.0;
        }

        let boost = if self.actions.down(&self.input, Action("boost")) {
            2.5
        } else {
            1.0
        };
        let orbit = 1.2 * dt * boost;
        if self.actions.down(&self.input, Action("left")) {
            self.yaw -= orbit;
        }
        if self.actions.down(&self.input, Action("right")) {
            self.yaw += orbit;
        }
        if self.actions.down(&self.input, Action("forward")) {
            self.distance = (self.distance - 6.0 * dt * boost).max(4.0);
        }
        if self.actions.down(&self.input, Action("back")) {
            self.distance = (self.distance + 6.0 * dt * boost).min(40.0);
        }

        self.update_camera_from_orbit();
        self.showcase
            .tick(dt, fixed, frame.elapsed.as_secs_f32(), &self.jobs, &self.input);

        // Parallel CPU instance matrix build (rayon) — reuse buffer, no per-frame alloc.
        let t = frame.elapsed.as_secs_f32();
        let cube_count = self.showcase.cube_count();
        self.cube_mats.resize(cube_count, Mat4::IDENTITY);
        {
            use rayon::prelude::*;
            self.cube_mats
                .par_iter_mut()
                .enumerate()
                .for_each(|(i, slot)| {
                    let angle = t * 0.6 + i as f32 * 0.35;
                    let radius = 3.5 + (i % 5) as f32 * 0.55;
                    let y = 0.6 + ((t * 2.0 + i as f32).sin() * 0.35);
                    let pos = Vec3::new(angle.cos() * radius, y, angle.sin() * radius);
                    let rot = Quat::from_euler(
                        glam::EulerRot::YXZ,
                        angle,
                        t * 0.8 + i as f32 * 0.1,
                        0.2,
                    );
                    let scale = Vec3::splat(0.35 + (i % 3) as f32 * 0.08);
                    *slot = Mat4::from_scale_rotation_translation(scale, rot, pos);
                });
        }
        // Also exercise the engine job system each frame.
        self.jobs
            .spawn_batch((0..4u32).map(|i| {
                move || {
                    let _ = i.wrapping_mul(3);
                }
            }))
            .wait();

        let bob = (t * 1.4).sin() * 0.4;
        self.sphere_mats = vec![
            Mat4::from_scale_rotation_translation(
                Vec3::splat(1.4),
                Quat::from_rotation_y(t * 0.5),
                Vec3::new(0.0, 1.6 + bob, 0.0),
            ),
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.55),
                Quat::from_rotation_x(t),
                Vec3::new(5.5, 1.2, -2.0),
            ),
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.55),
                Quat::from_rotation_z(t * 1.2),
                Vec3::new(-5.0, 1.0, 3.0),
            ),
        ];

        if let Some(renderer) = self.renderer.as_mut() {
            let (w, h) = renderer.size;
            self.camera.set_aspect(w, h);
            renderer.render(
                self.camera.view_proj(),
                self.camera.eye,
                t,
                &self.cube_mats,
                &self.sphere_mats,
            )?;
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
            .with_title("Shiloh3D — Engine Showcase")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        match event_loop.create_window(attrs) {
            Ok(window) => {
                let window = Arc::new(window);
                match pollster::block_on(ForwardRenderer::new(Arc::clone(&window))) {
                    Ok(renderer) => {
                        info!("forward renderer ready");
                        self.camera.set_aspect(renderer.size.0, renderer.size.1);
                        self.renderer = Some(renderer);
                        self.window = Some(window);
                        self.update_camera_from_orbit();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                    Err(err) => {
                        warn!(?err, "failed to create GPU renderer");
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
                    let dx = (x - lx) as f32 * 0.005;
                    let dy = (y - ly) as f32 * 0.005;
                    self.yaw += dx;
                    self.pitch = (self.pitch + dy).clamp(-1.2, 1.2);
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
                self.distance = (self.distance - dy * 1.5).clamp(4.0, 40.0);
            }
            _ => {}
        }

        if self.exit_requested {
            event_loop.exit();
        }
        let _ = WinitKey::Escape; // keep import used on all platforms
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
    }
    info!(frames, "headless showcase complete");
    Ok(())
}

fn info_banner(args: &Args) {
    eprintln!(
        "\n  Shiloh3D Showcase  |  cubes={}  |  platform={}  |  controls: WASD orbit, drag LMB, scroll zoom, Esc quit\n",
        args.cubes,
        std::env::consts::OS
    );
}
