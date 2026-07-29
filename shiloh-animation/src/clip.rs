//! Keyframed animation clips.

use glam::{Quat, Vec3};

#[derive(Debug, Clone)]
pub struct Vec3Track {
    pub times: Vec<f32>,
    pub values: Vec<Vec3>,
}

#[derive(Debug, Clone)]
pub struct QuatTrack {
    pub times: Vec<f32>,
    pub values: Vec<Quat>,
}

#[derive(Debug, Clone)]
pub struct JointTracks {
    pub joint: u16,
    pub translation: Option<Vec3Track>,
    pub rotation: Option<QuatTrack>,
    pub scale: Option<Vec3Track>,
}

#[derive(Debug, Clone, Default)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<JointTracks>,
}
