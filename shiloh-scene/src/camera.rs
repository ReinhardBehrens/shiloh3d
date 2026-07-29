//! Perspective / orthographic / isometric camera for scene rendering.

use glam::{Mat4, Vec3};

/// Projection mode for the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectionKind {
    #[default]
    Perspective,
    /// True orthographic (RTS / top-down).
    Orthographic,
    /// Perspective pitched for isometric ARPG / RTS *feel*.
    Isometric,
}

/// Camera projection + view helpers (component-friendly POD).
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub projection: ProjectionKind,
    pub fov_y_radians: f32,
    /// Half-height of the ortho volume (world units).
    pub ortho_half_extent: f32,
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
            projection: ProjectionKind::Perspective,
            fov_y_radians: 60f32.to_radians(),
            ortho_half_extent: 12.0,
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
    /// Classic isometric-style chase camera looking at `focus`.
    pub fn isometric(focus: Vec3, distance: f32) -> Self {
        let yaw = 45f32.to_radians();
        let pitch = 35.264f32.to_radians(); // classic iso pitch
        let eye = focus
            + Vec3::new(
                distance * yaw.cos() * pitch.cos(),
                distance * pitch.sin(),
                distance * yaw.sin() * pitch.cos(),
            );
        Self {
            projection: ProjectionKind::Isometric,
            eye,
            target: focus,
            fov_y_radians: 35f32.to_radians(),
            ..Default::default()
        }
    }

    pub fn orthographic_top_down(focus: Vec3, half_extent: f32, height: f32) -> Self {
        Self {
            projection: ProjectionKind::Orthographic,
            ortho_half_extent: half_extent,
            eye: focus + Vec3::new(0.0, height, 0.001),
            target: focus,
            up: -Vec3::Z,
            ..Default::default()
        }
    }

    #[inline]
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    #[inline]
    pub fn projection_matrix(&self) -> Mat4 {
        let aspect = self.aspect.max(0.01);
        match self.projection {
            ProjectionKind::Perspective | ProjectionKind::Isometric => {
                Mat4::perspective_rh(self.fov_y_radians, aspect, self.z_near, self.z_far)
            }
            ProjectionKind::Orthographic => {
                let h = self.ortho_half_extent;
                let w = h * aspect;
                Mat4::orthographic_rh(-w, w, -h, h, self.z_near, self.z_far)
            }
        }
    }

    #[inline]
    pub fn view_proj(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    pub fn set_aspect(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height.max(1) as f32;
    }

    /// Pan focus while keeping eye offset (iso / ortho).
    pub fn pan_focus(&mut self, delta: Vec3) {
        self.target += delta;
        self.eye += delta;
    }
}
