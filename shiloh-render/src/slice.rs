//! Believable 3D slice — PBR forward, shadows, water, post, HUD, skinning.

use std::path::Path;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable, bytes_of, cast_slice};
use glam::{Mat4, Vec3};
use tracing::info;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::mesh::{
    SliceInstance, SliceMeshCpu, SliceVertex, SkinnedMeshCpu, SkinnedVertex, demo_skinned_character,
    slice_foliage_mesh, slice_ground_mesh, slice_icosphere, slice_mountain_mesh, slice_rock_mesh,
    slice_unit_cube,
};

const SHADOW_SIZE: u32 = 2048;
const MAX_SKIN_JOINTS: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    ambient: [f32; 4],
    fog: [f32; 4],
    point0_pos_range: [f32; 4],
    point0_color: [f32; 4],
    point1_pos_range: [f32; 4],
    point1_color: [f32; 4],
    /// Spot: xyz position, w = range.
    spot_pos_range: [f32; 4],
    /// Spot: xyz direction, w = cos(outer cone).
    spot_dir_cos: [f32; 4],
    /// Spot: rgb intensity, w = cos(inner cone).
    spot_color: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShadowUniform {
    light_view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostUniform {
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkinUniform {
    joints: [[[f32; 4]; 4]; MAX_SKIN_JOINTS],
}

/// NDC colored quad vertex for the HUD pass.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

impl HudVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 24,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x4,
        ],
    };
}

/// Per-frame draw inputs for [`SliceRenderer::render`].
pub struct SliceDrawParams<'a> {
    pub view_proj: Mat4,
    pub camera_pos: Vec3,
    pub time: f32,
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub ambient: Vec3,
    pub fog_color: Vec3,
    pub fog_density: f32,
    pub light_view_proj: Mat4,
    pub point0_pos: Vec3,
    pub point0_range: f32,
    pub point0_color: Vec3,
    pub point1_pos: Vec3,
    pub point1_range: f32,
    pub point1_color: Vec3,
    pub spot_pos: Vec3,
    pub spot_range: f32,
    pub spot_dir: Vec3,
    pub spot_inner_cos: f32,
    pub spot_outer_cos: f32,
    pub spot_color: Vec3,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub cube_instances: &'a [Mat4],
    pub sphere_instances: &'a [Mat4],
    /// Instances of the optional imported mesh set via [`SliceRenderer::set_extra_mesh`].
    /// Empty by default; harmless (draws nothing) when no extra mesh is uploaded.
    pub extra_instances: &'a [Mat4],
    /// Forest-valley foliage (vertex-colored green cones/columns).
    pub foliage_instances: &'a [Mat4],
    /// Rock / boulder scatter (vertex-colored brown-grey).
    pub rock_instances: &'a [Mat4],
    /// Distant mountain backdrop (vertex-colored dark slate).
    pub mountain_instances: &'a [Mat4],
    /// Ground / terrain plane (vertex-colored grass green).
    pub ground_instances: &'a [Mat4],
    /// CC0 prop meshes uploaded via [`SliceRenderer::set_prop_mesh`] (slots 0–3).
    pub prop0_instances: &'a [Mat4],
    pub prop1_instances: &'a [Mat4],
    pub prop2_instances: &'a [Mat4],
    pub prop3_instances: &'a [Mat4],
    /// Optional root transform multiplied into each skin joint before upload.
    pub skinned_model: Option<Mat4>,
    pub skin_joints: &'a [Mat4],
    pub hud_verts: &'a [HudVertex],
    pub draw_water: bool,
    /// When set, copies the presented frame to this PNG path after the pass.
    pub screenshot_path: Option<&'a Path>,
}

struct GpuMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
}

