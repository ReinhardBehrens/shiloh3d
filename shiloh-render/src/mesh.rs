//! CPU mesh builders and GPU-friendly vertex layout.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Interleaved vertex used by the lit WGSL pipeline.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

impl Vertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 36,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x3,
        ],
    };
}

/// Per-instance model matrix as 4× vec4 columns (matches WGSL locations 3–6).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct InstanceRaw {
    pub cols: [[f32; 4]; 4],
}

impl InstanceRaw {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            3 => Float32x4,
            4 => Float32x4,
            5 => Float32x4,
            6 => Float32x4,
        ],
    };

    #[inline]
    pub fn from_mat4(m: glam::Mat4) -> Self {
        Self {
            cols: m.to_cols_array_2d(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MeshCpu {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshCpu {
    pub fn unit_cube(color: Vec3) -> Self {
        let c = color.to_array();
        // 24 unique verts (per-face normals).
        let faces: [(Vec3, Vec3, [(f32, f32, f32); 4]); 6] = [
            (Vec3::Z, Vec3::Z, [(-0.5, -0.5, 0.5), (0.5, -0.5, 0.5), (0.5, 0.5, 0.5), (-0.5, 0.5, 0.5)]),
            (-Vec3::Z, -Vec3::Z, [(0.5, -0.5, -0.5), (-0.5, -0.5, -0.5), (-0.5, 0.5, -0.5), (0.5, 0.5, -0.5)]),
            (Vec3::X, Vec3::X, [(0.5, -0.5, 0.5), (0.5, -0.5, -0.5), (0.5, 0.5, -0.5), (0.5, 0.5, 0.5)]),
            (-Vec3::X, -Vec3::X, [(-0.5, -0.5, -0.5), (-0.5, -0.5, 0.5), (-0.5, 0.5, 0.5), (-0.5, 0.5, -0.5)]),
            (Vec3::Y, Vec3::Y, [(-0.5, 0.5, 0.5), (0.5, 0.5, 0.5), (0.5, 0.5, -0.5), (-0.5, 0.5, -0.5)]),
            (-Vec3::Y, -Vec3::Y, [(-0.5, -0.5, -0.5), (0.5, -0.5, -0.5), (0.5, -0.5, 0.5), (-0.5, -0.5, 0.5)]),
        ];
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (ni, (_fn, n, corners)) in faces.iter().enumerate() {
            let base = (ni * 4) as u32;
            let na = n.to_array();
            for (x, y, z) in corners {
                vertices.push(Vertex {
                    position: [*x, *y, *z],
                    normal: na,
                    color: c,
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self { vertices, indices }
    }

    pub fn icosphere(subdivisions: u32, color: Vec3) -> Self {
        // Start from unit icosahedron, subdivide, project to sphere.
        let t = (1.0 + 5.0f32.sqrt()) / 2.0;
        let mut positions = vec![
            Vec3::new(-1.0, t, 0.0),
            Vec3::new(1.0, t, 0.0),
            Vec3::new(-1.0, -t, 0.0),
            Vec3::new(1.0, -t, 0.0),
            Vec3::new(0.0, -1.0, t),
            Vec3::new(0.0, 1.0, t),
            Vec3::new(0.0, -1.0, -t),
            Vec3::new(0.0, 1.0, -t),
            Vec3::new(t, 0.0, -1.0),
            Vec3::new(t, 0.0, 1.0),
            Vec3::new(-t, 0.0, -1.0),
            Vec3::new(-t, 0.0, 1.0),
        ];
        for p in &mut positions {
            *p = p.normalize();
        }
        let mut faces: Vec<[u32; 3]> = vec![
            [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
            [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
            [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
            [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
        ];

        for _ in 0..subdivisions {
            let mut midpoints = ahash::AHashMap::new();
            let mut new_faces = Vec::with_capacity(faces.len() * 4);
            let mut midpoint = |i: u32, j: u32, positions: &mut Vec<Vec3>| -> u32 {
                let key = if i < j { (i, j) } else { (j, i) };
                if let Some(&m) = midpoints.get(&key) {
                    return m;
                }
                let mid = (positions[i as usize] + positions[j as usize]).normalize();
                let idx = positions.len() as u32;
                positions.push(mid);
                midpoints.insert(key, idx);
                idx
            };
            for [a, b, c] in faces {
                let ab = midpoint(a, b, &mut positions);
                let bc = midpoint(b, c, &mut positions);
                let ca = midpoint(c, a, &mut positions);
                new_faces.push([a, ab, ca]);
                new_faces.push([b, bc, ab]);
                new_faces.push([c, ca, bc]);
                new_faces.push([ab, bc, ca]);
            }
            faces = new_faces;
        }

        let c = color.to_array();
        let vertices: Vec<Vertex> = positions
            .iter()
            .map(|p| Vertex {
                position: (*p * 0.5).to_array(),
                normal: p.normalize().to_array(),
                color: c,
            })
            .collect();
        let indices: Vec<u32> = faces.into_iter().flatten().collect();
        Self { vertices, indices }
    }
}

// --- Slice path (pos / nrm / uv / color + instance model) ---

/// Vertex for slice PBR / shadow meshes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SliceVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
}

impl SliceVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 44,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Float32x3,
        ],
    };

    /// Shadow pass only needs position @loc0; same stride as [`SliceVertex`].
    pub const SHADOW_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 44,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
    };
}

/// Per-instance model matrix (WGSL locations 4–7).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SliceInstance {
    pub cols: [[f32; 4]; 4],
}

impl SliceInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            4 => Float32x4,
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
        ],
    };

    #[inline]
    pub fn from_mat4(m: glam::Mat4) -> Self {
        Self {
            cols: m.to_cols_array_2d(),
        }
    }
}

/// Skinned vertex — joints @loc4, weights @loc5.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
}

impl SkinnedVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: 76,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Float32x3,
            4 => Uint32x4,
            5 => Float32x4,
        ],
    };
}

