//! Software mixer skeleton (f32 interleaved).

use std::sync::Arc;

use parking_lot::Mutex;

use crate::source::{AudioClip, AudioSource, Listener};

/// A currently-playing, fire-and-forget sound effect.
struct OneshotVoice {
    clip: Arc<AudioClip>,
    /// Next unread sample frame within `clip.samples` (mono index).
    frame: usize,
    gain: f32,
}

impl OneshotVoice {
    fn finished(&self) -> bool {
        self.frame >= self.clip.samples.len()
    }
}

pub struct AudioMixer {
    sample_rate: u32,
    sources: Mutex<Vec<AudioSource>>,
    listener: Mutex<Listener>,
    oneshots: Mutex<Vec<OneshotVoice>>,
}

impl AudioMixer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            sources: Mutex::new(Vec::new()),
            listener: Mutex::new(Listener::default()),
            oneshots: Mutex::new(Vec::new()),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn set_listener(&self, listener: Listener) {
        *self.listener.lock() = listener;
    }

    pub fn add_source(&self, source: AudioSource) -> usize {
        let mut sources = self.sources.lock();
        sources.push(source);
        sources.len() - 1
    }

    /// Queues `clip` to start playing immediately, mixed at the next `mix` call(s).
    /// Multiple one-shots can overlap; each tracks its own playback position.
    pub fn play_oneshot(&self, clip: Arc<AudioClip>, gain: f32) {
        self.oneshots.lock().push(OneshotVoice {
            clip,
            frame: 0,
            gain,
        });
    }

    /// Number of one-shot voices still playing (mostly for tests/diagnostics).
    pub fn active_oneshot_count(&self) -> usize {
        self.oneshots.lock().len()
    }

    /// Fills `out` with the mixed signal for this callback period.
    ///
    /// `out` is treated as interleaved stereo when its length is even (mono clip
    /// samples are duplicated across the L/R pair); otherwise it is treated as a
    /// flat mono buffer. Finished one-shot voices are dropped after mixing.
    pub fn mix(&self, out: &mut [f32]) {
        out.fill(0.0);

        let stereo = out.len() % 2 == 0 && !out.is_empty();
        let mut oneshots = self.oneshots.lock();

        for voice in oneshots.iter_mut() {
            let samples = &voice.clip.samples;
            if stereo {
                let frame_count = out.len() / 2;
                for i in 0..frame_count {
                    if voice.frame >= samples.len() {
                        break;
                    }
                    let s = samples[voice.frame] * voice.gain;
                    out[i * 2] += s;
                    out[i * 2 + 1] += s;
                    voice.frame += 1;
                }
            } else {
                for slot in out.iter_mut() {
                    if voice.frame >= samples.len() {
                        break;
                    }
                    *slot += samples[voice.frame] * voice.gain;
                    voice.frame += 1;
                }
            }
        }

        oneshots.retain(|v| !v.finished());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_oneshot_mixes_nonzero_samples() {
        let mixer = AudioMixer::new(48_000);
        let clip = Arc::new(AudioClip::sine_beep(48_000, 880.0, 0.15, 0.3));
        mixer.play_oneshot(clip, 1.0);

        let mut buf = [0.0f32; 256];
        mixer.mix(&mut buf);

        assert!(
            buf.iter().any(|s| s.abs() > 1e-6),
            "expected non-silent mix buffer after playing a one-shot"
        );
    }

    #[test]
    fn finished_oneshot_is_removed() {
        let mixer = AudioMixer::new(48_000);
        // Very short clip so a single mix call drains it.
        let clip = Arc::new(AudioClip::sine_beep(48_000, 440.0, 0.001, 0.5));
        mixer.play_oneshot(clip, 1.0);
        assert_eq!(mixer.active_oneshot_count(), 1);

        let mut buf = [0.0f32; 256];
        mixer.mix(&mut buf);

        assert_eq!(mixer.active_oneshot_count(), 0);
    }

    #[test]
    fn mix_with_no_voices_is_silent() {
        let mixer = AudioMixer::new(48_000);
        let mut buf = [1.0f32; 16];
        mixer.mix(&mut buf);
        assert!(buf.iter().all(|s| *s == 0.0));
    }
}