/// Multi-pass slice renderer (shadow → HDR PBR → post → HUD).
pub struct SliceRenderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
    present_tex: Option<wgpu::Texture>,
    present_view: Option<wgpu::TextureView>,
    present_format: wgpu::TextureFormat,
    depth: wgpu::TextureView,
    hdr_tex: wgpu::Texture,
    hdr_view: wgpu::TextureView,
    shadow_view: wgpu::TextureView,
    frame_buf: wgpu::Buffer,
    shadow_buf: wgpu::Buffer,
    post_buf: wgpu::Buffer,
    skin_buf: wgpu::Buffer,
    pbr_bg: wgpu::BindGroup,
    water_bg: wgpu::BindGroup,
    shadow_bg: wgpu::BindGroup,
    skin_bg: wgpu::BindGroup,
    post_bg: wgpu::BindGroup,
    post_bgl: wgpu::BindGroupLayout,
    hdr_samp: wgpu::Sampler,
    shadow_pipeline: wgpu::RenderPipeline,
    pbr_pipeline: wgpu::RenderPipeline,
    water_pipeline: wgpu::RenderPipeline,
    skinned_pipeline: wgpu::RenderPipeline,
    post_pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    cube: GpuMesh,
    sphere: GpuMesh,
    foliage: GpuMesh,
    rock: GpuMesh,
    mountain: GpuMesh,
    ground: GpuMesh,
    skinned: GpuMesh,
    /// Optional GPU mesh uploaded via [`SliceRenderer::set_extra_mesh`] (e.g. an imported glTF).
    extra: Option<GpuMesh>,
    /// CC0 prop slots (shrub, fern, rock_09, rock_06).
    props: [Option<GpuMesh>; 4],
    instance_buf: wgpu::Buffer,
    instance_capacity: u32,
    hud_buf: wgpu::Buffer,
    hud_capacity: u32,
    pub size: (u32, u32),
}

