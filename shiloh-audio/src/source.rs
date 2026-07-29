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

/// A fully decoded, in-memory audio clip (mono or interleaved multi-channel `f32`).
#[derive(Debug, Clone)]
pub struct AudioClip {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioClip {
    /// Generates a mono sine-wave "beep" clip, useful for smoke tests and UI feedback
    /// sounds before real asset loading is wired up.
    pub fn sine_beep(sample_rate: u32, freq_hz: f32, duration_secs: f32, amplitude: f32) -> Self {
        let frame_count = (sample_rate as f32 * duration_secs).round().max(0.0) as usize;
        let mut samples = Vec::with_capacity(frame_count);
        let angular_freq = std::f32::consts::TAU * freq_hz;
        for i in 0..frame_count {
            let t = i as f32 / sample_rate as f32;
            samples.push((angular_freq * t).sin() * amplitude);
        }
        Self {
            samples,
            sample_rate,
            channels: 1,
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
