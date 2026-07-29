//! Clip blending.

use crate::clip::AnimationClip;

#[derive(Debug, Clone)]
pub struct BlendLayer {
    pub clip_index: usize,
    pub weight: f32,
    pub time: f32,
}

#[derive(Debug, Default)]
pub struct BlendTree {
    pub clips: Vec<AnimationClip>,
    pub layers: Vec<BlendLayer>,
}

impl BlendTree {
    pub fn add_clip(&mut self, clip: AnimationClip) -> usize {
        self.clips.push(clip);
        self.clips.len() - 1
    }
}
