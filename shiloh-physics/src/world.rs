//! Physics world trait + stub integrator (pure Rust).

use glam::Vec3;

use crate::body::RigidBody;

pub trait PhysicsBackend: Send + Sync {
    fn step(&mut self, dt: f32);
    fn set_gravity(&mut self, gravity: Vec3);
}

/// Placeholder integrator for bring-up without a native physics SDK.
///
/// This crate intentionally has no dependency on `shiloh-scene`, so it does not
/// know about `Transform`. Callers that want to reflect physics state into the
/// scene graph should read `RigidBody::position` (via [`StubPhysics::bodies`] or
/// [`StubPhysics::body_mut`]) each fixed step and copy it into their own
/// `Transform::translation`, then mark the transform dirty for propagation.
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

    /// Read-only access to all bodies (e.g. for syncing into scene transforms).
    pub fn bodies(&self) -> &[RigidBody] {
        &self.bodies
    }

    /// Mutable access to a single body by index, if it exists.
    pub fn body_mut(&mut self, id: usize) -> Option<&mut RigidBody> {
        self.bodies.get_mut(id)
    }
}

impl PhysicsBackend for StubPhysics {
    fn step(&mut self, dt: f32) {
        for body in &mut self.bodies {
            if body.kind == crate::body::RigidBodyKind::Dynamic {
                body.linear_velocity += self.gravity * dt;
                body.position += body.linear_velocity * dt;
                if body.position.y < 0.0 {
                    body.position.y = 0.0;
                }
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

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::RigidBodyKind;

    #[test]
    fn dynamic_body_falls_and_moves() {
        let mut physics = StubPhysics::new();
        let id = physics.add_body(RigidBody {
            kind: RigidBodyKind::Dynamic,
            position: Vec3::new(0.0, 3.0, 0.0),
            linear_velocity: Vec3::new(1.0, 0.0, 0.0),
            ..Default::default()
        });

        physics.step(1.0 / 60.0);

        let body = physics.body_mut(id).expect("body exists");
        assert!(body.linear_velocity.y < 0.0, "gravity should pull velocity down");
        assert!(body.position.x > 0.0, "sideways velocity should move position");
        assert!(body.position.y < 3.0, "gravity should reduce height after one step");
    }

    #[test]
    fn dynamic_body_clamps_to_ground() {
        let mut physics = StubPhysics::new();
        physics.add_body(RigidBody {
            kind: RigidBodyKind::Dynamic,
            position: Vec3::new(0.0, 0.01, 0.0),
            linear_velocity: Vec3::new(0.0, -50.0, 0.0),
            ..Default::default()
        });

        physics.step(1.0);

        assert_eq!(physics.bodies()[0].position.y, 0.0);
    }

    #[test]
    fn static_body_does_not_move() {
        let mut physics = StubPhysics::new();
        physics.add_body(RigidBody {
            kind: RigidBodyKind::Static,
            position: Vec3::new(1.0, 2.0, 3.0),
            ..Default::default()
        });

        physics.step(1.0 / 60.0);

        assert_eq!(physics.bodies()[0].position, Vec3::new(1.0, 2.0, 3.0));
    }
}
