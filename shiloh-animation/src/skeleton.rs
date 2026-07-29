//! Skeleton joint hierarchy.

use glam::{Mat4, Quat, Vec3};
use smallvec::SmallVec;

#[derive(Debug, Clone)]
pub struct Joint {
    pub name: String,
    pub parent: Option<u16>,
    pub bind_local: Mat4,
}

#[derive(Debug, Clone, Default)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
}

impl Skeleton {
    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }
}

/// Pose as local TRS per joint.
#[derive(Debug, Clone, Default)]
pub struct Pose {
    pub translations: Vec<Vec3>,
    pub rotations: Vec<Quat>,
    pub scales: Vec<Vec3>,
}

impl Pose {
    pub fn bind_pose(skeleton: &Skeleton) -> Self {
        let n = skeleton.joints.len();
        Self {
            translations: vec![Vec3::ZERO; n],
            rotations: vec![Quat::IDENTITY; n],
            scales: vec![Vec3::ONE; n],
        }
    }
}

pub type JointPath = SmallVec<[u16; 8]>;
