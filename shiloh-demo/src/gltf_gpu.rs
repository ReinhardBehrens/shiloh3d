//! Converts `shiloh-assets` glTF imports into GPU-ready slice meshes.
//!
//! Kept in the demo (rather than `shiloh-render` or `shiloh-assets`) to avoid a
//! `shiloh-render <-> shiloh-assets` crate dependency cycle: `shiloh-render`
//! stays mesh-format-agnostic and `shiloh-assets` stays render-API-agnostic.

use glam::Vec3;
use shiloh_assets::ImportedPrimitive;
use shiloh_render::{SkinnedMeshCpu, SkinnedVertex, SliceMeshCpu, SliceVertex};

/// Converts an imported primitive into a static [`SliceMeshCpu`] for the slice
/// PBR / shadow pipelines. Vertex color is the imported vertex color tinted by
/// the material's base color, matching how the built-in cubes/spheres bake
/// color into the vertex stream for the shared checker-albedo shader.
pub fn to_slice_mesh(prim: &ImportedPrimitive) -> SliceMeshCpu {
    let base_color = Vec3::new(
        prim.material.base_color[0],
        prim.material.base_color[1],
        prim.material.base_color[2],
    );
    let vertices = prim
        .vertices
        .iter()
        .map(|v| {
            let vcol = Vec3::from_array(v.color);
            SliceVertex {
                position: v.position,
                normal: v.normal,
                uv: v.uv,
                color: (vcol * base_color).to_array(),
            }
        })
        .collect();
    SliceMeshCpu {
        vertices,
        indices: prim.indices.clone(),
    }
}

/// Converts an imported (skinned) primitive into a [`SkinnedMeshCpu`] for the
/// slice skinned pipeline. glTF joint indices are widened from `u16` to the
/// `u32` expected by the WGSL skin shader.
#[allow(dead_code)]
pub fn to_skinned_mesh(prim: &ImportedPrimitive) -> SkinnedMeshCpu {
    let base_color = Vec3::new(
        prim.material.base_color[0],
        prim.material.base_color[1],
        prim.material.base_color[2],
    );
    let vertices = prim
        .vertices
        .iter()
        .map(|v| {
            let vcol = Vec3::from_array(v.color);
            SkinnedVertex {
                position: v.position,
                normal: v.normal,
                uv: v.uv,
                color: (vcol * base_color).to_array(),
                joints: [
                    u32::from(v.joints[0]),
                    u32::from(v.joints[1]),
                    u32::from(v.joints[2]),
                    u32::from(v.joints[3]),
                ],
                weights: v.weights,
            }
        })
        .collect();
    SkinnedMeshCpu {
        vertices,
        indices: prim.indices.clone(),
    }
}
