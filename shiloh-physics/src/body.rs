//! Rigid body component.

use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyKind {
    Static,
    Kinematic,
    Dynamic,
}

#[derive(Debug, Clone, Copy)]
pub struct RigidBody {
    pub kind: RigidBodyKind,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub mass: f32,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            kind: RigidBodyKind::Dynamic,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: 1.0,
        }
    }
}