#[derive(Clone, Debug)]
pub struct SliceMeshCpu {
    pub vertices: Vec<SliceVertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct SkinnedMeshCpu {
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
}

/// Unit cube with per-face UVs for the slice PBR path.
pub fn slice_unit_cube(color: Vec3) -> SliceMeshCpu {
    let c = color.to_array();
    let faces: [(Vec3, [(f32, f32, f32); 4], [(f32, f32); 4]); 6] = [
        (
            Vec3::Z,
            [(-0.5, -0.5, 0.5), (0.5, -0.5, 0.5), (0.5, 0.5, 0.5), (-0.5, 0.5, 0.5)],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        ),
        (
            -Vec3::Z,
            [(0.5, -0.5, -0.5), (-0.5, -0.5, -0.5), (-0.5, 0.5, -0.5), (0.5, 0.5, -0.5)],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        ),
        (
            Vec3::X,
            [(0.5, -0.5, 0.5), (0.5, -0.5, -0.5), (0.5, 0.5, -0.5), (0.5, 0.5, 0.5)],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        ),
        (
            -Vec3::X,
            [(-0.5, -0.5, -0.5), (-0.5, -0.5, 0.5), (-0.5, 0.5, 0.5), (-0.5, 0.5, -0.5)],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        ),
        (
            Vec3::Y,
            [(-0.5, 0.5, 0.5), (0.5, 0.5, 0.5), (0.5, 0.5, -0.5), (-0.5, 0.5, -0.5)],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        ),
        (
            -Vec3::Y,
            [(-0.5, -0.5, -0.5), (0.5, -0.5, -0.5), (0.5, -0.5, 0.5), (-0.5, -0.5, 0.5)],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        ),
    ];
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (ni, (n, corners, uvs)) in faces.iter().enumerate() {
        let base = (ni * 4) as u32;
        let na = n.to_array();
        for ((x, y, z), (u, v)) in corners.iter().zip(uvs.iter()) {
            vertices.push(SliceVertex {
                position: [*x, *y, *z],
                normal: na,
                uv: [*u, *v],
                color: c,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    SliceMeshCpu { vertices, indices }
}

/// UV-mapped icosphere for the slice path.
pub fn slice_icosphere(subdivisions: u32, color: Vec3) -> SliceMeshCpu {
    let mesh = MeshCpu::icosphere(subdivisions, color);
    let vertices: Vec<SliceVertex> = mesh
        .vertices
        .iter()
        .map(|v| {
            let p = Vec3::from_array(v.position);
            let n = Vec3::from_array(v.normal).normalize();
            let u = 0.5 + n.z.atan2(n.x) / (std::f32::consts::TAU);
            let v_coord = 0.5 - n.y.asin() / std::f32::consts::PI;
            SliceVertex {
                position: p.to_array(),
                normal: n.to_array(),
                uv: [u, v_coord],
                color: v.color,
            }
        })
        .collect();
    SliceMeshCpu {
        vertices,
        indices: mesh.indices,
    }
}

/// Simple 3-bone box character (hips / torso / head) for demo skin sway.
pub fn demo_skinned_character(color: Vec3) -> SkinnedMeshCpu {
    let c = color.to_array();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Parts: (center, half-extents, primary joint, secondary joint, blend toward secondary at top).
    let parts: [(Vec3, Vec3, u32, u32, f32); 3] = [
        (Vec3::new(0.0, 0.35, 0.0), Vec3::new(0.22, 0.35, 0.14), 0, 1, 0.35),
        (Vec3::new(0.0, 0.95, 0.0), Vec3::new(0.28, 0.30, 0.16), 1, 2, 0.25),
        (Vec3::new(0.0, 1.40, 0.0), Vec3::new(0.16, 0.16, 0.16), 2, 2, 0.0),
    ];

    for (center, half, j0, j1, blend) in parts {
        push_skinned_box(
            &mut vertices,
            &mut indices,
            center,
            half,
            c,
            j0,
            j1,
            blend,
        );
    }

    SkinnedMeshCpu { vertices, indices }
}

fn push_skinned_box(
    vertices: &mut Vec<SkinnedVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    color: [f32; 3],
    joint_lo: u32,
    joint_hi: u32,
    blend_span: f32,
) {
    let faces: [(Vec3, [(f32, f32, f32); 4]); 6] = [
        (Vec3::Z, [(-1.0, -1.0, 1.0), (1.0, -1.0, 1.0), (1.0, 1.0, 1.0), (-1.0, 1.0, 1.0)]),
        (-Vec3::Z, [(1.0, -1.0, -1.0), (-1.0, -1.0, -1.0), (-1.0, 1.0, -1.0), (1.0, 1.0, -1.0)]),
        (Vec3::X, [(1.0, -1.0, 1.0), (1.0, -1.0, -1.0), (1.0, 1.0, -1.0), (1.0, 1.0, 1.0)]),
        (-Vec3::X, [(-1.0, -1.0, -1.0), (-1.0, -1.0, 1.0), (-1.0, 1.0, 1.0), (-1.0, 1.0, -1.0)]),
        (Vec3::Y, [(-1.0, 1.0, 1.0), (1.0, 1.0, 1.0), (1.0, 1.0, -1.0), (-1.0, 1.0, -1.0)]),
        (-Vec3::Y, [(-1.0, -1.0, -1.0), (1.0, -1.0, -1.0), (1.0, -1.0, 1.0), (-1.0, -1.0, 1.0)]),
    ];
    let y0 = center.y - half.y;
    let y1 = center.y + half.y;
    let span = if blend_span > 1e-4 {
        blend_span
    } else {
        1.0
    };

    for (n, corners) in &faces {
        let base = vertices.len() as u32;
        let na = n.to_array();
        for (i, (sx, sy, sz)) in corners.iter().enumerate() {
            let local = Vec3::new(sx * half.x, sy * half.y, sz * half.z);
            let pos = center + local;
            let t = ((pos.y - y0) / (y1 - y0).max(1e-4)).clamp(0.0, 1.0);
            let w_hi = (t * span).clamp(0.0, 1.0);
            let w_lo = 1.0 - w_hi;
            let uv = match i {
                0 => [0.0, 0.0],
                1 => [1.0, 0.0],
                2 => [1.0, 1.0],
                _ => [0.0, 1.0],
            };
            vertices.push(SkinnedVertex {
                position: pos.to_array(),
                normal: na,
                uv,
                color,
                joints: [joint_lo, joint_hi, 0, 0],
                weights: [w_lo, w_hi, 0.0, 0.0],
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}
