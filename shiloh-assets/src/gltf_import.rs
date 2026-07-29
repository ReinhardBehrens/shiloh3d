//! glTF 2.0 import behind a Shiloh-owned mesh type (no `gltf` in game APIs).

use std::path::Path;

use glam::{Mat4, Quat, Vec3, Vec4};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GltfError {
    #[error("gltf feature disabled")]
    FeatureDisabled,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "gltf")]
    #[error("gltf: {0}")]
    Gltf(#[from] gltf::Error),
    #[error("{0}")]
    Message(String),
}

/// Interleaved PBR vertex (Shiloh layout).
#[derive(Clone, Debug)]
pub struct ImportedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

#[derive(Clone, Debug, Default)]
pub struct ImportedMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// Optional RGBA8 albedo bytes (width*height*4).
    pub albedo_rgba: Option<(u32, u32, Vec<u8>)>,
}

#[derive(Clone, Debug)]
pub struct ImportedPrimitive {
    pub vertices: Vec<ImportedVertex>,
    pub indices: Vec<u32>,
    pub material: ImportedMaterial,
    pub skinned: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ImportedSkin {
    pub inverse_bind: Vec<Mat4>,
    pub joint_parents: Vec<Option<u16>>,
    pub joint_local: Vec<Mat4>,
}

#[derive(Clone, Debug, Default)]
pub struct ImportedGltf {
    pub name: String,
    pub primitives: Vec<ImportedPrimitive>,
    pub skin: Option<ImportedSkin>,
}

/// Loads the first mesh (and optional skin) from a `.gltf` / `.glb` file.
pub fn load_gltf(path: impl AsRef<Path>) -> Result<ImportedGltf, GltfError> {
    #[cfg(not(feature = "gltf"))]
    {
        let _ = path;
        return Err(GltfError::FeatureDisabled);
    }
    #[cfg(feature = "gltf")]
    {
        load_gltf_inner(path.as_ref())
    }
}

#[cfg(feature = "gltf")]
fn load_gltf_inner(path: &Path) -> Result<ImportedGltf, GltfError> {
    let (document, buffers, images) = gltf::import(path)?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("gltf")
        .to_string();

    let mut out = ImportedGltf {
        name,
        primitives: Vec::new(),
        skin: None,
    };

    // Optional first skin.
    if let Some(skin) = document.skins().next() {
        let mut inverse_bind = Vec::new();
        let reader = skin.reader(|b| Some(&buffers[b.index()]));
        if let Some(iter) = reader.read_inverse_bind_matrices() {
            for m in iter {
                inverse_bind.push(Mat4::from_cols_array_2d(&m));
            }
        }
        let joints: Vec<_> = skin.joints().collect();
        let mut joint_parents = vec![None; joints.len()];
        let mut joint_local = vec![Mat4::IDENTITY; joints.len()];
        for (ji, node) in joints.iter().enumerate() {
            let t = node.transform().decomposed();
            let (trans, rot, scale) = t;
            joint_local[ji] = Mat4::from_scale_rotation_translation(
                Vec3::from(scale),
                Quat::from_xyzw(rot[0], rot[1], rot[2], rot[3]),
                Vec3::from(trans),
            );
            for child in node.children() {
                if let Some(ci) = joints.iter().position(|j| j.index() == child.index()) {
                    joint_parents[ci] = Some(ji as u16);
                }
            }
        }
        if inverse_bind.len() < joints.len() {
            inverse_bind.resize(joints.len(), Mat4::IDENTITY);
        }
        out.skin = Some(ImportedSkin {
            inverse_bind,
            joint_parents,
            joint_local,
        });
    }

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|b| Some(&buffers[b.index()]));
            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .map(|i| i.collect())
                .ok_or_else(|| GltfError::Message("mesh missing positions".into()))?;
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|i| i.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|t| t.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
            let colors: Vec<[f32; 3]> = reader
                .read_colors(0)
                .map(|c| {
                    c.into_rgb_f32()
                        .map(|rgb| [rgb[0], rgb[1], rgb[2]])
                        .collect()
                })
                .unwrap_or_else(|| vec![[1.0, 1.0, 1.0]; positions.len()]);

            let mut joints = vec![[0u16; 4]; positions.len()];
            let mut weights = vec![[1.0, 0.0, 0.0, 0.0]; positions.len()];
            let mut skinned = false;
            if let Some(js) = reader.read_joints(0) {
                skinned = true;
                for (i, j) in js.into_u16().enumerate() {
                    joints[i] = j;
                }
            }
            if let Some(ws) = reader.read_weights(0) {
                for (i, w) in ws.into_f32().enumerate() {
                    weights[i] = w;
                }
            }

            let indices: Vec<u32> = reader
                .read_indices()
                .map(|i| i.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            let mut material = ImportedMaterial {
                base_color: [0.8, 0.8, 0.8, 1.0],
                metallic: 0.0,
                roughness: 0.8,
                albedo_rgba: None,
            };
            let mat = primitive.material();
            let pbr = mat.pbr_metallic_roughness();
            material.base_color = pbr.base_color_factor();
            material.metallic = pbr.metallic_factor();
            material.roughness = pbr.roughness_factor();
            if let Some(tex) = pbr.base_color_texture() {
                let image = &images[tex.texture().source().index()];
                material.albedo_rgba = Some((
                    image.width,
                    image.height,
                    image.pixels.clone(),
                ));
            }

            let vertices: Vec<ImportedVertex> = positions
                .iter()
                .enumerate()
                .map(|(i, p)| ImportedVertex {
                    position: *p,
                    normal: normals[i],
                    uv: uvs[i],
                    color: colors[i],
                    joints: joints[i],
                    weights: weights[i],
                })
                .collect();

            out.primitives.push(ImportedPrimitive {
                vertices,
                indices,
                material,
                skinned,
            });
        }
    }

    if out.primitives.is_empty() {
        return Err(GltfError::Message("no mesh primitives in glTF".into()));
    }
    let _ = Vec4::ZERO; // keep glam Vec4 import useful for future
    Ok(out)
}
