//! Keyframed animation clips.

use glam::{Quat, Vec3};

use crate::skeleton::{Pose, Skeleton};

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

impl AnimationClip {
    /// Sample local TRS into `pose` at time `t` (loops if duration > 0).
    pub fn sample_into(&self, t: f32, pose: &mut Pose) {
        let t = if self.duration > 0.0 {
            t.rem_euclid(self.duration)
        } else {
            0.0
        };
        for track in &self.tracks {
            let j = track.joint as usize;
            if j >= pose.translations.len() {
                continue;
            }
            if let Some(tr) = &track.translation {
                pose.translations[j] = sample_vec3(tr, t);
            }
            if let Some(rot) = &track.rotation {
                pose.rotations[j] = sample_quat(rot, t);
            }
            if let Some(sc) = &track.scale {
                pose.scales[j] = sample_vec3(sc, t);
            }
        }
    }

    pub fn sample_pose(&self, skeleton: &Skeleton, t: f32) -> Pose {
        let mut pose = Pose::bind_pose(skeleton);
        // Seed bind locals as TRS defaults when tracks are sparse.
        for (i, joint) in skeleton.joints.iter().enumerate() {
            let (_s, r, t) = joint.bind_local.to_scale_rotation_translation();
            pose.translations[i] = t;
            pose.rotations[i] = r;
            pose.scales[i] = _s;
        }
        self.sample_into(t, &mut pose);
        pose
    }
}

fn sample_vec3(track: &Vec3Track, t: f32) -> Vec3 {
    if track.times.is_empty() || track.values.is_empty() {
        return Vec3::ZERO;
    }
    if t <= track.times[0] || track.times.len() == 1 {
        return track.values[0];
    }
    let last = track.times.len() - 1;
    if t >= track.times[last] {
        return track.values[last];
    }
    for i in 0..last {
        let t0 = track.times[i];
        let t1 = track.times[i + 1];
        if t >= t0 && t <= t1 {
            let u = if (t1 - t0).abs() < 1e-8 {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            return track.values[i].lerp(track.values[i + 1], u);
        }
    }
    track.values[last]
}

fn sample_quat(track: &QuatTrack, t: f32) -> Quat {
    if track.times.is_empty() || track.values.is_empty() {
        return Quat::IDENTITY;
    }
    if t <= track.times[0] || track.times.len() == 1 {
        return track.values[0];
    }
    let last = track.times.len() - 1;
    if t >= track.times[last] {
        return track.values[last];
    }
    for i in 0..last {
        let t0 = track.times[i];
        let t1 = track.times[i + 1];
        if t >= t0 && t <= t1 {
            let u = if (t1 - t0).abs() < 1e-8 {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            return track.values[i].slerp(track.values[i + 1], u);
        }
    }
    track.values[last]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_lerp_translation() {
        let clip = AnimationClip {
            name: "t".into(),
            duration: 1.0,
            tracks: vec![JointTracks {
                joint: 0,
                translation: Some(Vec3Track {
                    times: vec![0.0, 1.0],
                    values: vec![Vec3::ZERO, Vec3::X],
                }),
                rotation: None,
                scale: None,
            }],
        };
        let mut pose = Pose {
            translations: vec![Vec3::ZERO],
            rotations: vec![Quat::IDENTITY],
            scales: vec![Vec3::ONE],
        };
        clip.sample_into(0.5, &mut pose);
        assert!((pose.translations[0].x - 0.5).abs() < 1e-4);
    }
}
