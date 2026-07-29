//! Boots and ticks every Shiloh subsystem for the showcase.

use std::path::PathBuf;

use shiloh_animation::{AnimState, AnimStateMachine, AnimationClip, BlendTree, Skeleton};
use shiloh_assets::{AssetCache, AssetPackage};
use shiloh_audio::{AudioMixer, AudioSource, Listener};
use shiloh_core::{EngineConfig, JobSystem};
use shiloh_ecs::World;
use shiloh_editor::Project;
use shiloh_input::InputState;
use shiloh_network::{InMemoryTransport, Packet, ReplicationChannel, Transport};
use shiloh_physics::{PhysicsWorld, RigidBody, RigidBodyKind, StubPhysics};
use shiloh_rhi::{BufferDesc, BufferUsage, Device, NullDevice};
use shiloh_scene::{Scene, Transform};
use shiloh_scripting::{ScriptContext, ScriptModule, ScriptRegistry};
use tracing::debug;

struct ShowcaseScript {
    pulses: u64,
}

impl ScriptModule for ShowcaseScript {
    fn name(&self) -> &str {
        "showcase_script"
    }

    fn on_update(&mut self, ctx: &mut ScriptContext<'_>) {
        self.pulses = self.pulses.wrapping_add(1);
        if self.pulses % 120 == 0 {
            debug!(
                scene = ctx.scene_name,
                entities = ctx.world.entity_count(),
                dt = ctx.delta_seconds,
                "script heartbeat"
            );
        }
    }
}

pub struct ShowcaseState {
    pub world: World,
    pub scene: Scene,
    physics: PhysicsWorld<StubPhysics>,
    anim: AnimStateMachine,
    blend: BlendTree,
    audio: AudioMixer,
    scripts: ScriptRegistry,
    assets: AssetCache,
    transport_a: InMemoryTransport,
    transport_b: InMemoryTransport,
    null_rhi: NullDevice,
    cube_count: usize,
    net_ticks: u64,
}

impl ShowcaseState {
    pub fn boot(config: &EngineConfig, cubes: u32, _jobs: &JobSystem) -> anyhow::Result<Self> {
        let mut world = World::new();
        let mut scene = Scene::new(&config.app_name);

        // Scene + ECS entities with transforms.
        for i in 0..cubes.min(16) {
            let t = Transform::from_translation(glam::Vec3::new(i as f32, 0.0, 0.0));
            scene.spawn_transform(t);
            world.spawn(Transform::from_translation(glam::Vec3::X * i as f32));
        }

        let mut physics_backend = StubPhysics::new();
        physics_backend.add_body(RigidBody {
            kind: RigidBodyKind::Dynamic,
            ..Default::default()
        });
        let physics = PhysicsWorld::new(physics_backend);

        let mut blend = BlendTree::default();
        let clip_idx = blend.add_clip(AnimationClip {
            name: "idle".into(),
            duration: 1.0,
            tracks: Vec::new(),
        });
        let _ = Skeleton::default();
        let mut anim = AnimStateMachine::default();
        anim.states.push(AnimState {
            name: "idle".into(),
            clip_index: clip_idx,
        });
        anim.current = 0;

        let audio = AudioMixer::new(48_000);
        audio.set_listener(Listener::default());
        audio.add_source(AudioSource {
            spatial: true,
            position: glam::Vec3::new(2.0, 1.0, 0.0),
            ..Default::default()
        });

        let mut scripts = ScriptRegistry::new();
        scripts.register(ShowcaseScript { pulses: 0 });

        let assets = AssetCache::new();
        let demo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let asset_dir = demo_root.join("assets");
        std::fs::create_dir_all(&asset_dir)?;
        let readme = asset_dir.join("showcase.txt");
        if !readme.exists() {
            std::fs::write(
                &readme,
                "Shiloh3D showcase asset file — loaded via shiloh-assets.\n",
            )?;
        }
        let _asset_id = assets.load_bytes(&readme)?;
        let pkg = AssetPackage::new("showcase");
        let pkg_path = asset_dir.join("package.json");
        std::fs::write(&pkg_path, serde_json::to_string_pretty(&pkg)?)?;

        // Editor project sidecar (optional, under target-friendly temp in assets).
        let project_dir = asset_dir.join("demo_project");
        if !project_dir.join("shiloh.project.json").exists() {
            let _ = Project::create(&project_dir, "ShowcaseProject");
        }

        // Touch null RHI path so the abstraction stays exercised beside wgpu.
        let null_rhi = NullDevice::new();
        let _ = null_rhi.create_buffer(&BufferDesc {
            size: 256,
            usage: BufferUsage::UNIFORM,
            label: Some("showcase_stub"),
        })?;

        Ok(Self {
            world,
            scene,
            physics,
            anim,
            blend,
            audio,
            scripts,
            assets,
            transport_a: InMemoryTransport::new(),
            transport_b: InMemoryTransport::new(),
            null_rhi,
            cube_count: cubes as usize,
            net_ticks: 0,
        })
    }

    pub fn cube_count(&self) -> usize {
        self.cube_count
    }

    pub fn tick(
        &mut self,
        dt: f32,
        fixed_steps: u32,
        time_secs: f32,
        jobs: &JobSystem,
        _input: &InputState,
    ) {
        for _ in 0..fixed_steps {
            self.physics.step(1.0 / 60.0);
        }

        // Job system fence — schedule a trivial parallel batch each frame.
        let barrier = jobs.spawn_batch((0..4u32).map(|i| {
            move || {
                let _ = i.wrapping_mul(7);
            }
        }));
        barrier.wait();

        let mut mix = [0.0f32; 256];
        self.audio.mix(&mut mix);

        let _ = self.anim.current;
        let _ = self.blend.clips.len();
        let _ = time_secs;

        let mut ctx = ScriptContext {
            world: &mut self.world,
            scene_name: &self.scene.name,
            delta_seconds: dt,
        };
        self.scripts.update_all(&mut ctx);

        self.net_ticks = self.net_ticks.wrapping_add(1);
        if self.net_ticks % 30 == 0 {
            let _ = self.transport_a.send(Packet {
                channel: ReplicationChannel::Unreliable as u8,
                payload: self.net_ticks.to_le_bytes().to_vec(),
            });
            self.transport_a.deliver_to(&mut self.transport_b);
            let _ = self.transport_b.recv();
        }

        // Touch asset cache so the subsystem stays in the live demo path.
        let _ = self.assets.state(shiloh_assets::AssetId::from_raw(0));

        let _ = self.null_rhi.info();
    }
}
