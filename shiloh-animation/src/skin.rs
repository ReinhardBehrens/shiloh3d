//! Runtime skin pose evaluation (CPU) — GPU consumes joint matrices.

use glam::Mat4;

use crate::skeleton::{Pose, Skeleton};

/// Flat joint palette for GPU skinning (`mat4` per joint).
#[derive(Clone, Debug, Default)]
pub struct SkinPalette {
    pub joints: Vec<Mat4>,
}

impl SkinPalette {
    pub fn identity(count: usize) -> Self {
        Self {
            joints: vec![Mat4::IDENTITY; count],
        }
    }

    /// Builds world joint matrices from local TRS and inverse-bind.
    pub fn from_locals(
        locals: &[Mat4],
        parents: &[Option<u16>],
        inverse_bind: &[Mat4],
    ) -> Self {
        let n = locals.len();
        let mut world = vec![Mat4::IDENTITY; n];
        for i in 0..n {
            let parent = parents.get(i).copied().flatten();
            world[i] = match parent {
                Some(p) => world[p as usize] * locals[i],
                None => locals[i],
            };
        }
        let joints: Vec<Mat4> = (0..n)
            .map(|i| {
                let ib = inverse_bind.get(i).copied().unwrap_or(Mat4::IDENTITY);
                world[i] * ib
            })
            .collect();
        Self { joints }
    }

    /// Evaluate a local TRS pose against a skeleton (uses bind inverse = identity
    /// when `inverse_bind` is empty — fine for procedural demo skins).
    pub fn from_pose(pose: &Pose, skeleton: &Skeleton, inverse_bind: &[Mat4]) -> Self {
        let n = skeleton.joint_count().max(pose.translations.len());
        let mut locals = vec![Mat4::IDENTITY; n];
        for i in 0..n {
            let t = pose.translations.get(i).copied().unwrap_or(glam::Vec3::ZERO);
            let r = pose
                .rotations
                .get(i)
                .copied()
                .unwrap_or(glam::Quat::IDENTITY);
            let s = pose.scales.get(i).copied().unwrap_or(glam::Vec3::ONE);
            locals[i] = Mat4::from_scale_rotation_translation(s, r, t);
        }
        let parents: Vec<Option<u16>> = (0..n)
            .map(|i| skeleton.joints.get(i).and_then(|j| j.parent))
            .collect();
        let ib = if inverse_bind.is_empty() {
            vec![Mat4::IDENTITY; n]
        } else {
            inverse_bind.to_vec()
        };
        Self::from_locals(&locals, &parents, &ib)
    }

    /// Simple two-bone sway for demos without clips.
    pub fn demo_sway(time: f32, bone_count: usize) -> Self {
        let mut locals = vec![Mat4::IDENTITY; bone_count.max(2)];
        let angle = (time * 2.0).sin() * 0.45;
        locals[1] = Mat4::from_rotation_z(angle);
        if bone_count > 2 {
            locals[2] = Mat4::from_rotation_z(-angle * 0.7);
        }
        let parents: Vec<Option<u16>> = (0..locals.len())
            .map(|i| if i == 0 { None } else { Some((i as u16) - 1) })
            .collect();
        let inverse_bind = vec![Mat4::IDENTITY; locals.len()];
        Self::from_locals(&locals, &parents, &inverse_bind)
    }
}

/// Convenience: bind-pose palette size from skeleton joint count.
pub fn bind_palette(skeleton: &Skeleton) -> SkinPalette {
    SkinPalette::identity(skeleton.joint_count().max(1))
}
