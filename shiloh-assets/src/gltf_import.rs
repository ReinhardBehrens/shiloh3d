//! glTF 2.0 import behind a Shiloh-owned mesh type (no `gltf` in game APIs).

use std::path::Path;

use glam::{Mat4, Quat, Vec3, Vec4};
use shiloh_animation::{
    AnimationClip, Joint, JointTracks, QuatTrack, Skeleton, Vec3Track,
};
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
    pub joint_names: Vec<String>,
}

impl ImportedSkin {
    pub fn to_skeleton(&self) -> Skeleton {
        let joints = self
            .joint_local
            .iter()
            .enumerate()
            .map(|(i, bind_local)| Joint {
                name: self
                    .joint_names
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("joint_{i}")),
                parent: self.joint_parents.get(i).copied().flatten(),
                bind_local: *bind_local,
            })
            .collect();
        Skeleton { joints }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImportedAnimation {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<JointTracks>,
}

impl ImportedAnimation {
    pub fn to_clip(&self) -> AnimationClip {
        AnimationClip {
            name: self.name.clone(),
            duration: self.duration,
            tracks: self.tracks.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImportedGltf {
    pub name: String,
    pub primitives: Vec<ImportedPrimitive>,
    pub skin: Option<ImportedSkin>,
    pub animations: Vec<ImportedAnimation>,
}

/// Loads the first mesh (and optional skin + animations) from a `.gltf` / `.glb`.
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
        animations: Vec::new(),
    };

    // Optional first skin.
    let mut joint_node_indices: Vec<usize> = Vec::new();
    if let Some(skin) = document.skins().next() {
        let mut inverse_bind = Vec::new();
        let reader = skin.reader(|b| Some(&buffers[b.index()]));
        if let Some(iter) = reader.read_inverse_bind_matrices() {
            for m in iter {
                inverse_bind.push(Mat4::from_cols_array_2d(&m));
            }
        }
        let joints: Vec<_> = skin.joints().collect();
        joint_node_indices = joints.iter().map(|n| n.index()).collect();
        let mut joint_parents = vec![None; joints.len()];
        let mut joint_local = vec![Mat4::IDENTITY; joints.len()];
        let mut joint_names = Vec::with_capacity(joints.len());
        for (ji, node) in joints.iter().enumerate() {
            joint_names.push(node.name().unwrap_or("joint").to_string());
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
            joint_names,
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
                material.albedo_rgba = Some((image.width, image.height, image.pixels.clone()));
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

    // Animations → ImportedAnimation / AnimationClip tracks (joint index via skin).
    for anim in document.animations() {
        let mut duration = 0.0_f32;
        let mut by_joint: ahash::AHashMap<u16, JointTracks> = ahash::AHashMap::default();
        for channel in anim.channels() {
            let target = channel.target();
            let node_idx = target.node().index();
            let Some(joint) = joint_node_indices
                .iter()
                .position(|&i| i == node_idx)
                .map(|i| i as u16)
            else {
                // Animation targets a non-skin node — skip for skinned path.
                continue;
            };
            let reader = channel.reader(|b| Some(&buffers[b.index()]));
            let Some(inputs) = reader.read_inputs() else {
                continue;
            };
            let times: Vec<f32> = inputs.collect();
            if let Some(&last) = times.last() {
                duration = duration.max(last);
            }
            let entry = by_joint.entry(joint).or_insert_with(|| JointTracks {
                joint,
                translation: None,
                rotation: None,
                scale: None,
            });
            match target.property() {
                gltf::animation::Property::Translation => {
                    if let Some(outputs) = reader.read_outputs()
                        && let gltf::animation::util::ReadOutputs::Translations(iter) = outputs
                    {
                        let values: Vec<Vec3> = iter.map(Vec3::from).collect();
                        entry.translation = Some(Vec3Track { times: times.clone(), values });
                    }
                }
                gltf::animation::Property::Rotation => {
                    if let Some(outputs) = reader.read_outputs()
                        && let gltf::animation::util::ReadOutputs::Rotations(rots) = outputs
                    {
                        let values: Vec<Quat> = rots
                            .into_f32()
                            .map(|q| Quat::from_xyzw(q[0], q[1], q[2], q[3]))
                            .collect();
                        entry.rotation = Some(QuatTrack { times: times.clone(), values });
                    }
                }
                gltf::animation::Property::Scale => {
                    if let Some(outputs) = reader.read_outputs()
                        && let gltf::animation::util::ReadOutputs::Scales(iter) = outputs
                    {
                        let values: Vec<Vec3> = iter.map(Vec3::from).collect();
                        entry.scale = Some(Vec3Track { times: times.clone(), values });
                    }
                }
                gltf::animation::Property::MorphTargetWeights => {}
            }
        }
        if !by_joint.is_empty() {
            out.animations.push(ImportedAnimation {
                name: anim.name().unwrap_or("anim").to_string(),
                duration,
                tracks: by_joint.into_values().collect(),
            });
        }
    }

    if out.primitives.is_empty() {
        return Err(GltfError::Message("no mesh primitives in glTF".into()));
    }
    let _ = Vec4::ZERO;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shiloh_animation::SkinPalette;

    #[test]
    fn skin_to_palette_from_bind_pose() {
        let skin = ImportedSkin {
            inverse_bind: vec![Mat4::IDENTITY; 2],
            joint_parents: vec![None, Some(0)],
            joint_local: vec![Mat4::IDENTITY, Mat4::from_translation(Vec3::Y)],
            joint_names: vec!["root".into(), "child".into()],
        };
        let skeleton = skin.to_skeleton();
        let clip = AnimationClip {
            name: "t".into(),
            duration: 1.0,
            tracks: vec![JointTracks {
                joint: 1,
                translation: Some(Vec3Track {
                    times: vec![0.0, 1.0],
                    values: vec![Vec3::Y, Vec3::new(0.0, 2.0, 0.0)],
                }),
                rotation: None,
                scale: None,
            }],
        };
        let pose = clip.sample_pose(&skeleton, 0.5);
        let palette = SkinPalette::from_pose(&pose, &skeleton, &skin.inverse_bind);
        assert_eq!(palette.joints.len(), 2);
        // Mid-clip child local y ≈ 1.5 → world not identity.
        assert!(palette.joints[1] != Mat4::IDENTITY);
    }
}
