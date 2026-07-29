//! Engine application shell.

use shiloh_core::{EngineConfig, JobSystem, Time, logging};
use shiloh_ecs::{Schedule, Stage, World};
use shiloh_input::InputState;
use shiloh_render::{FrameContext, Renderer};
use shiloh_rhi::{Device, NullDevice};
use shiloh_scene::Scene;
use tracing::info;

use crate::lifecycle::Phase;
use crate::platform::{PlatformKind, detect_platform};

pub struct App {
    pub config: EngineConfig,
    pub time: Time,
    pub jobs: JobSystem,
    pub world: World,
    pub schedule: Schedule,
    pub scene: Scene,
    pub input: InputState,
    pub renderer: Renderer,
    device: Box<dyn Device>,
    phase: Phase,
    platform: PlatformKind,
    /// When set, exit after this many frames (headless / tests).
    max_frames: Option<u64>,
}

pub struct AppBuilder {
    config: EngineConfig,
    max_frames: Option<u64>,
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self {
            config: EngineConfig::default(),
            max_frames: None,
        }
    }
}

impl AppBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config(mut self, config: EngineConfig) -> Self {
        self.config = config;
        self
    }

    pub fn max_frames(mut self, frames: u64) -> Self {
        self.max_frames = Some(frames);
        self
    }

    pub fn build(self) -> App {
        logging::init();
        let mut jobs = JobSystem::builder();
        if let Some(n) = self.config.job_workers {
            jobs = jobs.worker_count(n);
        }
        let jobs = jobs.build();

        App {
            time: Time::new(self.config.fixed_update_hz),
            jobs,
            world: World::new(),
            schedule: Schedule::new(),
            scene: Scene::new(&self.config.app_name),
            input: InputState::new(),
            renderer: Renderer::new(),
            device: Box::new(NullDevice::new()),
            phase: Phase::Boot,
            platform: detect_platform(),
            max_frames: self.max_frames,
            config: self.config,
        }
    }
}

impl App {
    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn add_system(&mut self, stage: Stage, system: impl shiloh_ecs::System + 'static) {
        self.schedule.add_system(stage, system);
    }

    /// Runs the main loop (headless). Windowed loop lands behind feature `window`.
    pub fn run(mut self) -> anyhow::Result<()> {
        self.phase = Phase::Running;
        info!(
            app = %self.config.app_name,
            platform = ?self.platform,
            backend = self.device.info().backend,
            "Shiloh3D starting"
        );

        loop {
            self.input.begin_frame();
            let fixed_steps = self.time.tick();
            let frame = self.time.frame();

            self.schedule.run(&mut self.world);
            self.schedule.run_fixed(&mut self.world, fixed_steps);

            let ctx = FrameContext {
                device: self.device.as_ref(),
                frame_index: frame.frame_index,
                width: 1280,
                height: 720,
            };
            self.renderer.begin_frame(&ctx);
            self.schedule.run_render(&mut self.world);
            self.renderer.end_frame(self.device.as_ref())?;

            if let Some(max) = self.max_frames
                && frame.frame_index >= max
            {
                break;
            }
        }

        self.phase = Phase::Shutdown;
        info!("Shiloh3D shutdown");
        Ok(())
    }
}
