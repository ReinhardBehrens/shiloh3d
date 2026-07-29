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