impl SliceRenderer {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;
        let adapter = request_adapter(&instance, Some(&surface)).await?;
        let (device, queue) = request_device(&adapter).await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let mut renderer = init_renderer(device, queue, format, width, height, false)?;
        renderer.surface = Some(surface);
        renderer.config = Some(config);
        Ok(renderer)
    }

    /// Headless/offscreen renderer for editor embedding — no window surface.
    ///
    /// Renders into an internal `Rgba8UnormSrgb` texture. Use [`Self::read_rgba8`] after
    /// [`Self::render`] to read pixels on the CPU.
    ///
    /// Uses a flat white albedo so vertex colors (foliage/rock/mountain buckets) read clearly
    /// in the editor — the demo window path keeps the checker debug albedo.
    pub async fn new_offscreen(width: u32, height: u32) -> anyhow::Result<Self> {
        let width = width.max(1);
        let height = height.max(1);

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = request_adapter(&instance, None).await?;
        let (device, queue) = request_device(&adapter).await?;

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let (present_tex, present_view) = create_present_target(&device, width, height, format);

        let mut renderer = init_renderer(device, queue, format, width, height, true)?;
        renderer.present_tex = Some(present_tex);
        renderer.present_view = Some(present_view);
        Ok(renderer)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.size = (width, height);
        if let (Some(surface), Some(config)) = (&self.surface, &mut self.config) {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
        } else if self.present_tex.is_some() {
            let (present_tex, present_view) =
                create_present_target(&self.device, width, height, self.present_format);
            self.present_tex = Some(present_tex);
            self.present_view = Some(present_view);
        }
        self.depth = create_depth_view(&self.device, width, height);
        let (hdr_tex, hdr_view) = create_hdr_target(&self.device, width, height);
        self.hdr_tex = hdr_tex;
        self.hdr_view = hdr_view;
        self.post_bg = create_post_bg(
            &self.device,
            &self.post_bgl,
            &self.post_buf,
            &self.hdr_view,
            &self.hdr_samp,
        );
    }

    /// Uploads (or replaces) the optional extra mesh — e.g. a converted glTF
    /// import — drawn alongside the built-in cubes/spheres via
    /// [`SliceDrawParams::extra_instances`], using the same shadow + PBR pipelines.
    pub fn set_extra_mesh(&mut self, mesh: &SliceMeshCpu) {
        self.extra = Some(upload_slice_mesh(&self.device, mesh, "slice_extra"));
    }

    /// Uploads (or replaces) a CC0 prop mesh drawn like foliage (shadow + PBR).
    pub fn set_prop_mesh(&mut self, slot: usize, mesh: &SliceMeshCpu) {
        if slot >= self.props.len() {
            return;
        }
        let label = format!("slice_prop_{slot}");
        self.props[slot] = Some(upload_slice_mesh(&self.device, mesh, &label));
    }

    pub fn render(&mut self, params: SliceDrawParams<'_>) -> anyhow::Result<()> {
        let mut surface_frame = None;
        let (swap_view, screenshot_texture): (wgpu::TextureView, &wgpu::Texture) =
            if let Some(surface) = &self.surface {
                let config = self
                    .config
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("surface mode requires config"))?;
                let frame = match surface.get_current_texture() {
                    Ok(f) => f,
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        surface.configure(&self.device, config);
                        surface.get_current_texture()?
                    }
                    Err(e) => return Err(e.into()),
                };
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                surface_frame = Some(frame);
                let tex = &surface_frame.as_ref().unwrap().texture;
                (view, tex)
            } else {
                let tex = self
                    .present_tex
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("offscreen mode requires present_tex"))?;
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                (view, tex)
            };

        let frame_u = FrameUniform {
            view_proj: params.view_proj.to_cols_array_2d(),
            light_view_proj: params.light_view_proj.to_cols_array_2d(),
            camera_pos: [
                params.camera_pos.x,
                params.camera_pos.y,
                params.camera_pos.z,
                1.0,
            ],
            sun_dir: [params.sun_dir.x, params.sun_dir.y, params.sun_dir.z, 0.0],
            sun_color: [
                params.sun_color.x,
                params.sun_color.y,
                params.sun_color.z,
                1.0,
            ],
            ambient: [params.ambient.x, params.ambient.y, params.ambient.z, 1.0],
            fog: [
                params.fog_color.x,
                params.fog_color.y,
                params.fog_color.z,
                params.fog_density,
            ],
            point0_pos_range: [
                params.point0_pos.x,
                params.point0_pos.y,
                params.point0_pos.z,
                params.point0_range,
            ],
            point0_color: [
                params.point0_color.x,
                params.point0_color.y,
                params.point0_color.z,
                1.0,
            ],
            point1_pos_range: [
                params.point1_pos.x,
                params.point1_pos.y,
                params.point1_pos.z,
                params.point1_range,
            ],
            point1_color: [
                params.point1_color.x,
                params.point1_color.y,
                params.point1_color.z,
                1.0,
            ],
            spot_pos_range: [
                params.spot_pos.x,
                params.spot_pos.y,
                params.spot_pos.z,
                params.spot_range,
            ],
            spot_dir_cos: [
                params.spot_dir.x,
                params.spot_dir.y,
                params.spot_dir.z,
                params.spot_outer_cos,
            ],
            spot_color: [
                params.spot_color.x,
                params.spot_color.y,
                params.spot_color.z,
                params.spot_inner_cos,
            ],
            params: [
                params.time,
                params.exposure,
                params.contrast,
                params.saturation,
            ],
        };
        self.queue.write_buffer(&self.frame_buf, 0, bytes_of(&frame_u));

        let shadow_u = ShadowUniform {
            light_view_proj: params.light_view_proj.to_cols_array_2d(),
        };
        self.queue
            .write_buffer(&self.shadow_buf, 0, bytes_of(&shadow_u));

        let post_u = PostUniform {
            params: [params.exposure, params.contrast, params.saturation, 0.0],
        };
        self.queue.write_buffer(&self.post_buf, 0, bytes_of(&post_u));

        if !params.skin_joints.is_empty() {
            let mut skin = SkinUniform::zeroed();
            let root = params.skinned_model.unwrap_or(Mat4::IDENTITY);
            let ident = Mat4::IDENTITY.to_cols_array_2d();
            for slot in &mut skin.joints {
                *slot = ident;
            }
            for (i, &j) in params.skin_joints.iter().take(MAX_SKIN_JOINTS).enumerate() {
                skin.joints[i] = (root * j).to_cols_array_2d();
            }
            self.queue.write_buffer(&self.skin_buf, 0, bytes_of(&skin));
        }

        let mut packed: Vec<SliceInstance> = Vec::with_capacity(
            params.cube_instances.len()
                + params.sphere_instances.len()
                + params.extra_instances.len()
                + params.foliage_instances.len()
                + params.rock_instances.len()
                + params.mountain_instances.len()
                + params.ground_instances.len()
                + params.prop0_instances.len()
                + params.prop1_instances.len()
                + params.prop2_instances.len()
                + params.prop3_instances.len(),
        );
        packed.extend(
            params
                .cube_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let cube_count = params.cube_instances.len() as u32;
        packed.extend(
            params
                .sphere_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let sphere_count = params.sphere_instances.len() as u32;
        packed.extend(
            params
                .extra_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let extra_count = params.extra_instances.len() as u32;
        packed.extend(
            params
                .foliage_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let foliage_count = params.foliage_instances.len() as u32;
        packed.extend(
            params
                .rock_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let rock_count = params.rock_instances.len() as u32;
        packed.extend(
            params
                .mountain_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let mountain_count = params.mountain_instances.len() as u32;
        packed.extend(
            params
                .ground_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let ground_count = params.ground_instances.len() as u32;
        packed.extend(
            params
                .prop0_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let prop0_count = params.prop0_instances.len() as u32;
        packed.extend(
            params
                .prop1_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let prop1_count = params.prop1_instances.len() as u32;
        packed.extend(
            params
                .prop2_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let prop2_count = params.prop2_instances.len() as u32;
        packed.extend(
            params
                .prop3_instances
                .iter()
                .copied()
                .map(SliceInstance::from_mat4),
        );
        let prop3_count = params.prop3_instances.len() as u32;

        if packed.len() as u32 > self.instance_capacity {
            self.instance_capacity = (packed.len() as u32).next_power_of_two().max(64);
            self.instance_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("slice_instances"),
                size: (self.instance_capacity as u64) * 64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !packed.is_empty() {
            self.queue
                .write_buffer(&self.instance_buf, 0, cast_slice(&packed));
        }

        let hud_count = params.hud_verts.len() as u32;
        if hud_count > self.hud_capacity {
            self.hud_capacity = hud_count.next_power_of_two().max(64);
            self.hud_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("slice_hud"),
                size: (self.hud_capacity as u64) * std::mem::size_of::<HudVertex>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !params.hud_verts.is_empty() {
            self.queue
                .write_buffer(&self.hud_buf, 0, cast_slice(params.hud_verts));
        }

        let draw_skinned = !params.skin_joints.is_empty();
        let fog = params.fog_color;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("slice_frame"),
            });

        // --- Shadow pass ---
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("slice_shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.shadow_bg, &[]);
            let mut inst_off = 0u64;
            draw_instanced(
                &mut pass,
                &self.cube,
                &self.instance_buf,
                inst_off,
                cube_count,
            );
            inst_off += cube_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.sphere,
                &self.instance_buf,
                inst_off,
                sphere_count,
            );
            inst_off += sphere_count as u64 * 64;
            if extra_count > 0
                && let Some(extra) = &self.extra
            {
                draw_instanced(
                    &mut pass,
                    extra,
                    &self.instance_buf,
                    inst_off,
                    extra_count,
                );
            }
            inst_off += extra_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.ground,
                &self.instance_buf,
                inst_off,
                ground_count,
            );
            inst_off += ground_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.mountain,
                &self.instance_buf,
                inst_off,
                mountain_count,
            );
            inst_off += mountain_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.rock,
                &self.instance_buf,
                inst_off,
                rock_count,
            );
            inst_off += rock_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.foliage,
                &self.instance_buf,
                inst_off,
                foliage_count,
            );
            inst_off += foliage_count as u64 * 64;
            let prop_counts = [prop0_count, prop1_count, prop2_count, prop3_count];
            for (slot, &count) in prop_counts.iter().enumerate() {
                if count > 0
                    && let Some(prop) = &self.props[slot]
                {
                    draw_instanced(&mut pass, prop, &self.instance_buf, inst_off, count);
                }
                inst_off += count as u64 * 64;
            }
        }

        // --- Main HDR pass ---
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("slice_main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: fog.x as f64,
                            g: fog.y as f64,
                            b: fog.z as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pbr_pipeline);
            pass.set_bind_group(0, &self.pbr_bg, &[]);
            let mut inst_off = 0u64;
            draw_instanced(
                &mut pass,
                &self.cube,
                &self.instance_buf,
                inst_off,
                cube_count,
            );
            inst_off += cube_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.sphere,
                &self.instance_buf,
                inst_off,
                sphere_count,
            );
            inst_off += sphere_count as u64 * 64;
            if extra_count > 0
                && let Some(extra) = &self.extra
            {
                draw_instanced(
                    &mut pass,
                    extra,
                    &self.instance_buf,
                    inst_off,
                    extra_count,
                );
            }
            inst_off += extra_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.ground,
                &self.instance_buf,
                inst_off,
                ground_count,
            );
            inst_off += ground_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.mountain,
                &self.instance_buf,
                inst_off,
                mountain_count,
            );
            inst_off += mountain_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.rock,
                &self.instance_buf,
                inst_off,
                rock_count,
            );
            inst_off += rock_count as u64 * 64;
            draw_instanced(
                &mut pass,
                &self.foliage,
                &self.instance_buf,
                inst_off,
                foliage_count,
            );
            inst_off += foliage_count as u64 * 64;
            let prop_counts = [prop0_count, prop1_count, prop2_count, prop3_count];
            for (slot, &count) in prop_counts.iter().enumerate() {
                if count > 0
                    && let Some(prop) = &self.props[slot]
                {
                    draw_instanced(&mut pass, prop, &self.instance_buf, inst_off, count);
                }
                inst_off += count as u64 * 64;
            }

            if draw_skinned {
                pass.set_pipeline(&self.skinned_pipeline);
                pass.set_bind_group(0, &self.pbr_bg, &[]);
                pass.set_bind_group(1, &self.skin_bg, &[]);
                pass.set_vertex_buffer(0, self.skinned.vertex.slice(..));
                pass.set_index_buffer(self.skinned.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.skinned.index_count, 0, 0..1);
            }

            if params.draw_water {
                pass.set_pipeline(&self.water_pipeline);
                pass.set_bind_group(0, &self.water_bg, &[]);
                pass.draw(0..6, 0..1);
            }
        }

        // --- Post (HDR → swapchain) ---
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("slice_post"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &swap_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.post_pipeline);
            pass.set_bind_group(0, &self.post_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // --- HUD ---
        if hud_count > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("slice_hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &swap_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.hud_pipeline);
            pass.set_vertex_buffer(0, self.hud_buf.slice(..));
            pass.draw(0..hud_count, 0..1);
        }

        let screenshot = params.screenshot_path.map(|path| {
            let (w, h) = self.size;
            let bpp = 4u32;
            let unpadded_bytes_per_row = w * bpp;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
            let buffer_size = padded_bytes_per_row as u64 * h as u64;
            let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("slice_screenshot"),
                size: buffer_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: screenshot_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(h),
                    },
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            (path.to_path_buf(), staging, padded_bytes_per_row, unpadded_bytes_per_row, w, h)
        });

        self.queue.submit(std::iter::once(encoder.finish()));

        if let Some((path, staging, padded, unpadded, w, h)) = screenshot {
            let slice = staging.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            self.device
                .poll(wgpu::PollType::Wait)
                .map_err(|e| anyhow::anyhow!("screenshot map poll: {e}"))?;
            let data = slice.get_mapped_range();
            let mut rgba = Vec::with_capacity((unpadded * h) as usize);
            for row in 0..h {
                let start = (row * padded) as usize;
                let end = start + unpadded as usize;
                rgba.extend_from_slice(&data[start..end]);
            }
            drop(data);
            staging.unmap();

            // Swapchain is typically Bgra8UnormSrgb — convert to RGBA for PNG.
            if matches!(
                self.present_format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ) {
                for px in rgba.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
            }
            image::save_buffer(
                &path,
                &rgba,
                w,
                h,
                image::ColorType::Rgba8,
            )?;
            info!(path = %path.display(), width = w, height = h, "wrote screenshot");
        }

        if let Some(frame) = surface_frame {
            frame.present();
        }
        Ok(())
    }

    /// Reads the offscreen present target as RGBA8 bytes `(width, height, pixels)`.
    ///
    /// Intended for [`Self::new_offscreen`] after [`Self::render`]. Window/surface mode does
    /// not retain the swapchain texture after present; use [`SliceDrawParams::screenshot_path`]
    /// for window captures instead.
    pub fn read_rgba8(&mut self) -> anyhow::Result<(u32, u32, Vec<u8>)> {
        let tex = self
            .present_tex
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("read_rgba8 is only supported in offscreen mode"))?;
        let (w, h) = self.size;
        let rgba = copy_texture_to_rgba8(&self.device, &self.queue, tex, w, h, self.present_format)?;
        Ok((w, h, rgba))
    }
}

