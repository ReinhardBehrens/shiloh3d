//! Accurate mesh raycasting for editor picking and place/sculpt hits.
//!
//! # Open source
//! Built on **[Parry3d](https://github.com/dimforge/parry)** (Dimforge, Apache-2.0 / MIT) —
//! triangle mesh + BVH ray cast. See root README “Open source attributions”.
//!
//! # Borrowed UX
//! Blender-style mesh ray pick (not screen-AABB fuzzy pick) for Accurate Ray edit mode.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

use glam::{Mat4, Vec3};
use parry3d::math::{Isometry, Point, Vector};
use parry3d::query::{Ray as ParryRay, RayCast};
use parry3d::shape::TriMesh;

/// One triangle mesh registered for picking (world space).
pub struct RayMesh {
    pub id: u64,
    mesh: TriMesh,
}

/// Hit result from [`RayScene::cast`].
#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub id: u64,
    pub toi: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// Scene-level ray accel — rebuild when meshes change.
#[derive(Default)]
pub struct RayScene {
    meshes: Vec<RayMesh>,
}

impl RayScene {
    pub fn clear(&mut self) {
        self.meshes.clear();
    }

    /// Insert a triangle mesh in **world space** (already transformed).
    pub fn insert_world_tris(&mut self, id: u64, vertices: &[Vec3], indices: &[[u32; 3]]) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }
        let verts: Vec<Point<f32>> = vertices
            .iter()
            .map(|v| Point::new(v.x, v.y, v.z))
            .collect();
        let idx: Vec<[u32; 3]> = indices.to_vec();
        match TriMesh::new(verts, idx) {
            Ok(mesh) => self.meshes.push(RayMesh { id, mesh }),
            Err(err) => tracing::warn!("shiloh-ray TriMesh build failed for id={id}: {err:?}"),
        }
    }

    /// Insert a unit cube transformed by `world` (proxy when no glTF tris).
    pub fn insert_box(&mut self, id: u64, world: Mat4, half: Vec3) {
        let hx = half.x.max(0.05);
        let hy = half.y.max(0.05);
        let hz = half.z.max(0.05);
        let local = [
            Vec3::new(-hx, -hy, -hz),
            Vec3::new(hx, -hy, -hz),
            Vec3::new(hx, -hy, hz),
            Vec3::new(-hx, -hy, hz),
            Vec3::new(-hx, hy, -hz),
            Vec3::new(hx, hy, -hz),
            Vec3::new(hx, hy, hz),
            Vec3::new(-hx, hy, hz),
        ];
        let verts: Vec<Vec3> = local
            .iter()
            .map(|p| world.transform_point3(*p))
            .collect();
        let indices = [
            [0, 1, 2],
            [0, 2, 3],
            [4, 6, 5],
            [4, 7, 6],
            [0, 4, 5],
            [0, 5, 1],
            [1, 5, 6],
            [1, 6, 2],
            [2, 6, 7],
            [2, 7, 3],
            [3, 7, 4],
            [3, 4, 0],
        ];
        self.insert_world_tris(id, &verts, &indices);
    }

    /// Cast a world-space ray; closest hit wins.
    pub fn cast(&self, origin: Vec3, dir: Vec3, max_toi: f32) -> Option<RayHit> {
        let dir_n = dir.normalize_or_zero();
        if dir_n.length_squared() < 1e-12 {
            return None;
        }
        let ray = ParryRay::new(
            Point::new(origin.x, origin.y, origin.z),
            Vector::new(dir_n.x, dir_n.y, dir_n.z),
        );
        let identity = Isometry::identity();
        let mut best: Option<RayHit> = None;
        for m in &self.meshes {
            // solid = true so we hit front faces from outside
            if let Some(toi) = m.mesh.cast_ray(&identity, &ray, max_toi, true) {
                if toi < 0.0 || toi > max_toi {
                    continue;
                }
                if best.is_some_and(|b| toi >= b.toi) {
                    continue;
                }
                let point = origin + dir_n * toi;
                let normal = m
                    .mesh
                    .cast_ray_and_get_normal(&identity, &ray, max_toi, true)
                    .map(|ri| {
                        let n = ri.normal;
                        Vec3::new(n.x, n.y, n.z).normalize_or_zero()
                    })
                    .unwrap_or(Vec3::Y);
                best = Some(RayHit {
                    id: m.id,
                    toi,
                    point,
                    normal,
                });
            }
        }
        best
    }

    pub fn len(&self) -> usize {
        self.meshes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.meshes.is_empty()
    }
}

/// Build a camera ray from NDC (x,y in [-1,1], y up) using inverse view-proj.
pub fn camera_ray_from_ndc(view_proj: Mat4, ndc_x: f32, ndc_y: f32) -> (Vec3, Vec3) {
    let inv = view_proj.inverse();
    // Vulkan-style depth [0,1] for glam perspective_rh
    let near_h = inv * glam::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_h = inv * glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near = near_h.truncate() / near_h.w;
    let far = far_h.truncate() / far_h.w;
    let dir = (far - near).normalize_or_zero();
    (near, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_unit_box() {
        let mut scene = RayScene::default();
        scene.insert_box(7, Mat4::IDENTITY, Vec3::splat(0.5));
        let hit = scene
            .cast(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, -1.0), 100.0)
            .expect("hit");
        assert_eq!(hit.id, 7);
        assert!((hit.point.z - 0.5).abs() < 0.05);
    }

    #[test]
    fn misses_aside() {
        let mut scene = RayScene::default();
        scene.insert_box(1, Mat4::IDENTITY, Vec3::splat(0.5));
        assert!(scene
            .cast(Vec3::new(5.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0), 100.0)
            .is_none());
    }
}
