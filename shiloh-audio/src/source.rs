//! Audio source and listener components.

use glam::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct Listener {
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

impl Default for Listener {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            forward: -Vec3::Z,
            up: Vec3::Y,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioSource {
    pub gain: f32,
    pub pitch: f32,
    pub looping: bool,
    pub spatial: bool,
    pub position: Vec3,
}

impl Default for AudioSource {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pitch: 1.0,
            looping: false,
            spatial: true,
            position: Vec3::ZERO,
        }
    }
}