async fn request_adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> anyhow::Result<wgpu::Adapter> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|_| anyhow::anyhow!("no suitable GPU adapter"))?;

    info!(
        adapter = %adapter.get_info().name,
        backend = ?adapter.get_info().backend,
        "wgpu slice adapter"
    );
    Ok(adapter)
}

async fn request_device(adapter: &wgpu::Adapter) -> anyhow::Result<(wgpu::Device, wgpu::Queue)> {
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("shiloh-slice"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(Into::into)
}

fn create_present_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("slice_present"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn init_renderer(
    device: wgpu::Device,
    queue: wgpu::Queue,
    present_format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    flat_albedo: bool,
) -> anyhow::Result<SliceRenderer> {
    let depth = create_depth_view(&device, width, height);
    let (hdr_tex, hdr_view) = create_hdr_target(&device, width, height);

    let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("slice_shadow"),
        size: wgpu::Extent3d {
            width: SHADOW_SIZE,
            height: SHADOW_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let shadow_view = shadow_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let shadow_samp = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("slice_shadow_samp"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });

    let (albedo_view, albedo_samp) = if flat_albedo {
        create_solid_albedo(&device, &queue, [235, 235, 235, 255])
    } else {
        create_checker_albedo(&device, &queue)
    };

    let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slice_frame"),
        size: std::mem::size_of::<FrameUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shadow_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slice_shadow_ub"),
        size: std::mem::size_of::<ShadowUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let post_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slice_post"),
        size: std::mem::size_of::<PostUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let skin_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slice_skin"),
        size: std::mem::size_of::<SkinUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let pbr_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("slice_pbr_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Depth,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let water_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("slice_water_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("slice_shadow_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let skin_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("slice_skin_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let post_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("slice_post_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let hdr_samp = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("slice_hdr_samp"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let pbr_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("slice_pbr_bg"),
        layout: &pbr_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&shadow_samp),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&albedo_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&albedo_samp),
            },
        ],
    });

    let water_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("slice_water_bg"),
        layout: &water_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: frame_buf.as_entire_binding(),
        }],
    });

    let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("slice_shadow_bg"),
        layout: &shadow_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: shadow_buf.as_entire_binding(),
        }],
    });

    let skin_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("slice_skin_bg"),
        layout: &skin_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: skin_buf.as_entire_binding(),
        }],
    });

    let post_bg = create_post_bg(&device, &post_bgl, &post_buf, &hdr_view, &hdr_samp);

    let pbr_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("slice_pbr_layout"),
        bind_group_layouts: &[&pbr_bgl],
        push_constant_ranges: &[],
    });
    let water_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("slice_water_layout"),
        bind_group_layouts: &[&water_bgl],
        push_constant_ranges: &[],
    });
    let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("slice_shadow_layout"),
        bind_group_layouts: &[&shadow_bgl],
        push_constant_ranges: &[],
    });
    let skinned_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("slice_skinned_layout"),
        bind_group_layouts: &[&pbr_bgl, &skin_bgl],
        push_constant_ranges: &[],
    });
    let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("slice_post_layout"),
        bind_group_layouts: &[&post_bgl],
        push_constant_ranges: &[],
    });
    let hud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("slice_hud_layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let pbr_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("slice_pbr"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/slice/pbr.wgsl").into()),
    });
    let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("slice_shadow"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/slice/shadow.wgsl").into()),
    });
    let water_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("slice_water"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/slice/water.wgsl").into()),
    });
    let skinned_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("slice_skinned"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/slice/skinned.wgsl").into()),
    });
    let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("slice_post"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/slice/post.wgsl").into()),
    });
    let hud_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("slice_hud"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/slice/hud.wgsl").into()),
    });

    let hdr_target = wgpu::ColorTargetState {
        format: wgpu::TextureFormat::Rgba16Float,
        blend: Some(wgpu::BlendState::REPLACE),
        write_mask: wgpu::ColorWrites::ALL,
    };
    let hdr_blend = wgpu::ColorTargetState {
        format: wgpu::TextureFormat::Rgba16Float,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    };

    let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("slice_shadow_pipe"),
        layout: Some(&shadow_layout),
        vertex: wgpu::VertexState {
            module: &shadow_shader,
            entry_point: Some("vs_main"),
            buffers: &[SliceVertex::SHADOW_LAYOUT, SliceInstance::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let pbr_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("slice_pbr_pipe"),
        layout: Some(&pbr_layout),
        vertex: wgpu::VertexState {
            module: &pbr_shader,
            entry_point: Some("vs_main"),
            buffers: &[SliceVertex::LAYOUT, SliceInstance::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &pbr_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(hdr_target.clone())],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let water_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("slice_water_pipe"),
        layout: Some(&water_layout),
        vertex: wgpu::VertexState {
            module: &water_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &water_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(hdr_blend)],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let skinned_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("slice_skinned_pipe"),
        layout: Some(&skinned_layout),
        vertex: wgpu::VertexState {
            module: &skinned_shader,
            entry_point: Some("vs_main"),
            buffers: &[SkinnedVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &skinned_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(hdr_target)],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let post_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("slice_post_pipe"),
        layout: Some(&post_layout),
        vertex: wgpu::VertexState {
            module: &post_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &post_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: present_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("slice_hud_pipe"),
        layout: Some(&hud_layout),
        vertex: wgpu::VertexState {
            module: &hud_shader,
            entry_point: Some("vs_main"),
            buffers: &[HudVertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &hud_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: present_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let cube = upload_slice_mesh(&device, &slice_unit_cube(Vec3::new(0.85, 0.55, 0.35)), "slice_cube");
    let sphere =
        upload_slice_mesh(&device, &slice_icosphere(2, Vec3::new(0.35, 0.65, 0.85)), "slice_sphere");
    let foliage = upload_slice_mesh(&device, &slice_foliage_mesh(), "slice_foliage");
    let rock = upload_slice_mesh(&device, &slice_rock_mesh(), "slice_rock");
    let mountain = upload_slice_mesh(&device, &slice_mountain_mesh(), "slice_mountain");
    let ground = upload_slice_mesh(&device, &slice_ground_mesh(), "slice_ground");
    let skinned = upload_skinned_mesh(
        &device,
        &demo_skinned_character(Vec3::new(0.55, 0.75, 0.45)),
        "slice_char",
    );

    let instance_capacity = 512u32;
    let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slice_instances"),
        size: (instance_capacity as u64) * 64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let hud_capacity = 256u32;
    let hud_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slice_hud"),
        size: (hud_capacity as u64) * std::mem::size_of::<HudVertex>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    Ok(SliceRenderer {
        device,
        queue,
        surface: None,
        config: None,
        present_tex: None,
        present_view: None,
        present_format,
        depth,
        hdr_tex,
        hdr_view,
        shadow_view,
        frame_buf,
        shadow_buf,
        post_buf,
        skin_buf,
        pbr_bg,
        water_bg,
        shadow_bg,
        skin_bg,
        post_bg,
        post_bgl,
        hdr_samp,
        shadow_pipeline,
        pbr_pipeline,
        water_pipeline,
        skinned_pipeline,
        post_pipeline,
        hud_pipeline,
        cube,
        sphere,
        foliage,
        rock,
        mountain,
        ground,
        skinned,
        extra: None,
        props: [None, None, None, None],
        instance_buf,
        instance_capacity,
        hud_buf,
        hud_capacity,
        size: (width, height),
    })
}

/// Orthographic light matrix looking along `sun_dir` at `center` (handy for demos).
pub fn orthographic_light_matrix(
    sun_dir: Vec3,
    center: Vec3,
    half_extent: f32,
    near: f32,
    far: f32,
) -> Mat4 {
    let dir = sun_dir.normalize_or_zero();
    let eye = center - dir * ((far - near) * 0.5 + near.max(0.0));
    let up = if dir.y.abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = Mat4::look_at_rh(eye, center, up);
    let proj = Mat4::orthographic_rh(
        -half_extent,
        half_extent,
        -half_extent,
        half_extent,
        near,
        far,
    );
    proj * view
}

fn copy_texture_to_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> anyhow::Result<Vec<u8>> {
    let bpp = 4u32;
    let unpadded_bytes_per_row = width * bpp;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
    let buffer_size = padded_bytes_per_row as u64 * height as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slice_readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("slice_readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|e| anyhow::anyhow!("readback map poll: {e}"))?;
    let data = slice.get_mapped_range();
    let mut rgba = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        rgba.extend_from_slice(&data[start..end]);
    }
    drop(data);
    staging.unmap();

    if matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }
    Ok(rgba)
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("slice_depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    depth.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_hdr_target(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("slice_hdr"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_post_bg(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    post_buf: &wgpu::Buffer,
    hdr_view: &wgpu::TextureView,
    hdr_samp: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("slice_post_bg"),
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: post_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(hdr_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(hdr_samp),
            },
        ],
    })
}

fn create_solid_albedo(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: [u8; 4],
) -> (wgpu::TextureView, wgpu::Sampler) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("slice_albedo_flat"),
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let pixels: Vec<u8> = (0..16).flat_map(|_| rgba).collect();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(16),
            rows_per_image: Some(4),
        },
        wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let samp = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("slice_albedo_flat_samp"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    (view, samp)
}

fn create_checker_albedo(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    const N: u32 = 64;
    let mut pixels = vec![0u8; (N * N * 4) as usize];
    for y in 0..N {
        for x in 0..N {
            let check = ((x / 8) + (y / 8)) % 2 == 0;
            let v = if check { 220 } else { 40 };
            let i = ((y * N + x) * 4) as usize;
            pixels[i] = v;
            pixels[i + 1] = v;
            pixels[i + 2] = v;
            pixels[i + 3] = 255;
        }
    }
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("slice_albedo"),
        size: wgpu::Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(N * 4),
            rows_per_image: Some(N),
        },
        wgpu::Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let samp = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("slice_albedo_samp"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    (view, samp)
}

fn draw_instanced(
    pass: &mut wgpu::RenderPass<'_>,
    mesh: &GpuMesh,
    instance_buf: &wgpu::Buffer,
    instance_offset: u64,
    count: u32,
) {
    if count == 0 {
        return;
    }
    pass.set_vertex_buffer(0, mesh.vertex.slice(..));
    pass.set_vertex_buffer(1, instance_buf.slice(instance_offset..));
    pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
    pass.draw_indexed(0..mesh.index_count, 0, 0..count);
}

fn upload_slice_mesh(device: &wgpu::Device, mesh: &SliceMeshCpu, label: &str) -> GpuMesh {
    let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{label}_idx")),
        contents: cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    GpuMesh {
        vertex,
        index,
        index_count: mesh.indices.len() as u32,
    }
}

fn upload_skinned_mesh(device: &wgpu::Device, mesh: &SkinnedMeshCpu, label: &str) -> GpuMesh {
    let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{label}_idx")),
        contents: cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    GpuMesh {
        vertex,
        index,
        index_count: mesh.indices.len() as u32,
    }
}
