//! Perspective / orthographic camera for scene rendering.

use glam::{Mat4, Vec3};

/// Camera projection + view helpers (component-friendly POD).
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub fov_y_radians: f32,
    pub aspect: f32,
    pub z_near: f32,
    pub z_far: f32,
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            fov_y_radians: 60f32.to_radians(),
            aspect: 16.0 / 9.0,
            z_near: 0.1,
            z_far: 500.0,
            eye: Vec3::new(0.0, 4.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
        }
    }
}

impl Camera {
    #[inline]
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    #[inline]
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y_radians, self.aspect.max(0.01), self.z_near, self.z_far)
    }

    /// Column-major `view_proj` for WGSL / GPU upload (glam is column-major).
    #[inline]
    pub fn view_proj(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    pub fn set_aspect(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height.max(1) as f32;
    }
}
