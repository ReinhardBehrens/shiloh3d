//! Simple animation state machine with timed blend transitions.

use crate::clip::AnimationClip;
use crate::skeleton::{Pose, Skeleton};

#[derive(Debug, Clone)]
pub struct AnimState {
    pub name: String,
    pub clip_index: usize,
}

#[derive(Debug, Clone)]
pub struct AnimTransition {
    pub from: usize,
    pub to: usize,
    pub duration: f32,
}

#[derive(Debug, Default)]
pub struct AnimStateMachine {
    pub states: Vec<AnimState>,
    pub transitions: Vec<AnimTransition>,
    pub current: usize,
    /// Active blend (from → to), if any.
    blend_from: Option<usize>,
    blend_t: f32,
    blend_duration: f32,
    pub time: f32,
}

impl AnimStateMachine {
    pub fn goto(&mut self, state: usize) {
        if state >= self.states.len() || state == self.current {
            return;
        }
        let duration = self
            .transitions
            .iter()
            .find(|t| t.from == self.current && t.to == state)
            .map(|t| t.duration)
            .unwrap_or(0.15);
        if duration <= 0.0 {
            self.current = state;
            self.blend_from = None;
            self.blend_t = 0.0;
            self.time = 0.0;
            return;
        }
        self.blend_from = Some(self.current);
        self.current = state;
        self.blend_t = 0.0;
        self.blend_duration = duration;
        self.time = 0.0;
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        if self.blend_from.is_some() {
            self.blend_t += dt;
            if self.blend_t >= self.blend_duration {
                self.blend_from = None;
                self.blend_t = 0.0;
            }
        }
    }

    pub fn evaluate(&self, clips: &[AnimationClip], skeleton: &Skeleton) -> Pose {
        let cur = self.states.get(self.current);
        let mut pose = if let Some(st) = cur {
            clips
                .get(st.clip_index)
                .map(|c| c.sample_pose(skeleton, self.time))
                .unwrap_or_else(|| Pose::bind_pose(skeleton))
        } else {
            Pose::bind_pose(skeleton)
        };

        if let Some(from) = self.blend_from
            && let Some(st) = self.states.get(from)
            && let Some(clip) = clips.get(st.clip_index)
        {
            let from_pose = clip.sample_pose(skeleton, self.time);
            let u = (self.blend_t / self.blend_duration.max(1e-4)).clamp(0.0, 1.0);
            let to_pose = pose.clone();
            blend_poses(&from_pose, &to_pose, u, &mut pose);
        }
        pose
    }
}

fn blend_poses(a: &Pose, b: &Pose, u: f32, out: &mut Pose) {
    let n = out
        .translations
        .len()
        .min(a.translations.len())
        .min(b.translations.len());
    for i in 0..n {
        out.translations[i] = a.translations[i].lerp(b.translations[i], u);
        out.rotations[i] = a.rotations[i].slerp(b.rotations[i], u);
        out.scales[i] = a.scales[i].lerp(b.scales[i], u);
    }
}
