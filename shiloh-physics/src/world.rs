//! Physics world trait + stub integrator (pure Rust).

use glam::Vec3;

use crate::body::RigidBody;

pub trait PhysicsBackend: Send + Sync {
    fn step(&mut self, dt: f32);
    fn set_gravity(&mut self, gravity: Vec3);
}

/// Placeholder integrator for bring-up without a native physics SDK.
pub struct StubPhysics {
    gravity: Vec3,
    bodies: Vec<RigidBody>,
}

impl Default for StubPhysics {
    fn default() -> Self {
        Self::new()
    }
}

impl StubPhysics {
    pub fn new() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            bodies: Vec::new(),
        }
    }

    pub fn add_body(&mut self, body: RigidBody) -> usize {
        self.bodies.push(body);
        self.bodies.len() - 1
    }
}

impl PhysicsBackend for StubPhysics {
    fn step(&mut self, dt: f32) {
        for body in &mut self.bodies {
            if body.kind == crate::body::RigidBodyKind::Dynamic {
                body.linear_velocity += self.gravity * dt;
            }
        }
    }

    fn set_gravity(&mut self, gravity: Vec3) {
        self.gravity = gravity;
    }
}

pub struct PhysicsWorld<B: PhysicsBackend> {
    pub backend: B,
}

impl<B: PhysicsBackend> PhysicsWorld<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn step(&mut self, dt: f32) {
        self.backend.step(dt);
    }
}
