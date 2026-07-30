//! Converts `shiloh-assets` glTF imports into editor slice meshes.

use glam::Vec3;
use shiloh_assets::ImportedPrimitive;
use shiloh_render::{SliceMeshCpu, SliceVertex};

/// Average RGB from embedded albedo bytes (0–1).
fn average_albedo(rgba: &(u32, u32, Vec<u8>)) -> Vec3 {
    let (w, h, data) = rgba;
    let count = (w * h) as usize;
    if count == 0 || data.len() < count * 4 {
        return Vec3::ONE;
    }
    let mut sum = Vec3::ZERO;
    for px in data.chunks_exact(4).take(count) {
        sum += Vec3::new(px[0] as f32, px[1] as f32, px[2] as f32) / 255.0;
    }
    sum / count as f32
}

/// Converts an imported primitive into a static [`SliceMeshCpu`] for the slice
/// PBR / shadow pipelines. Vertex color is tinted by material base color and, when
/// present, the average albedo texture color so photogrammetry scans read without
/// full multi-material PBR yet.
pub fn to_slice_mesh(prim: &ImportedPrimitive) -> SliceMeshCpu {
    let base_color = Vec3::new(
        prim.material.base_color[0],
        prim.material.base_color[1],
        prim.material.base_color[2],
    );
    let albedo_tint = prim
        .material
        .albedo_rgba
        .as_ref()
        .map(average_albedo)
        .unwrap_or(Vec3::ONE);
    let tint = base_color * albedo_tint;

    let vertices = prim
        .vertices
        .iter()
        .map(|v| {
            let vcol = Vec3::from_array(v.color);
            SliceVertex {
                position: v.position,
                normal: v.normal,
                uv: v.uv,
                color: (vcol * tint).to_array(),
            }
        })
        .collect();

    SliceMeshCpu {
        vertices,
        indices: prim.indices.clone(),
    }
}
