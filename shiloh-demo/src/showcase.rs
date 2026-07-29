//! Boots and ticks every Shiloh subsystem for the showcase.

use std::path::PathBuf;
use std::sync::Arc;

use shiloh_animation::{AnimState, AnimStateMachine, AnimationClip, BlendTree, Skeleton};
use shiloh_assets::{AssetCache, AssetPackage};
use shiloh_audio::{AudioClip, AudioMixer, AudioSource, Listener};
use shiloh_core::{EngineConfig, JobSystem};
use shiloh_ecs::{Entity, World};
use shiloh_editor::Project;
use shiloh_input::InputState;
use shiloh_network::{InMemoryTransport, Packet, ReplicationChannel, Transport};
use shiloh_physics::{PhysicsWorld, RigidBody, RigidBodyKind, StubPhysics};
use shiloh_rhi::{BufferDesc, BufferUsage, Device, NullDevice};
use shiloh_scene::{Scene, Transform, propagate_transforms, set_parent};
use shiloh_scripting::{ScriptContext, ScriptModule, ScriptRegistry};
use tracing::{debug, info, warn};

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
    pub gltf_prims: usize,
    physics_body_id: usize,
    physics_entity: Entity,
    beep_clip: Arc<AudioClip>,
    audio_logged: bool,
}

impl ShowcaseState {
    pub fn boot(config: &EngineConfig, cubes: u32, _jobs: &JobSystem) -> anyhow::Result<Self> {
        let mut world = World::new();
        let mut scene = Scene::new(&config.app_name);

        // Scene hierarchy: root + children (exercises GlobalTransform propagation).
        let root = scene.spawn_transform(Transform::from_translation(glam::Vec3::ZERO));
        for i in 0..cubes.min(16) {
            let child = scene.spawn_transform(Transform::from_translation(glam::Vec3::new(
                i as f32 * 0.5,
                0.25,
                0.0,
            )));
            set_parent(&mut scene.world, child, root);
            world.spawn(Transform::from_translation(glam::Vec3::X * i as f32));
        }
        propagate_transforms(&mut scene.world);

        let mut physics_backend = StubPhysics::new();
        let physics_body_id = physics_backend.add_body(RigidBody {
            kind: RigidBodyKind::Dynamic,
            position: glam::Vec3::new(0.0, 3.0, 0.0),
            linear_velocity: glam::Vec3::new(0.6, 0.0, 0.3),
            ..Default::default()
        });
        let physics = PhysicsWorld::new(physics_backend);

        // Entity whose Transform tracks the dynamic body each fixed step (Phase 2
        // physics→Transform sync; shiloh-physics stays free of shiloh-scene).
        let physics_entity =
            scene.spawn_transform(Transform::from_translation(glam::Vec3::new(0.0, 3.0, 0.0)));

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

        // Boot-time one-shot: proves the software mixer actually renders samples
        // (exit criterion: non-silent mix buffer), not just silence.
        let beep_clip = Arc::new(AudioClip::sine_beep(48_000, 880.0, 0.15, 0.3));
        audio.play_oneshot(Arc::clone(&beep_clip), 0.3);

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

        // Optional glTF smoke-load (sample.gltf / sample.glb under demo assets/).
        let mut gltf_prims = 0usize;
        let gltf_candidates = [
            asset_dir.join("sample.gltf"),
            asset_dir.join("sample.glb"),
        ];
        let mut loaded_gltf = false;
        for gltf_path in &gltf_candidates {
            if !gltf_path.exists() {
                continue;
            }
            match shiloh_assets::load_gltf(gltf_path) {
                Ok(doc) => {
                    gltf_prims = doc.primitives.len();
                    info!(
                        path = %gltf_path.display(),
                        prims = gltf_prims,
                        skinned = doc.skin.is_some(),
                        "loaded glTF"
                    );
                    loaded_gltf = true;
                    break;
                }
                Err(err) => warn!(?err, path = %gltf_path.display(), "glTF load failed"),
            }
        }
        if !loaded_gltf {
            debug!("no sample.gltf/glb — procedural skinned mesh remains the fallback");
        }

        let project_dir = asset_dir.join("demo_project");
        if !project_dir.join("shiloh.project.json").exists() {
            let _ = Project::create(&project_dir, "ShowcaseProject");
        }

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
            gltf_prims,
            physics_body_id,
            physics_entity,
            beep_clip,
            audio_logged: false,
        })
    }

    pub fn cube_count(&self) -> usize {
        self.cube_count
    }

    /// World position of the dynamic physics ball (for visual instance sync).
    pub fn physics_ball_position(&self) -> glam::Vec3 {
        self.physics
            .backend
            .bodies()
            .get(self.physics_body_id)
            .map(|b| b.position)
            .unwrap_or(glam::Vec3::Y * 3.0)
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

        // Phase 2 physics→Transform sync: copy the dynamic body's position into
        // its tracked scene entity each fixed step (shiloh-physics has no
        // knowledge of Transform, so the copy happens here in the demo).
        let physics_pos = self
            .physics
            .backend_mut()
            .bodies()
            .get(self.physics_body_id)
            .map(|b| b.position);
        if let Some(position) = physics_pos
            && let Some(transform) = self.scene.world.get_mut::<Transform>(self.physics_entity)
        {
            transform.translation = position;
            transform.mark_dirty();
        }

        let barrier = jobs.spawn_batch((0..4u32).map(|i| {
            move || {
                let _ = i.wrapping_mul(7);
            }
        }));
        barrier.wait();

        let mut mix = [0.0f32; 256];
        self.audio.mix(&mut mix);
        if !self.audio_logged {
            let peak = mix.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
            if peak > 1e-6 {
                info!(peak, "audio oneshot mixed");
                self.audio_logged = true;
            }
        }
        let _ = &self.beep_clip;

        let _ = self.anim.current;
        let _ = self.blend.clips.len();
        let _ = time_secs;
        let _ = self.gltf_prims;

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

        let _ = self.assets.state(shiloh_assets::AssetId::from_raw(0));
        let _ = self.null_rhi.info();
    }
}
